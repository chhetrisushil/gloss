use std::collections::HashMap;

/// Names that can be asked which of them are exactly one edit away from another name,
/// without comparing every pair: two names one edit apart share the name itself or a
/// version of it with one character removed, so only names sharing such a key are compared.
#[derive(Default)]
pub struct Neighbours<'a> {
    names: Vec<&'a str>,
    keys: HashMap<String, Vec<usize>>,
}

/// The name and each way to remove one character from it.
fn keys_of(name: &str) -> Vec<String> {
    let chars: Vec<char> = name.chars().collect();
    let without = |skip: usize| {
        let kept = chars.iter().enumerate().filter(|(i, _)| *i != skip);
        kept.map(|(_, c)| *c).collect::<String>()
    };
    std::iter::once(name.to_string())
        .chain((0..chars.len()).map(without))
        .collect()
}

impl<'a> Neighbours<'a> {
    pub fn add(&mut self, name: &'a str) {
        for key in keys_of(name) {
            self.keys.entry(key).or_default().push(self.names.len());
        }
        self.names.push(name);
    }

    /// The added names at edit distance exactly 1 from `name`, in the order they were added.
    pub fn within_one(&self, name: &str) -> Vec<&'a str> {
        let mut found: Vec<usize> = keys_of(name)
            .iter()
            .filter_map(|key| self.keys.get(key))
            .flatten()
            .copied()
            .collect();
        found.sort_unstable();
        found.dedup();
        let near = found.into_iter().map(|i| self.names[i]);
        near.filter(|other| distance(name, other) == 1).collect()
    }
}

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
    fn neighbours_finds_exactly_the_names_one_edit_away() {
        let all = ["total", "totl", "totals", "tutal", "other", "total"];
        let mut index = Neighbours::default();
        all.iter().for_each(|n| index.add(n));
        for name in ["total", "totl", "tota", "xotal", "ab", ""] {
            let slow: Vec<&str> = all
                .iter()
                .copied()
                .filter(|o| distance(name, o) == 1)
                .collect();
            assert_eq!(index.within_one(name), slow, "{name}");
        }
    }

    #[test]
    fn short_names_get_no_suggestion() {
        assert_eq!(nearest("ab", ["ab", "ac"].into_iter()), None);
    }
}
