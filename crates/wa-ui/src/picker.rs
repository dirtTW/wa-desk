//! The new-chat / forward picker list: saved contacts, known chats and a raw
//! number, with letter sections and the forward-selection state.

use std::collections::{HashMap, HashSet};

use serde::{Deserialize, Serialize};

use crate::html::{avatar, escape};
use crate::text::{format_phone, phone_from_jid};

#[derive(Debug, Clone, Default, Deserialize)]
pub struct PickerChat {
    #[serde(default)]
    pub jid: String,
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub is_group: bool,
}

#[derive(Debug, Clone, Deserialize)]
pub struct PickerRequest {
    #[serde(default)]
    pub contacts: HashMap<String, String>,
    #[serde(default)]
    pub chats: Vec<PickerChat>,
    #[serde(default)]
    pub phones: HashMap<String, String>,
    /// LID user parts already known, so their contacts are not duplicated.
    #[serde(default)]
    pub known_lids: Vec<String>,
    #[serde(default)]
    pub photos: HashMap<String, String>,
    #[serde(default)]
    pub me_jid: String,
    #[serde(default)]
    pub me_lid: String,
    #[serde(default)]
    pub query: String,
    /// True when the list is picking a forward target (groups included).
    #[serde(default)]
    pub forward: bool,
    #[serde(default)]
    pub forward_targets: Vec<String>,
}

#[derive(Debug, Serialize)]
pub struct PickerOutput {
    pub html: String,
    /// The "New group / New contact" action row hides while searching or forwarding.
    pub actions_hidden: bool,
    pub rows: usize,
}

#[derive(Debug, Clone)]
struct Entry {
    jid: String,
    name: String,
    subtitle: String,
    is_self: bool,
}

fn digits_only(text: &str) -> String {
    text.chars().filter(|ch| ch.is_ascii_digit()).collect()
}

fn looks_like_number(query: &str) -> bool {
    let mut chars = query.chars();
    match chars.next() {
        Some(first) if first == '+' || first.is_ascii_digit() => {}
        _ => return false,
    }
    query
        .chars()
        .all(|ch| ch.is_ascii_digit() || matches!(ch, '+' | ' ' | '(' | ')' | '.' | '-'))
}

fn entries(req: &PickerRequest) -> Vec<Entry> {
    let mut entries: Vec<Entry> = Vec::new();
    let mut seen: HashSet<String> = HashSet::new();

    let self_jid = if !req.me_lid.is_empty() {
        req.me_lid.clone()
    } else {
        req.me_jid.clone()
    };
    if !self_jid.is_empty() {
        entries.push(Entry {
            jid: self_jid.clone(),
            name: "You".to_string(),
            subtitle: "Message yourself".to_string(),
            is_self: true,
        });
        seen.insert(self_jid);
    }

    let known_lids: HashSet<&str> = req.known_lids.iter().map(String::as_str).collect();
    let mut by_name: HashMap<String, String> = HashMap::new();
    let mut by_phone: HashMap<String, String> = HashMap::new();
    for chat in &req.chats {
        if chat.is_group {
            continue;
        }
        by_name.insert(chat.name.trim().to_lowercase(), chat.jid.clone());
        by_phone.insert(phone_from_jid(&chat.jid), chat.jid.clone());
        if let Some(resolved) = req.phones.get(&chat.jid)
            && resolved.chars().count() > 5
        {
            by_phone.insert(resolved.clone(), chat.jid.clone());
        }
    }

    let self_phone = phone_from_jid(&req.me_jid);
    for (key, name) in &req.contacts {
        if name.is_empty()
            || !key.chars().all(|ch| ch.is_ascii_digit())
            || !(8..=13).contains(&key.chars().count())
        {
            continue;
        }
        if known_lids.contains(key.as_str()) || *key == self_phone {
            continue;
        }
        let jid = by_name
            .get(&name.trim().to_lowercase())
            .or_else(|| by_phone.get(key))
            .cloned()
            .unwrap_or_else(|| format!("{key}@s.whatsapp.net"));
        if !seen.insert(jid.clone()) {
            continue;
        }
        entries.push(Entry {
            jid,
            name: name.clone(),
            subtitle: format_phone(key),
            is_self: false,
        });
    }

    for chat in &req.chats {
        if chat.is_group {
            if !req.forward || !seen.insert(chat.jid.clone()) {
                continue;
            }
            entries.push(Entry {
                jid: chat.jid.clone(),
                name: chat.name.clone(),
                subtitle: "Group".to_string(),
                is_self: false,
            });
            continue;
        }
        if chat.jid.ends_with("@lid") || !seen.insert(chat.jid.clone()) {
            continue;
        }
        entries.push(Entry {
            jid: chat.jid.clone(),
            name: chat.name.clone(),
            subtitle: format_phone(&phone_from_jid(&chat.jid)),
            is_self: false,
        });
    }

    entries
}

fn row(entry: &Entry, req: &PickerRequest, selected: bool) -> String {
    let target = if req.forward {
        format!(r#"data-forward-jid="{}""#, escape(&entry.jid))
    } else {
        format!(
            r#"data-chat-jid="{}" data-chat-name="{}""#,
            escape(&entry.jid),
            escape(&entry.name)
        )
    };
    format!(
        r#"<button type="button" class="new-chat-row{forward}{selected}" {target}>
      {tick}
      {avatar}
      <div class="new-chat-meta"><strong>{}</strong><span>{}</span></div>
    </button>"#,
        escape(&entry.name),
        escape(&entry.subtitle),
        forward = if req.forward { " forward-row" } else { "" },
        selected = if selected { " selected" } else { "" },
        tick = if req.forward {
            r#"<span class="forward-tick" aria-hidden="true"></span>"#
        } else {
            ""
        },
        avatar = avatar(&entry.name, &entry.jid, "avatar", &req.photos),
    )
}

/// Render the picker list for the current query.
pub fn render(req: &PickerRequest) -> PickerOutput {
    let needle = req.query.trim().to_lowercase();
    let digits = digits_only(&req.query);
    let mut list: Vec<Entry> = entries(req)
        .into_iter()
        .filter(|entry| {
            if needle.is_empty() {
                return true;
            }
            if entry.name.to_lowercase().contains(&needle) {
                return true;
            }
            digits.chars().count() >= 3 && digits_only(&entry.subtitle).contains(&digits)
        })
        .collect();

    list.sort_by(|a, b| {
        if a.is_self != b.is_self {
            return if a.is_self {
                std::cmp::Ordering::Less
            } else {
                std::cmp::Ordering::Greater
            };
        }
        a.name.to_lowercase().cmp(&b.name.to_lowercase())
    });

    let mut html = String::new();
    let mut rows = 0usize;

    // A typed number that is not already a contact gets its own row.
    if !needle.is_empty() && digits.chars().count() >= 7 && looks_like_number(&req.query) {
        let jid = format!("{digits}@s.whatsapp.net");
        if !list.iter().any(|entry| entry.jid == jid) {
            let label = format_phone(&digits);
            let entry = Entry {
                jid,
                name: label.clone(),
                subtitle: "Start a chat with this number".to_string(),
                is_self: false,
            };
            html.push_str(&row(&entry, req, false));
            rows += 1;
        }
    }

    let targets: HashSet<&str> = req.forward_targets.iter().map(String::as_str).collect();
    let mut last_letter = String::new();
    let mut entries_html = String::new();
    for entry in &list {
        let letter = if entry.is_self {
            String::new()
        } else {
            match entry.name.chars().next() {
                Some(first) if first.is_ascii_alphabetic() => first.to_uppercase().to_string(),
                _ => "#".to_string(),
            }
        };
        if !letter.is_empty() && letter != last_letter {
            entries_html.push_str(&format!(
                r#"<div class="new-chat-letter">{}</div>"#,
                escape(&letter)
            ));
            last_letter = letter;
        }
        entries_html.push_str(&row(entry, req, targets.contains(entry.jid.as_str())));
        rows += 1;
    }

    PickerOutput {
        html: html + &entries_html,
        actions_hidden: !needle.is_empty() || req.forward,
        rows,
    }
}

/* ---------------------------------------------------- group member picker */

#[derive(Debug, Clone, Deserialize)]
pub struct GroupContactsRequest {
    #[serde(default)]
    pub contacts: HashMap<String, String>,
    #[serde(default)]
    pub known_lids: Vec<String>,
    #[serde(default)]
    pub photos: HashMap<String, String>,
    #[serde(default)]
    pub me_jid: String,
    #[serde(default)]
    pub query: String,
    /// JIDs already ticked, so the checkboxes survive a repaint.
    #[serde(default)]
    pub selected: Vec<String>,
}

#[derive(Debug, Serialize)]
pub struct GroupContactsOutput {
    pub html: String,
    pub queue: Vec<String>,
    pub empty: bool,
}

/// The "New group" / "Add members" contact checklist.
#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct GroupModal {
    pub title: &'static str,
    pub submit_label: &'static str,
    /// The subject field only belongs to the create dialog.
    pub show_subject: bool,
    /// Which field the dialog focuses when it opens.
    pub focus: &'static str,
    /// The toast after the dialog succeeds.
    pub success_toast: &'static str,
}

/// The toast after forwarding N messages.
pub fn forward_toast(sent: usize) -> String {
    if sent == 1 {
        "Message forwarded".to_string()
    } else {
        format!("{sent} messages forwarded")
    }
}

/// The New group / Add members dialog in `mode` (`create` or `add`).
pub fn group_modal(mode: &str) -> GroupModal {
    if mode == "add" {
        GroupModal {
            title: "Add members",
            submit_label: "Add",
            show_subject: false,
            focus: "group-search",
            success_toast: "Members added",
        }
    } else {
        GroupModal {
            title: "New group",
            submit_label: "Create",
            show_subject: true,
            focus: "group-subject",
            success_toast: "Group created",
        }
    }
}

/// Validation for the new-group / add-members dialog: the first problem, or
/// `None` when the form may be submitted.
pub fn group_form_error(mode: &str, subject: &str, selected: usize) -> Option<String> {
    if mode != "add" && subject.trim().is_empty() {
        return Some("Give the group a name.".to_string());
    }
    if selected == 0 {
        return Some("Pick at least one participant.".to_string());
    }
    None
}

pub fn group_contacts(req: &GroupContactsRequest) -> GroupContactsOutput {
    let known_lids: HashSet<&str> = req.known_lids.iter().map(String::as_str).collect();
    let self_phone = phone_from_jid(&req.me_jid);
    let mut entries: Vec<(String, String, String)> = req
        .contacts
        .iter()
        .filter(|(key, name)| {
            !name.is_empty()
                && key.chars().all(|ch| ch.is_ascii_digit())
                && (8..=13).contains(&key.chars().count())
                && !known_lids.contains(key.as_str())
                && **key != self_phone
        })
        .map(|(key, name)| {
            (
                format!("{key}@s.whatsapp.net"),
                name.clone(),
                format_phone(key),
            )
        })
        .collect();
    entries.sort_by_key(|entry| entry.1.to_lowercase());

    let needle = req.query.trim().to_lowercase();
    let digits = digits_only(&req.query);
    let selected: HashSet<&str> = req.selected.iter().map(String::as_str).collect();
    let rows: Vec<&(String, String, String)> = entries
        .iter()
        .filter(|(_, name, phone)| {
            needle.is_empty()
                || name.to_lowercase().contains(&needle)
                || (digits.chars().count() >= 3 && digits_only(phone).contains(&digits))
        })
        .collect();

    let html = if rows.is_empty() {
        r#"<p class="new-chat-empty">No contacts match.</p>"#.to_string()
    } else {
        rows.iter()
            .map(|(jid, name, phone)| {
                format!(
                    r#"
      <label class="group-contact">
        <input type="checkbox" data-group-jid="{}" {} />
        {}
        <span class="group-contact-meta"><strong>{}</strong><span>{}</span></span>
      </label>"#,
                    escape(jid),
                    if selected.contains(jid.as_str()) {
                        "checked"
                    } else {
                        ""
                    },
                    avatar(name, jid, "avatar", &req.photos),
                    escape(name),
                    escape(phone),
                )
            })
            .collect()
    };

    GroupContactsOutput {
        empty: rows.is_empty(),
        queue: rows
            .iter()
            .take(30)
            .map(|(jid, _, _)| jid.clone())
            .collect(),
        html,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn request() -> PickerRequest {
        PickerRequest {
            contacts: HashMap::new(),
            chats: Vec::new(),
            phones: HashMap::new(),
            known_lids: Vec::new(),
            photos: HashMap::new(),
            me_jid: "62111@s.whatsapp.net".into(),
            me_lid: String::new(),
            query: String::new(),
            forward: false,
            forward_targets: Vec::new(),
        }
    }

    #[test]
    fn the_forward_toast_counts_messages() {
        assert_eq!(forward_toast(1), "Message forwarded");
        assert_eq!(forward_toast(3), "3 messages forwarded");
        assert_eq!(forward_toast(0), "0 messages forwarded");
    }

    #[test]
    fn the_group_dialog_labels_follow_its_mode() {
        let create = group_modal("create");
        assert_eq!(create.title, "New group");
        assert_eq!(create.submit_label, "Create");
        assert!(create.show_subject);
        assert_eq!(create.focus, "group-subject");
        assert_eq!(create.success_toast, "Group created");

        let add = group_modal("add");
        assert_eq!(add.title, "Add members");
        assert_eq!(add.submit_label, "Add");
        assert!(!add.show_subject, "adding members keeps the existing name");
        assert_eq!(add.focus, "group-search");
        assert_eq!(add.success_toast, "Members added");

        // An unknown mode behaves like create, never like add.
        assert!(group_modal("").show_subject);
    }

    #[test]
    fn the_group_form_asks_for_a_name_and_a_member() {
        assert_eq!(
            group_form_error("create", "  ", 2).as_deref(),
            Some("Give the group a name.")
        );
        assert_eq!(
            group_form_error("create", "Team", 0).as_deref(),
            Some("Pick at least one participant.")
        );
        assert!(group_form_error("create", "Team", 1).is_none());
        // Adding members needs no subject.
        assert!(group_form_error("add", "", 1).is_none());
        assert_eq!(
            group_form_error("add", "", 0).as_deref(),
            Some("Pick at least one participant.")
        );
        // The name is checked before the member count.
        assert_eq!(
            group_form_error("create", "", 0).as_deref(),
            Some("Give the group a name.")
        );
    }

    #[test]
    fn self_contacts_and_chats_are_listed_once() {
        let mut req = request();
        req.contacts.insert("628770000000".into(), "Budi".into());
        req.contacts.insert("628550000000".into(), "Ada".into());
        req.chats = vec![
            PickerChat {
                jid: "628550000000@s.whatsapp.net".into(),
                name: "Ada".into(),
                is_group: false,
            },
            PickerChat {
                jid: "120363@g.us".into(),
                name: "Team".into(),
                is_group: true,
            },
        ];
        let out = render(&req);
        assert!(!out.actions_hidden);
        assert!(out.html.contains("Message yourself"));
        assert_eq!(out.html.matches(">Ada<").count(), 1, "{}", out.html);
        assert!(out.html.contains("Budi"));
        assert!(out.html.contains("+62 877-0000-000"), "{}", out.html);
        assert!(
            !out.html.contains("Team"),
            "groups are not a new-chat target"
        );
        assert!(out.html.contains("new-chat-letter"));
    }

    #[test]
    fn searching_by_name_or_number() {
        let mut req = request();
        req.contacts.insert("628770000000".into(), "Budi".into());
        req.query = "bud".into();
        let out = render(&req);
        assert!(out.actions_hidden);
        assert!(out.html.contains("Budi"));
        assert!(!out.html.contains("Message yourself"));

        req.query = "8770".into();
        let out = render(&req);
        assert!(out.html.contains("Budi"));
    }

    #[test]
    fn a_typed_number_gets_its_own_row() {
        let mut req = request();
        req.query = "+62 812 3456 7890".into();
        let out = render(&req);
        assert!(
            out.html
                .contains("data-chat-jid=\"6281234567890@s.whatsapp.net\"")
        );
        assert!(out.html.contains("Start a chat with this number"));
        assert!(out.html.contains("+62 812-3456-7890"));
    }

    #[test]
    fn forward_mode_includes_groups_and_ticks() {
        let mut req = request();
        req.forward = true;
        req.forward_targets = vec!["120363@g.us".into()];
        req.chats = vec![PickerChat {
            jid: "120363@g.us".into(),
            name: "Team".into(),
            is_group: true,
        }];
        let out = render(&req);
        assert!(out.html.contains("data-forward-jid=\"120363@g.us\""));
        assert!(out.html.contains("forward-row selected"));
        assert!(out.html.contains("forward-tick"));
        assert!(out.html.contains(">Group<"));
    }

    #[test]
    fn known_lids_do_not_duplicate_contacts() {
        let mut req = request();
        req.known_lids = vec!["628770000000".into()];
        req.contacts.insert("628770000000".into(), "Budi".into());
        let out = render(&req);
        assert!(!out.html.contains("Budi"));
    }

    #[test]
    fn group_contact_checklist_filters_and_ticks() {
        let mut req = GroupContactsRequest {
            contacts: HashMap::new(),
            known_lids: Vec::new(),
            photos: HashMap::new(),
            me_jid: "62111@s.whatsapp.net".into(),
            query: String::new(),
            selected: vec!["628770000000@s.whatsapp.net".into()],
        };
        req.contacts.insert("628770000000".into(), "Budi".into());
        req.contacts.insert("628550000000".into(), "Ada".into());
        req.contacts.insert("62111".into(), "Me".into());
        req.contacts.insert("123".into(), "Short".into());
        let out = group_contacts(&req);
        assert!(!out.empty);
        assert!(
            out.html
                .contains("data-group-jid=\"628770000000@s.whatsapp.net\" checked")
        );
        assert!(out.html.contains(">Ada<"));
        assert!(!out.html.contains("Me"));
        assert!(!out.html.contains("Short"));
        assert!(out.html.contains("+62 855-0000-000"));
        assert_eq!(out.queue.len(), 2);

        req.query = "ada".into();
        let out = group_contacts(&req);
        assert!(out.html.contains(">Ada<"));
        assert!(!out.html.contains(">Budi<"));

        req.query = "zzz".into();
        let out = group_contacts(&req);
        assert!(out.empty);
        assert!(out.html.contains("No contacts match."));
    }
}
