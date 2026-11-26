//! The listing `gloss explain` prints: each feature's inferred names and its code.

use crate::check::{FeatureInfo, TypeInfo};
use crate::lower::{FunctionNotes, Notes};
use crate::source::Source;
use crate::target::listing::{Item, Routine};

/// The listing for a program: a header, then each feature's names and code. `texts` is for
/// the runtime section, which comes later.
pub(crate) fn listing(
    name: &str,
    src: &Source,
    types: &TypeInfo,
    notes: &Notes,
    routines: &[Routine],
    _texts: &[Vec<u8>],
) -> String {
    let mut out = format!("Explain {name} for x86-64 Linux\n\n");
    for routine in routines {
        let Some(fnotes) = notes.functions.get(&routine.name) else {
            continue;
        };
        out.push_str(&format!(
            "feature {} (line {})\n",
            fnotes.feature, fnotes.line
        ));
        if let Some(info) = types.features.get(&fnotes.feature) {
            names(&mut out, info, fnotes);
        }
        out.push('\n');
        items(&mut out, src, routine);
        out.push('\n');
    }
    out
}

/// The slot of a named variable, as the code writes it.
fn slot_of(notes: &FunctionNotes, name: &str) -> String {
    let slot = notes
        .slots
        .iter()
        .find(|(_, n)| *n == name)
        .map(|(k, _)| *k)
        .unwrap_or_else(|| panic!("`{name}` has a slot in feature {}", notes.feature));
    format!("[rbp-{}]", 8 * slot)
}

fn names(out: &mut String, info: &FeatureInfo, notes: &FunctionNotes) {
    let count = info.inputs.len();
    let mut seen: Vec<&str> = Vec::new();
    for (k, (name, ty)) in info.inputs.iter().enumerate() {
        seen.push(name);
        out.push_str(&format!(
            "  {name}: {}, input {}, at [rbp+{}]\n",
            ty.describe(),
            k + 1,
            16 + 8 * (count - k - 1)
        ));
    }
    for (name, ty, line) in &info.vars {
        if !seen.contains(&name.as_str()) {
            seen.push(name);
            out.push_str(&format!(
                "  {name}: {}, created on line {line}, at {}\n",
                ty.describe(),
                slot_of(notes, name)
            ));
        }
    }
    for var in &info.loops {
        if !seen.contains(&var.name.as_str()) {
            seen.push(&var.name);
            out.push_str(&format!(
                "  {}: {}, loop variable on line {}, at {}\n",
                var.name,
                var.ty.describe(),
                var.line,
                slot_of(notes, &var.name)
            ));
        }
    }
}

fn items(out: &mut String, src: &Source, routine: &Routine) {
    let mut current: Option<u32> = None;
    let mut printed: Vec<u32> = Vec::new();
    for item in &routine.items {
        match item {
            Item::Line(n) => {
                if current != Some(*n) {
                    let again = if printed.contains(n) {
                        " (continued)"
                    } else {
                        ""
                    };
                    out.push_str(&format!("line {n}: {}{again}\n", src.line(*n).trim()));
                    printed.push(*n);
                    current = Some(*n);
                }
            }
            Item::Label { text, .. } => out.push_str(&format!("  {text}:\n")),
            Item::Inst { text, .. } => out.push_str(&format!("    {text}\n")),
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::check::check;
    use crate::lower::lower_explained;
    use crate::source::Source;

    const PROGRAM: &str = "feature total_of\n  needs prices as list of number\n  set total to 0\n  \
         for each price in prices\n    add price to total\n  give total\n\n\
         feature start\n  show (total_of with (list of 1, 2, 3))\n";

    fn explain(text: &str) -> String {
        let src = Source::new(text);
        let program = crate::parse_source(&src).expect("parses");
        let types = check(&program, &src).types.expect("checks clean");
        let (module, notes) = lower_explained(&program, &types, &src).expect("lowers");
        let routines = crate::target::x86_64_linux()
            .listing(&module)
            .expect("lists");
        super::listing("x.gloss", &src, &types, &notes, &routines, &[])
    }

    #[test]
    fn a_feature_starts_with_its_inferred_names() {
        let out = explain(PROGRAM);
        let want = "Explain x.gloss for x86-64 Linux\n\nfeature total_of (line 1)\n  \
            prices: list of number, input 1, at [rbp+16]\n  \
            total: number, created on line 3, at [rbp-8]\n  \
            price: number, loop variable on line 4, at [rbp-16]\n\n";
        assert!(out.starts_with(want), "{out}");
    }

    #[test]
    fn inputs_are_addressed_from_the_last() {
        let out = explain(
            "feature sum3\n  needs a as number, b as number, c as number\n  \
             give a + b + c\n\nfeature start\n  show (sum3 with 1, 2, 3)\n",
        );
        let want = "feature sum3 (line 1)\n  a: number, input 1, at [rbp+32]\n  \
                    b: number, input 2, at [rbp+24]\n  c: number, input 3, at [rbp+16]\n\n";
        assert!(out.contains(want), "{out}");
        // The instructions use the same forms.
        for at in ["[rbp+32]", "[rbp+24]", "[rbp+16]"] {
            assert!(out.contains(&format!("    mov rax, {at}\n")), "{out}");
        }
    }

    #[test]
    fn labels_and_instructions_are_indented() {
        let out = explain(PROGRAM);
        let label = out.lines().position(|l| l == "  .L0:").expect("label");
        let lines: Vec<&str> = out.lines().skip(label - 1).take(3).collect();
        assert_eq!(
            lines,
            ["    mov [rbp-32], rax", "  .L0:", "    mov rax, [rbp-32]"],
            "{out}"
        );
    }

    #[test]
    fn two_loops_with_one_name_list_it_once() {
        let out = explain(
            "feature start\n  for each x in (list of 1, 2)\n    show x\n  \
             for each x in (list of 3)\n    show x\n",
        );
        assert_eq!(out.matches("\n  x: ").count(), 1, "{out}");
        assert!(
            out.contains("\n  x: number, loop variable on line 2, at [rbp-"),
            "{out}"
        );
    }

    #[test]
    fn features_are_separated_by_one_blank_line_and_nothing_else_is_listed() {
        let out = explain(PROGRAM);
        assert_eq!(out.matches("\nfeature ").count(), 2, "{out}");
        assert!(out.contains("\n\nfeature start (line 8)\n"), "{out}");
        assert!(!out.contains("\n\n\n"), "{out}");
        // Only feature routines print: nothing before the first header but the title.
        assert_eq!(
            out.split("\nfeature ").next().unwrap(),
            "Explain x.gloss for x86-64 Linux\n"
        );
    }

    /// The loop line is listed again, as continued, after the body's line.
    fn assert_continued(program: &str, loop_line: &str, body_line: &str) {
        let out = explain(program);
        let plain = format!("{loop_line}\n");
        assert_eq!(out.matches(&plain).count(), 1, "{out}");
        let body = out.find(body_line).expect("body line");
        let again = out
            .find(&format!("{loop_line} (continued)\n"))
            .expect("continued");
        assert!(body < again, "{out}");
    }

    #[test]
    fn lines_repeat_as_continued() {
        let (loop_line, body) = ("line 4: for each price in prices", "line 5: add price");
        assert_continued(PROGRAM, loop_line, body);
    }

    #[test]
    fn a_counting_loop_continues_on_its_line() {
        let text =
            "feature start\n  set t to 0\n  for each i from 1 to 3\n    add i to t\n  show t\n";
        assert_continued(text, "line 3: for each i from 1 to 3", "line 4: add i to t");
    }

    #[test]
    fn a_repeat_while_continues_on_its_line() {
        let text = "feature start\n  set n to 0\n  repeat while n is less than 3\n    add 1 to n\n  show n\n";
        let loop_line = "line 3: repeat while n is less than 3";
        assert_continued(text, loop_line, "line 4: add 1 to n");
    }

    #[test]
    fn an_immediate_repeat_of_a_line_prints_nothing() {
        use crate::target::listing::{Item, Routine};
        let src = Source::new("feature a\n  show 1\n  show 2\n");
        let inst = || Item::Inst {
            text: "nop".into(),
            refs: vec![],
        };
        let items = vec![Item::Line(2), Item::Line(2), inst(), Item::Line(3), inst()];
        let mut out = String::new();
        super::items(
            &mut out,
            &src,
            &Routine {
                name: "f_a".into(),
                items,
            },
        );
        assert_eq!(out, "line 2: show 1\n    nop\nline 3: show 2\n    nop\n");
    }
}
