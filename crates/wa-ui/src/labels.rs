//! Clock, stamp and day labels. The official client always renders
//! `8:44 pm` — English, lowercase, 12-hour — independent of the OS locale.

use chrono::{DateTime, Datelike, Local, TimeZone, Timelike};

const MONTHS: [&str; 12] = [
    "Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec",
];
const WEEKDAYS: [&str; 7] = [
    "Monday",
    "Tuesday",
    "Wednesday",
    "Thursday",
    "Friday",
    "Saturday",
    "Sunday",
];

fn local(ms: i64) -> DateTime<Local> {
    Local
        .timestamp_millis_opt(ms)
        .single()
        .unwrap_or_else(Local::now)
}

/// Which calendar day a timestamp belongs to, in local time.
fn day_key(date: &DateTime<Local>) -> i32 {
    date.year() * 10_000 + date.month() as i32 * 100 + date.day() as i32
}

/// Bubble and message stamp: `8:44 pm`.
pub fn clock_label(ms: i64) -> String {
    let date = local(ms);
    let hour = date.hour() % 12;
    let hour = if hour == 0 { 12 } else { hour };
    let meridiem = if date.hour() < 12 { "am" } else { "pm" };
    format!("{hour}:{:02} {meridiem}", date.minute())
}

/// Sidebar stamp: time for today, short date for anything older.
pub fn stamp_of(ms: i64, now: i64) -> String {
    if ms <= 0 {
        return String::new();
    }
    let date = local(ms);
    let today = local(now);
    if day_key(&date) == day_key(&today) {
        return clock_label(ms);
    }
    if date.year() == today.year() {
        format!("{} {}", MONTHS[date.month0() as usize], date.day())
    } else {
        format!(
            "{} {}, {:02}",
            MONTHS[date.month0() as usize],
            date.day(),
            date.year() % 100
        )
    }
}

/// Conversation divider: `Today`, `Yesterday`, the weekday within a week, then
/// the full date.
pub fn day_of(ms: i64, now: i64) -> String {
    let date = local(if ms > 0 { ms } else { now });
    let today = local(now);
    let key = day_key(&date);
    if key == day_key(&today) {
        return "Today".to_string();
    }
    let yesterday = today - chrono::Duration::days(1);
    if key == day_key(&yesterday) {
        return "Yesterday".to_string();
    }
    if today.timestamp_millis() - date.timestamp_millis() < 7 * 86_400_000 {
        return WEEKDAYS[date.weekday().num_days_from_monday() as usize].to_string();
    }
    format!(
        "{} {}, {}",
        MONTHS[date.month0() as usize],
        date.day(),
        date.year()
    )
}

/// `7100000` -> `"7.1m followers"`, `84300` -> `"84.3k followers"`.
pub fn format_followers(count: i64) -> String {
    if count <= 0 {
        return "Channel".to_string();
    }
    let one_decimal = |value: f64| {
        let text = format!("{value:.1}");
        text.trim_end_matches(".0").to_string()
    };
    if count >= 1_000_000 {
        format!("{}m followers", one_decimal(count as f64 / 1_000_000.0))
    } else if count >= 1_000 {
        format!("{}k followers", one_decimal(count as f64 / 1_000.0))
    } else if count == 1 {
        "1 follower".to_string()
    } else {
        format!("{count} followers")
    }
}

/// `34` -> `"Group - 34 members"`, `1` -> `"Group - 1 member"`.
pub fn group_member_count(count: usize) -> String {
    if count == 1 {
        "Group - 1 member".to_string()
    } else {
        format!("Group - {count} members")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn at(y: i32, m: u32, d: u32, h: u32, min: u32) -> i64 {
        Local
            .with_ymd_and_hms(y, m, d, h, min, 0)
            .single()
            .unwrap()
            .timestamp_millis()
    }

    #[test]
    fn the_group_member_count_reads_naturally() {
        assert_eq!(group_member_count(34), "Group - 34 members");
        assert_eq!(group_member_count(1), "Group - 1 member");
        assert_eq!(group_member_count(0), "Group - 0 members");
    }

    #[test]
    fn clock_is_lowercase_twelve_hour() {
        assert_eq!(clock_label(at(2026, 7, 4, 20, 47)), "8:47 pm");
        assert_eq!(clock_label(at(2026, 7, 4, 8, 5)), "8:05 am");
        assert_eq!(clock_label(at(2026, 7, 4, 0, 0)), "12:00 am");
        assert_eq!(clock_label(at(2026, 7, 4, 12, 0)), "12:00 pm");
    }

    #[test]
    fn stamps_switch_from_time_to_date() {
        let now = at(2026, 7, 4, 20, 0);
        assert_eq!(stamp_of(at(2026, 7, 4, 9, 30), now), "9:30 am");
        assert_eq!(stamp_of(at(2026, 7, 3, 9, 30), now), "Jul 3");
        assert_eq!(stamp_of(at(2025, 12, 24, 9, 30), now), "Dec 24, 25");
        assert_eq!(stamp_of(0, now), "");
    }

    #[test]
    fn day_labels() {
        let now = at(2026, 7, 4, 20, 0);
        assert_eq!(day_of(at(2026, 7, 4, 1, 0), now), "Today");
        assert_eq!(day_of(at(2026, 7, 3, 23, 0), now), "Yesterday");
        assert_eq!(day_of(at(2026, 7, 1, 12, 0), now), "Wednesday");
        assert_eq!(day_of(at(2026, 1, 9, 12, 0), now), "Jan 9, 2026");
    }

    #[test]
    fn follower_counts() {
        assert_eq!(format_followers(0), "Channel");
        assert_eq!(format_followers(1), "1 follower");
        assert_eq!(format_followers(84), "84 followers");
        assert_eq!(format_followers(84_300), "84.3k followers");
        assert_eq!(format_followers(7_100_000), "7.1m followers");
        assert_eq!(format_followers(2_000_000), "2m followers");
    }
}
