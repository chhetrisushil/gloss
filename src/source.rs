/// Program text split into lines, addressable by 1-based line number.
#[derive(Debug, Clone)]
pub struct Source {
    lines: Vec<String>,
}

impl Source {
    pub fn new(text: &str) -> Source {
        let lines = text
            .split('\n')
            .map(|l| l.strip_suffix('\r').unwrap_or(l).to_string())
            .collect();
        Source { lines }
    }

    /// The 1-based line `n`, or `""` when out of range.
    pub fn line(&self, n: u32) -> &str {
        n.checked_sub(1)
            .and_then(|i| self.lines.get(i as usize))
            .map_or("", String::as_str)
    }

    pub fn line_count(&self) -> u32 {
        self.lines.len() as u32
    }
}
