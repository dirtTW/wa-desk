//! Open Graph link previews: a fallback for links the client sends itself or
//! older messages that carry no embedded WhatsApp preview.

use std::sync::Mutex;

use serde::Serialize;

/// Open Graph metadata for a web link, as shown by WhatsApp link previews.
#[derive(Debug, Clone, Serialize)]
pub struct LinkPreview {
    url: String,
    title: String,
    description: String,
    site: String,
    image: Option<String>,
}

fn preview_cache() -> &'static Mutex<std::collections::HashMap<String, LinkPreview>> {
    static CACHE: std::sync::OnceLock<Mutex<std::collections::HashMap<String, LinkPreview>>> =
        std::sync::OnceLock::new();
    CACHE.get_or_init(|| Mutex::new(std::collections::HashMap::new()))
}

#[tauri::command]
pub async fn link_preview(url: String) -> Result<LinkPreview, String> {
    let url = url.trim().to_owned();
    let lower = url.to_ascii_lowercase();
    if !(lower.starts_with("http://") || lower.starts_with("https://")) {
        return Err("only web links can be previewed".into());
    }
    let host = lower
        .split("://")
        .nth(1)
        .unwrap_or("")
        .split('/')
        .next()
        .unwrap_or("");
    if host.is_empty()
        || host.starts_with("localhost")
        || host.starts_with("127.")
        || host.starts_with("10.")
        || host.starts_with("192.168.")
        || host.starts_with("[::1]")
    {
        return Err("private links are not previewed".into());
    }

    if let Some(hit) = preview_cache()
        .lock()
        .expect("preview cache")
        .get(&url)
        .cloned()
    {
        return Ok(hit);
    }

    let fetch_url = url.clone();
    let preview = tokio::task::spawn_blocking(move || fetch_link_preview(&fetch_url))
        .await
        .map_err(|e| e.to_string())??;

    let mut cache = preview_cache().lock().expect("preview cache");
    if cache.len() >= 200 {
        cache.clear();
    }
    cache.insert(url, preview.clone());
    Ok(preview)
}

fn fetch_link_preview(url: &str) -> Result<LinkPreview, String> {
    let agent = ureq::Agent::config_builder()
        .timeout_global(Some(std::time::Duration::from_secs(9)))
        .max_redirects(6)
        .user_agent(
            "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 \
             (KHTML, like Gecko) Chrome/126.0 Safari/537.36",
        )
        .build()
        .new_agent();

    let mut response = agent
        .get(url)
        .header("Accept", "text/html,application/xhtml+xml")
        .header("Accept-Language", "en-US,en;q=0.8")
        .call()
        .map_err(|e| e.to_string())?;

    let content_type = response
        .headers()
        .get("content-type")
        .and_then(|value| value.to_str().ok())
        .unwrap_or("")
        .to_ascii_lowercase();
    if !content_type.is_empty() && !content_type.contains("text/html") {
        return Err("not an HTML page".into());
    }

    let body = response
        .body_mut()
        .with_config()
        .limit(700_000)
        .read_to_string()
        .map_err(|e| e.to_string())?;

    let title = meta_content(&body, &["og:title", "twitter:title"])
        .or_else(|| html_title(&body))
        .unwrap_or_default();
    let description = meta_content(
        &body,
        &["og:description", "twitter:description", "description"],
    )
    .unwrap_or_default();
    let site = meta_content(&body, &["og:site_name"]).unwrap_or_else(|| host_of(url));
    let image = meta_content(
        &body,
        &[
            "og:image",
            "og:image:url",
            "twitter:image",
            "twitter:image:src",
        ],
    )
    .map(|image| resolve_url(url, &image));

    if title.is_empty() && image.is_none() {
        return Err("no preview metadata".into());
    }

    Ok(LinkPreview {
        url: url.to_owned(),
        title: title.chars().take(180).collect(),
        description: description.chars().take(260).collect(),
        site,
        image,
    })
}

fn meta_content(html: &str, keys: &[&str]) -> Option<String> {
    let lower = html.to_ascii_lowercase();
    let mut start = 0;
    while let Some(rel) = lower[start..].find("<meta") {
        let begin = start + rel;
        let Some(rel_end) = lower[begin..].find('>') else {
            break;
        };
        let end = (begin + rel_end + 1).min(html.len());
        let tag = &html[begin..end];
        let lower_tag = &lower[begin..end];

        let key =
            attr_value(tag, lower_tag, "property").or_else(|| attr_value(tag, lower_tag, "name"));
        if let Some(key) = key {
            let key = key.to_ascii_lowercase();
            if keys.contains(&key.as_str())
                && let Some(content) = attr_value(tag, lower_tag, "content")
            {
                let content = decode_entities(content.trim());
                if !content.is_empty() {
                    return Some(content);
                }
            }
        }
        start = end;
    }
    None
}

fn attr_value(tag: &str, lower_tag: &str, attr: &str) -> Option<String> {
    let needle = format!("{attr}=");
    let mut from = 0;
    while let Some(rel) = lower_tag[from..].find(&needle) {
        let at = from + rel;
        let value_at = at + needle.len();
        let before_ok = at == 0 || lower_tag.as_bytes()[at - 1].is_ascii_whitespace();
        if before_ok
            && let Some(quote @ (b'"' | b'\'')) = lower_tag.as_bytes().get(value_at).copied()
        {
            let value_start = value_at + 1;
            if let Some(len) = tag[value_start..].find(quote as char) {
                return Some(tag[value_start..value_start + len].to_owned());
            }
            return None;
        }
        from = value_at;
    }
    None
}

fn html_title(html: &str) -> Option<String> {
    let lower = html.to_ascii_lowercase();
    let start = lower.find("<title")?;
    let open = start + lower[start..].find('>')? + 1;
    let close = open + lower[open..].find("</title>")?;
    let title = decode_entities(html[open..close].trim());
    (!title.is_empty()).then_some(title)
}

fn decode_entities(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut rest = text;
    while let Some(pos) = rest.find('&') {
        out.push_str(&rest[..pos]);
        let tail = &rest[pos..];
        let Some(semi) = tail.find(';') else {
            out.push_str(tail);
            return out;
        };
        let entity = &tail[1..semi];
        let decoded = match entity {
            "amp" => Some('&'),
            "lt" => Some('<'),
            "gt" => Some('>'),
            "quot" => Some('"'),
            "apos" | "#39" => Some('\''),
            "nbsp" => Some(' '),
            _ => None,
        };
        if let Some(ch) = decoded {
            out.push(ch);
        } else if let Some(code) = entity
            .strip_prefix("#x")
            .or_else(|| entity.strip_prefix("#X"))
            .and_then(|hex| u32::from_str_radix(hex, 16).ok())
            .or_else(|| entity.strip_prefix('#').and_then(|dec| dec.parse().ok()))
            .and_then(char::from_u32)
        {
            out.push(code);
        } else {
            out.push_str(&tail[..=semi]);
        }
        rest = &tail[semi + 1..];
    }
    out.push_str(rest);
    out
}

fn host_of(url: &str) -> String {
    url.split("://")
        .nth(1)
        .unwrap_or(url)
        .split('/')
        .next()
        .unwrap_or("")
        .to_owned()
}

fn resolve_url(base: &str, image: &str) -> String {
    if image.starts_with("http://") || image.starts_with("https://") {
        return image.to_owned();
    }
    let scheme = if base.starts_with("https://") {
        "https"
    } else {
        "http"
    };
    if let Some(rest) = image.strip_prefix("//") {
        return format!("{scheme}://{rest}");
    }
    let host = host_of(base);
    if image.starts_with('/') {
        return format!("{scheme}://{host}{image}");
    }
    let dir = base.rsplit_once('/').map(|(dir, _)| dir).unwrap_or(base);
    format!("{dir}/{image}")
}
