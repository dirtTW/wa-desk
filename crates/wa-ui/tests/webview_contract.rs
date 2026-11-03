//! Contract tests: the exact JSON the webview sends (`sidebarRequest()` in
//! `apps/desktop/ui/chat-list.js`) must deserialize and render. These catch
//! field-name drift between JavaScript and Rust, which is the failure mode a
//! type checker would otherwise catch for us.

use wa_ui::screens::{StarredEntry, channels, community, starred};
use wa_ui::sidebar::SidebarRequest;

const REQUEST: &str = r#"{
  "chats": [
    {
      "jid": "6281234567890@s.whatsapp.net",
      "name": "Ada",
      "is_group": false,
      "unread": 2,
      "pinned": true,
      "pin_ts": 5,
      "deleted": false,
      "archived": false,
      "cleared": false,
      "mute_until": 0,
      "mute_forever": false,
      "last_ts": 1770000000000,
      "last": {
        "id": "m1",
        "text": "hi @628770000000",
        "kind": "text",
        "from_me": false,
        "status": "read",
        "sender": "6281234567890@s.whatsapp.net",
        "sender_name": "Ada",
        "sender_phone": "",
        "deleted": false,
        "deleted_for_me": false,
        "cleared": false,
        "timestamp": 1770000000000,
        "document": null,
        "mentions": ["628770000000@s.whatsapp.net"],
        "reactions": [{"emoji": "x", "by": "me"}],
        "link": {"title": "a link", "url": "https://a.test", "thumb": "data:image/jpeg;base64,AAAA"}
      }
    },
    {
      "jid": "6281234567890-1@lid",
      "name": "Ada",
      "is_group": false,
      "unread": 0,
      "pinned": false,
      "pin_ts": 0,
      "deleted": false,
      "archived": true,
      "cleared": false,
      "mute_until": 1770000600000,
      "mute_forever": false,
      "last_ts": 0,
      "last": null
    },
    {
      "jid": "123@newsletter",
      "name": "News",
      "is_group": false,
      "unread": 0,
      "pinned": false,
      "pin_ts": 0,
      "deleted": false,
      "archived": false,
      "cleared": false,
      "mute_until": 0,
      "mute_forever": false,
      "last_ts": 1770000000000,
      "last": {"id": "p1", "text": "post", "kind": "image", "from_me": false, "status": "", "sender": "", "sender_name": "", "sender_phone": "", "deleted": false, "deleted_for_me": false, "cleared": false, "timestamp": 1770000000000}
    }
  ],
  "contacts": {"628770000000": "Budi"},
  "photos": {"6281234567890@s.whatsapp.net": "https://pps.whatsapp.net/x.jpg"},
  "group_meta": {},
  "phones": {},
  "sender_names": {},
  "me": "Me",
  "me_jid": "621110000000@s.whatsapp.net",
  "me_lid": "",
  "self_handle": "@me",
  "active": "6281234567890@s.whatsapp.net",
  "search": "",
  "filter": "all",
  "show_deleted": false,
  "selecting": false,
  "selected": [],
  "archived_open": false,
  "max_search_rows": 40,
  "now": 1770000000000
}"#;

fn request() -> SidebarRequest {
    serde_json::from_str(REQUEST).expect("the webview's sidebar request must deserialize")
}

#[test]
fn sidebar_contract_from_the_webview() {
    let out = wa_ui::sidebar::render(&request());
    assert!(
        out.html
            .contains(r#"data-jid="6281234567890@s.whatsapp.net""#)
    );
    assert!(out.html.contains("chat-pin"), "pinned chat keeps its glyph");
    assert!(out.html.contains(r#"<span class="badge">2</span>"#));
    assert!(out.html.contains("avatar-img"), "a cached photo is used");
    assert!(
        out.html.contains("hi @Budi"),
        "mentions resolve to names: {out_html}",
        out_html = out.html
    );
    assert!(
        out.html.contains("archived-row"),
        "the archived row is offered"
    );
    assert!(
        !out.html.contains("123@newsletter"),
        "channels stay out of the list"
    );
}

#[test]
fn search_contract_from_the_webview() {
    let mut req = request();
    req.search = "budi".to_string();
    let out = wa_ui::sidebar::render(&req);
    assert!(
        out.html
            .contains(r#"class="search-section">Contacts</div>"#)
    );
    assert!(
        out.html
            .contains(r#"data-picker-jid="628770000000@s.whatsapp.net""#)
    );
}

#[test]
fn channels_and_community_contract_from_the_webview() {
    let req = request();
    let out = channels(&req, "");
    assert!(out.html.contains("123@newsletter"));
    assert!(out.html.contains("channel-kind"));

    assert!(community(&req, "1@g.us").html.is_empty());

    let starred_entries: Vec<StarredEntry> = serde_json::from_str(
        r#"[{"jid":"6281234567890@s.whatsapp.net","message":{"id":"m1","text":"hi","kind":"text","timestamp":1770000000000,"from_me":false}}]"#,
    )
    .expect("starred entries deserialize");
    let out = starred(&starred_entries, &req);
    assert!(out.html.contains("data-starred-id=\"m1\""));
    assert!(out.html.contains("hi"));
}
