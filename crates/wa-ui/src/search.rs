//! In-chat search planning: which rendered bubbles match a query, and the
//! match counter label. The webview still walks text nodes to paint the ranges.

use serde::Serialize;

/// Indices of `texts` that contain `query` (case-insensitive substring).
pub fn matched_indices(texts: &[String], query: &str) -> Vec<usize> {
    let needle = query.trim().to_lowercase();
    if needle.is_empty() {
        return Vec::new();
    }
    texts
        .iter()
        .enumerate()
        .filter(|(_, text)| text.to_lowercase().contains(&needle))
        .map(|(index, _)| index)
        .collect()
}

#[derive(Debug, Serialize)]
pub struct SearchLabel {
    pub text: String,
    pub disabled: bool,
}

/// `"2/5"` while there are hits, `"0/0"` for a query with none, `""` when the
/// search box is empty.
pub fn count_label(index: i64, total: usize, query: &str) -> SearchLabel {
    if total == 0 {
        return SearchLabel {
            text: if query.trim().is_empty() {
                String::new()
            } else {
                "0/0".to_string()
            },
            disabled: true,
        };
    }
    let current = if index < 0 { 0 } else { index as usize + 1 };
    SearchLabel {
        text: format!("{current}/{total}"),
        disabled: false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn matches_are_case_insensitive() {
        let texts = vec![
            "Hello there".to_string(),
            "nothing".to_string(),
            "say HELLO".to_string(),
        ];
        assert_eq!(matched_indices(&texts, "hello"), vec![0, 2]);
        assert_eq!(matched_indices(&texts, "  HeLLo "), vec![0, 2]);
        assert!(matched_indices(&texts, "").is_empty());
        assert!(matched_indices(&texts, "zzz").is_empty());
    }

    #[test]
    fn label_counts_hits() {
        assert_eq!(count_label(0, 4, "x").text, "1/4");
        assert_eq!(count_label(3, 4, "x").text, "4/4");
        assert_eq!(count_label(-1, 4, "x").text, "0/4");
        assert!(!count_label(0, 4, "x").disabled);

        let none = count_label(-1, 0, "x");
        assert_eq!(none.text, "0/0");
        assert!(none.disabled);

        let empty = count_label(-1, 0, "   ");
        assert_eq!(empty.text, "");
        assert!(empty.disabled);
    }
}
