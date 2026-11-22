/// Levenshtein edit distance between two names.
pub fn distance(a: &str, b: &str) -> usize {
    let b: Vec<char> = b.chars().collect();
    let mut row: Vec<usize> = (0..=b.len()).collect();
    for (i, ca) in a.chars().enumerate() {
        let mut diag = row[0];
        row[0] = i + 1;
        for (j, cb) in b.iter().enumerate() {
            let up = row[j + 1];
            row[j + 1] = (diag + usize::from(ca != *cb)).min(up + 1).min(row[j] + 1);
            diag = up;
        }
    }
    row[b.len()]
}

/// The candidate closest to `name` within distance 2: smallest distance wins,
/// then source order. Names under 3 characters get no suggestion.
pub fn nearest<'a>(name: &str, candidates: impl Iterator<Item = &'a str>) -> Option<&'a str> {
    if name.len() < 3 {
        return None;
    }
    let mut best: Option<(usize, &'a str)> = None;
    for c in candidates {
        let d = distance(name, c);
        if c != name && d <= 2 && best.is_none_or(|(bd, _)| d < bd) {
            best = Some((d, c));
        }
    }
    best.map(|(_, c)| c)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn distance_basics() {
        assert_eq!(distance("", ""), 0);
        assert_eq!(distance("abc", "abc"), 0);
        assert_eq!(distance("abc", ""), 3);
        assert_eq!(distance("kitten", "sitting"), 3);
        assert_eq!(distance("Expens", "Expense"), 1);
    }

    #[test]
    fn nearest_prefers_smaller_distance_then_source_order() {
        assert_eq!(
            nearest("total", ["totl", "total2", "tota"].into_iter()),
            Some("totl")
        );
        assert_eq!(
            nearest("total", ["totalxx", "totals"].into_iter()),
            Some("totals")
        );
        assert_eq!(nearest("abcd", ["abce", "abcf"].into_iter()), Some("abce"));
        assert_eq!(nearest("total", ["unrelated"].into_iter()), None);
        assert_eq!(nearest("total", ["total"].into_iter()), None);
    }

    #[test]
    fn short_names_get_no_suggestion() {
        assert_eq!(nearest("ab", ["ab", "ac"].into_iter()), None);
    }
}
