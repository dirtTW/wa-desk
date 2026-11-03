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

/// Normalise a requested hit index: wrap into `0..total`, and `-1` when there
/// are no hits (the viewer has nothing to point at).
pub fn hit_index(total: usize, requested: i64) -> i64 {
    if total == 0 {
        return -1;
    }
    let total = total as i64;
    ((requested % total) + total) % total
}

/// Keep the current hit after a re-run of the query: the list may have shrunk.
pub fn keep_hit(total: usize, current: i64) -> i64 {
    if total == 0 || current < 0 {
        return if total == 0 { -1 } else { 0 };
    }
    current.min(total as i64 - 1)
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
    fn hit_indices_wrap_and_survive_a_shorter_list() {
        // Stepping past either end wraps, like the arrows in the search bar.
        assert_eq!(hit_index(5, 0), 0);
        assert_eq!(hit_index(5, 4), 4);
        assert_eq!(hit_index(5, 5), 0);
        assert_eq!(hit_index(5, -1), 4);
        assert_eq!(hit_index(5, -6), 4);
        assert_eq!(hit_index(5, 12), 2);
        // No hits at all.
        assert_eq!(hit_index(0, 0), -1);
        assert_eq!(hit_index(0, 3), -1);

        // Keeping the position after the query changed.
        assert_eq!(keep_hit(3, 2), 2);
        assert_eq!(keep_hit(2, 2), 1, "the list shrank");
        assert_eq!(keep_hit(4, -1), 0, "nothing was selected yet");
        assert_eq!(keep_hit(0, 3), -1, "no hits left");
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
