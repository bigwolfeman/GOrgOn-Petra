//! The filtered list as a pure function, not an engine mechanism.
//!
//! Binding: spec 009 T009. Combobox and Command call this from their own
//! `Compound::update`. It is `(items, query) -> Vec<usize>`, total, and
//! testable with no daemon.

/// Indices of `items` whose haystack contains `query`.
///
/// Matching is a case-insensitive substring: both the query and each
/// haystack are compared after Unicode lowercase. An empty query returns
/// every index in order and does not call `haystack`.
///
/// The function is total. It always returns a `Vec`, never errors, and
/// allocates only that vec plus the lowercased query and, on a non-empty
/// query, one lowercased haystack at a time.
#[must_use]
pub fn filter_indices<T>(items: &[T], query: &str, haystack: impl Fn(&T) -> &str) -> Vec<usize> {
    if query.is_empty() {
        return (0..items.len()).collect();
    }
    let needle = query.to_lowercase();
    items
        .iter()
        .enumerate()
        .filter_map(|(i, item)| haystack(item).to_lowercase().contains(&needle).then_some(i))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::filter_indices;

    #[test]
    fn empty_query_returns_every_index_in_order() {
        let items = ["alpha", "bravo", "charlie"];
        assert_eq!(filter_indices(&items, "", |s| *s), vec![0, 1, 2]);
    }

    #[test]
    fn empty_query_does_not_call_haystack() {
        let items = ["a", "b"];
        let out = filter_indices(&items, "", |_| panic!("haystack must not run"));
        assert_eq!(out, vec![0, 1]);
    }

    #[test]
    fn empty_items_returns_empty() {
        let items: [&str; 0] = [];
        assert!(filter_indices(&items, "x", |s| *s).is_empty());
        assert!(filter_indices(&items, "", |s| *s).is_empty());
    }

    #[test]
    fn substring_match_is_case_insensitive() {
        let items = ["Alpha", "bravo", "CHARLIE", "palette"];
        assert_eq!(filter_indices(&items, "al", |s| *s), vec![0, 3]);
        assert_eq!(filter_indices(&items, "AL", |s| *s), vec![0, 3]);
        assert_eq!(filter_indices(&items, "ChAr", |s| *s), vec![2]);
    }

    #[test]
    fn no_match_returns_empty() {
        let items = ["alpha", "bravo"];
        assert!(filter_indices(&items, "zzz", |s| *s).is_empty());
    }

    #[test]
    fn order_is_the_input_order() {
        let items = ["z-keep", "a-drop", "m-keep"];
        assert_eq!(filter_indices(&items, "keep", |s| *s), vec![0, 2]);
    }

    #[test]
    fn haystack_selects_the_compared_field() {
        struct Row {
            name: String,
            hidden: String,
        }
        let items = [
            Row {
                name: "alpha".into(),
                hidden: "needle".into(),
            },
            Row {
                name: "bravo".into(),
                hidden: "other".into(),
            },
        ];
        assert_eq!(
            filter_indices(&items, "needle", |r| r.name.as_str()),
            vec![]
        );
        assert_eq!(
            filter_indices(&items, "needle", |r| r.hidden.as_str()),
            vec![0]
        );
        assert_eq!(filter_indices(&items, "br", |r| r.name.as_str()), vec![1]);
    }

    #[test]
    fn query_longer_than_haystack_does_not_match() {
        let items = ["ab"];
        assert!(filter_indices(&items, "abc", |s| *s).is_empty());
    }

    #[test]
    fn unicode_lowercase_is_the_fold() {
        let items = ["café", "CAFE"];
        assert_eq!(filter_indices(&items, "É", |s| *s), vec![0]);
        assert_eq!(filter_indices(&items, "cafe", |s| *s), vec![1]);
    }

    #[test]
    fn whitespace_query_is_not_empty() {
        let items = ["a b", "ab"];
        assert_eq!(filter_indices(&items, " ", |s| *s), vec![0]);
    }
}
