//! Chat-export (`.txt`) parsing and the import merge plan: which lines become
//! messages, which already-imported rows are superseded, and which parsed
//! messages are new.

use chrono::{Local, TimeZone};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ExportMessage {
    #[serde(default)]
    pub sender: String,
    #[serde(default)]
    pub text: String,
    #[serde(default)]
    pub timestamp: i64,
}

#[derive(Debug, Clone, Default, Deserialize)]
pub struct ExistingMessage {
    #[serde(default)]
    pub id: String,
    #[serde(default)]
    pub timestamp: i64,
    #[serde(default)]
    pub text: String,
}

#[derive(Debug, Default, Serialize)]
pub struct ImportPlan {
    /// Parsed messages that are not in the chat yet.
    pub add: Vec<ExportMessage>,
    /// Superseded `export-*` rows to drop (the phone sent the same message).
    pub remove_export_ids: Vec<String>,
}

/// The phone may already have part of the export; the window absorbs its
/// second-level rounding.
pub const SAME_WINDOW_MS: i64 = 120_000;

fn strip_bidi(line: &str) -> String {
    line.chars()
        .filter(|ch| !matches!(*ch, '\u{200e}' | '\u{200f}' | '\u{202a}'..='\u{202e}'))
        .collect()
}

fn normal(text: &str) -> String {
    text.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// `[d/m/yy h:mm:ss AM] text` — the official export's line header.
struct Header {
    year: i32,
    month: u32,
    day: u32,
    hour: u32,
    minute: u32,
    second: u32,
    rest: String,
}

fn parse_header(line: &str) -> Option<Header> {
    let rest = line.strip_prefix('[')?;
    let close = rest.find(']')?;
    let stamp = &rest[..close];
    let body = rest[close + 1..].trim_start().to_string();
    let (date, time) = stamp.split_once(", ")?;
    let mut date_parts = date.split('/');
    let day: u32 = date_parts.next()?.trim().parse().ok()?;
    let month: u32 = date_parts.next()?.trim().parse().ok()?;
    let year_raw = date_parts.next()?.trim();
    if date_parts.next().is_some() {
        return None;
    }
    let year = match year_raw.len() {
        2 => 2000 + year_raw.parse::<i32>().ok()?,
        4 => year_raw.parse().ok()?,
        _ => return None,
    };
    let (clock, ampm) = match time.rsplit_once(' ') {
        Some((clock, suffix))
            if suffix.eq_ignore_ascii_case("am") || suffix.eq_ignore_ascii_case("pm") =>
        {
            (clock, Some(suffix.to_ascii_lowercase()))
        }
        _ => (time, None),
    };
    let mut clock_parts = clock.split(':');
    let mut hour: u32 = clock_parts.next()?.trim().parse().ok()?;
    let minute: u32 = clock_parts.next()?.trim().parse().ok()?;
    let second: u32 = match clock_parts.next() {
        Some(value) => value.trim().parse().ok()?,
        None => 0,
    };
    if clock_parts.next().is_some() {
        return None;
    }
    if let Some(ampm) = ampm {
        if ampm == "pm" && hour < 12 {
            hour += 12;
        }
        if ampm == "am" && hour == 12 {
            hour = 0;
        }
    }
    if month == 0 || month > 12 || day == 0 || day > 31 || hour > 23 || minute > 59 || second > 59 {
        return None;
    }
    Some(Header {
        year,
        month,
        day,
        hour,
        minute,
        second,
        rest: body,
    })
}

/// Parse a WhatsApp "Export chat" file. Continuation lines extend the message
/// above them; system entries (`- Alice left`) and separator lines are ignored.
pub fn parse(text: &str, allow_no_sender: bool) -> Vec<ExportMessage> {
    let mut out: Vec<ExportMessage> = Vec::new();
    let mut current: Option<usize> = None;

    for raw in text.split('\n') {
        let line = strip_bidi(raw.trim_end_matches('\r'));
        let Some(header) = parse_header(&line) else {
            if let Some(index) = current {
                let trimmed = line.trim();
                if !trimmed.is_empty() {
                    out[index].text.push('\n');
                    out[index].text.push_str(trimmed);
                }
            }
            continue;
        };
        let Some(timestamp) = Local
            .with_ymd_and_hms(
                header.year,
                header.month,
                header.day,
                header.hour,
                header.minute,
                header.second,
            )
            .single()
            .map(|value| value.timestamp_millis())
        else {
            current = None;
            continue;
        };

        let body = header.rest.trim();
        if body.is_empty() || body.starts_with("- ") {
            current = None;
            continue;
        }
        if allow_no_sender {
            out.push(ExportMessage {
                sender: "Channel".to_string(),
                text: body.to_string(),
                timestamp,
            });
            current = Some(out.len() - 1);
            continue;
        }
        let Some((sender, message_text)) = body.split_once(": ") else {
            current = None;
            continue;
        };
        let sender = sender.trim();
        let message_text = message_text.trim();
        if sender.is_empty()
            || message_text.is_empty()
            || message_text
                .to_lowercase()
                .starts_with("waiting for this message")
        {
            current = None;
            continue;
        }
        out.push(ExportMessage {
            sender: sender.to_string(),
            text: message_text.to_string(),
            timestamp,
        });
        current = Some(out.len() - 1);
    }
    out
}

fn same(left_ts: i64, left_text: &str, right_ts: i64, right_text: &str) -> bool {
    (left_ts - right_ts).abs() < SAME_WINDOW_MS && normal(left_text) == normal(right_text)
}

/// Decide what an import adds: rows already replaced by a real message are
/// dropped, and parsed messages that the chat already has are skipped.
pub fn import_plan(parsed: &[ExportMessage], existing: &[ExistingMessage]) -> ImportPlan {
    let mut keep: Vec<&ExistingMessage> = existing.iter().collect();
    let mut remove_export_ids: Vec<String> = Vec::new();

    for message in existing {
        if !message.id.starts_with("export-") {
            continue;
        }
        let superseded = existing.iter().any(|other| {
            other.id != message.id
                && !other.id.starts_with("export-")
                && same(
                    message.timestamp,
                    &message.text,
                    other.timestamp,
                    &other.text,
                )
        });
        if superseded {
            remove_export_ids.push(message.id.clone());
            keep.retain(|entry| entry.id != message.id);
        }
    }

    let mut known: Vec<(i64, String)> = keep
        .iter()
        .map(|message| (message.timestamp, normal(&message.text)))
        .collect();
    let mut add = Vec::new();
    for item in parsed {
        let text = normal(&item.text);
        if known
            .iter()
            .any(|(ts, known_text)| same(item.timestamp, &text, *ts, known_text))
        {
            continue;
        }
        known.push((item.timestamp, text));
        add.push(item.clone());
    }

    ImportPlan {
        add,
        remove_export_ids,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn at(y: i32, m: u32, d: u32, h: u32, mi: u32, s: u32) -> i64 {
        Local
            .with_ymd_and_hms(y, m, d, h, mi, s)
            .single()
            .unwrap()
            .timestamp_millis()
    }

    #[test]
    fn parses_messages_and_continuations() {
        let text =
            "[4/7/26, 8:44:05 PM] Alice: hello\nsecond line\n[4/7/26, 8:45 PM] Bob: hi there";
        let parsed = parse(text, false);
        assert_eq!(parsed.len(), 2);
        assert_eq!(parsed[0].sender, "Alice");
        assert_eq!(parsed[0].text, "hello\nsecond line");
        assert_eq!(parsed[0].timestamp, at(2026, 7, 4, 20, 44, 5));
        assert_eq!(parsed[1].sender, "Bob");
        assert_eq!(parsed[1].timestamp, at(2026, 7, 4, 20, 45, 0));
    }

    #[test]
    fn skips_system_lines_and_waiting_stubs() {
        let text = "[4/7/26, 8:44 PM] Alice: hi\n[4/7/26, 8:45 PM] - Alice left\n[4/7/26, 8:46 PM] Bob: Waiting for this message. This may take a while.\n[4/7/26, 8:47 PM] Bob: real";
        let parsed = parse(text, false);
        assert_eq!(parsed.len(), 2);
        assert_eq!(parsed[0].text, "hi");
        assert_eq!(parsed[1].text, "real");
    }

    #[test]
    fn channel_exports_have_no_sender_column() {
        let text = "[4/7/26, 8:44 PM] Post: with a colon inside";
        let parsed = parse(text, true);
        assert_eq!(parsed.len(), 1);
        assert_eq!(parsed[0].sender, "Channel");
        assert_eq!(parsed[0].text, "Post: with a colon inside");
    }

    #[test]
    fn am_pm_and_two_digit_years() {
        let parsed = parse("[4/7/26, 12:05 am] A: x\n[4/7/26, 12:15 PM] B: y", false);
        assert_eq!(parsed[0].timestamp, at(2026, 7, 4, 0, 5, 0));
        assert_eq!(parsed[1].timestamp, at(2026, 7, 4, 12, 15, 0));
    }

    #[test]
    fn export_lines_are_bidi_cleaned() {
        let parsed = parse("\u{200e}[4/7/26, 8:44 PM] A: hi\u{200f}", false);
        assert_eq!(parsed.len(), 1);
        assert_eq!(parsed[0].text, "hi");
    }

    #[test]
    fn import_plan_skips_existing_and_drops_superseded_exports() {
        let parsed = parse(
            "[4/7/26, 8:44 PM] Alice: hello\n[4/7/26, 8:50 PM] Alice: brand new",
            false,
        );
        let existing = vec![
            ExistingMessage {
                id: "export-1".into(),
                timestamp: at(2026, 7, 4, 20, 44, 0),
                text: "hello".into(),
            },
            ExistingMessage {
                id: "real-1".into(),
                timestamp: at(2026, 7, 4, 20, 44, 30),
                text: "hello".into(),
            },
        ];
        let plan = import_plan(&parsed, &existing);
        assert_eq!(plan.remove_export_ids, vec!["export-1".to_string()]);
        assert_eq!(plan.add.len(), 1);
        assert_eq!(plan.add[0].text, "brand new");
    }

    #[test]
    fn import_plan_is_idempotent() {
        let parsed = parse("[4/7/26, 8:44 PM] Alice: hello", false);
        let first = import_plan(&parsed, &[]);
        assert_eq!(first.add.len(), 1);
        let existing = vec![ExistingMessage {
            id: "export-1".into(),
            timestamp: first.add[0].timestamp,
            text: first.add[0].text.clone(),
        }];
        let second = import_plan(&parsed, &existing);
        assert!(second.add.is_empty());
        assert!(second.remove_export_ids.is_empty());
    }
}
