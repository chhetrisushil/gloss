const WIDTH: usize = 100;

/// `text` as lines at `level`. A line over the width breaks after commas that
/// sit outside text literals; each continuation is one level deeper.
pub fn wrap(level: usize, text: &str) -> Vec<String> {
    let mut lines = Vec::new();
    let pieces = pieces(text);
    let mut current = format!("{}{}", "  ".repeat(level), pieces[0]);
    for piece in &pieces[1..] {
        if current.chars().count() + 1 + piece.chars().count() <= WIDTH {
            current = format!("{current} {piece}");
        } else {
            lines.push(current);
            current = format!("{}{piece}", "  ".repeat(level + 1));
        }
    }
    lines.push(current);
    lines
}

/// The text cut after each comma that is outside a text literal.
fn pieces(text: &str) -> Vec<&str> {
    let (mut out, mut start, mut in_text) = (Vec::new(), 0, false);
    for (i, c) in text.char_indices() {
        match c {
            '"' => in_text = !in_text,
            ',' if !in_text => {
                out.push(text[start..=i].trim_start());
                start = i + 1;
            }
            _ => {}
        }
    }
    out.push(text[start..].trim_start());
    out
}
