//! Property test: formatting generated programs re-parses, and formatting the output
//! again changes nothing. The generator is deterministic, so a failure is repeatable.

use gloss::{format_source, parse_source, source::Source};

/// A small deterministic pseudo-random source (xorshift).
struct Rng(u64);

impl Rng {
    fn below(&mut self, n: usize) -> usize {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        (self.0 % n as u64) as usize
    }

    fn pick<'a>(&mut self, options: &[&'a str]) -> &'a str {
        options[self.below(options.len())]
    }
}

const CONDITIONS: [&str; 6] = [
    "d is yes",
    "not (d is yes)",
    "not d is no",
    "t is greater than 1 and d",
    "(d is no) or e",
    "t is at least 2",
];

const SIMPLE: [&str; 9] = [
    "set  t to 1",
    "set t to t + 1",
    "set t to 3 * t",
    "set d to yes",
    "show t",
    "add 1 to t",
    "note a note, with a comma",
    "show \"a, b\", \"say \"\"hi\"\"\", t",
    "show (d is yes), (t + 1)",
];

fn long_show(rng: &mut Rng) -> String {
    let items: Vec<String> = (0..10 + rng.below(30))
        .map(|i| match rng.below(3) {
            0 => format!("\"item, {i}\""),
            1 => format!("n{i}"),
            _ => format!("\"q\"\"{i}\""),
        })
        .collect();
    format!("show {}", items.join(", "))
}

/// Statement lines at `level` (two spaces each), blocks nested up to `depth` deep.
fn statements(rng: &mut Rng, level: usize, depth: usize, out: &mut Vec<String>) {
    let pad = "  ".repeat(level);
    for _ in 0..1 + rng.below(3) {
        if rng.below(5) == 0 {
            out.push(String::new());
        }
        let kind = if depth == 0 { 0 } else { rng.below(6) };
        match kind {
            0..=2 => out.push(format!("{pad}{}", rng.pick(&SIMPLE))),
            3 => out.push(format!("{pad}{}", long_show(rng))),
            _ => {
                let head = match kind {
                    4 => format!("if {}", rng.pick(&CONDITIONS)),
                    _ if rng.below(2) == 0 => "for each i from 1 to 3".to_string(),
                    _ => "repeat while t is less than 3".to_string(),
                };
                out.push(format!("{pad}{head}"));
                statements(rng, level + 1, depth - 1, out);
                if head.starts_with("if") && rng.below(2) == 0 {
                    out.push(format!("{pad}otherwise"));
                    statements(rng, level + 1, depth - 1, out);
                }
            }
        }
    }
}

fn program(rng: &mut Rng) -> String {
    let mut lines = vec!["note top of the file".to_string()];
    lines.push("record E".into());
    lines.push("  a as text".into());
    if rng.below(2) == 0 {
        lines.push(String::new());
    }
    lines.push("  note about b".into());
    lines.push("  b as number".into());
    lines.push(String::new());
    lines.push("feature f".into());
    if rng.below(2) == 0 {
        lines.push("  note before needs".into());
    }
    lines.push("  needs t as number, d as truth, e as truth".into());
    statements(rng, 1, 2, &mut lines);
    lines.push(String::new());
    lines.push("feature start".into());
    statements(rng, 1, 2, &mut lines);
    lines.push("test \"a title\"".into());
    statements(rng, 1, 1, &mut lines);
    lines.join("\n") + "\n"
}

/// For each logical line (a wrapped statement counts once): whether a blank line comes
/// right before it, and whether it is a note. Blank lines at the top of a block and at the
/// top level are not kept by the formatter, so they are not recorded; every other blank
/// line, and every note, must come out where it went in.
fn layout(text: &str) -> Vec<(bool, bool)> {
    let mut logical: Vec<(bool, String)> = Vec::new();
    let (mut blank, mut joining) = (false, false);
    for line in text.lines() {
        if line.trim().is_empty() {
            blank = true;
        } else if joining {
            logical.last_mut().unwrap().1.push_str(line.trim());
        } else {
            logical.push((blank, line.to_string()));
        }
        joining = !line.trim().is_empty() && line.ends_with(',');
        blank = blank && line.trim().is_empty();
    }
    let opens = |l: &str| {
        let word = l.trim().split(' ').next().unwrap();
        [
            "feature",
            "if",
            "for",
            "repeat",
            "otherwise",
            "test",
            "record",
        ]
        .contains(&word)
    };
    let mut result = Vec::new();
    let mut previous_opens = true;
    for (blank, line) in logical {
        let kept = blank && !previous_opens && line.starts_with(' ');
        result.push((kept, line.trim().starts_with("note ")));
        previous_opens = opens(&line);
    }
    result
}

#[test]
fn formatting_generated_programs_is_stable() {
    let mut rng = Rng(0x9E37_79B9_7F4A_7C15);
    for n in 0..300 {
        let src = program(&mut rng);
        let out = format_source(&src).unwrap_or_else(|d| panic!("#{n}: {d:?}\n{src}"));
        assert!(
            parse_source(&Source::new(&out)).is_ok(),
            "#{n} re-parse\n{out}"
        );
        assert_eq!(
            layout(&out),
            layout(&src),
            "#{n} blank lines or notes moved\n{src}\n{out}"
        );
        let again = format_source(&out).unwrap();
        assert_eq!(again, out, "#{n} not idempotent\n{src}");
    }
}
