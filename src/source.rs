/// Program text split into lines, addressable by 1-based line number.
#[derive(Debug, Clone)]
pub struct Source {
    lines: Vec<String>,
}

impl Source {
    /// Splits `text` on line breaks; a UTF-8 byte-order mark at the very start is dropped. Text that ends in a newline has a final empty line, so
    /// `"a\n"` has two lines, `"a"` and `""`.
    pub fn new(text: &str) -> Source {
        let lines = text
            .strip_prefix('\u{FEFF}')
            .unwrap_or(text)
            .split('\n')
            .map(|l| l.strip_suffix('\r').unwrap_or(l).to_string())
            .collect();
        Source { lines }
    }

    /// A copy in which every line of a statement that continues over several lines reads as
    /// the whole statement: its lines trimmed and joined with single spaces. `spans` are the
    /// first and last line of each such statement. Diagnostics echo these lines.
    pub fn with_statements(&self, spans: &[(u32, u32)]) -> Source {
        let mut lines = self.lines.clone();
        for &(first, last) in spans {
            let (from, to) = (first as usize - 1, last as usize);
            let pieces: Vec<&str> = self.lines[from..to]
                .iter()
                .map(|l| l.trim())
                .filter(|l| !l.is_empty())
                .collect();
            let whole = pieces.join(" ");
            lines[from..to].fill(whole);
        }
        Source { lines }
    }

    /// The 1-based line `n`, or `""` when out of range.
    pub fn line(&self, n: u32) -> &str {
        n.checked_sub(1)
            .and_then(|i| self.lines.get(i as usize))
            .map_or("", String::as_str)
    }

    /// The number of lines, counting the empty one after a final newline.
    pub fn line_count(&self) -> u32 {
        self.lines.len() as u32
    }
}

#[cfg(test)]
mod tests {
    use super::Source;

    #[test]
    fn source_handles_crlf_and_out_of_range() {
        let src = Source::new("one\r\ntwo\r\n");
        assert_eq!(src.line(1), "one");
        assert_eq!(src.line(2), "two");
        assert_eq!(src.line(99), "");
    }

    #[test]
    fn a_byte_order_mark_at_the_start_is_dropped() {
        assert_eq!(
            Source::new("\u{FEFF}feature start\n").line(1),
            "feature start"
        );
        assert_eq!(Source::new("a\u{FEFF}b").line(1), "a\u{FEFF}b");
        assert!(crate::check_source("\u{FEFF}feature start\n  show 1\n").is_empty());
    }

    #[test]
    fn a_continued_statement_reads_as_one_line() {
        let src = Source::new("a,\n  b ,\n\n  c\nd\n");
        let joined = src.with_statements(&[(1, 4)]);
        assert_eq!((joined.line(1), joined.line(4)), ("a, b , c", "a, b , c"));
        assert_eq!(joined.line(5), "d");
        assert_eq!(src.line(1), "a,");
    }

    #[test]
    fn a_final_newline_leaves_an_empty_last_line() {
        let src = Source::new("a\n");
        assert_eq!((src.line_count(), src.line(2)), (2, ""));
        assert_eq!(Source::new("a").line_count(), 1);
    }
}
