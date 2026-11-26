//! The listing `gloss explain` prints: each feature's inferred names and its code.

mod comment;

use comment::comment;

use crate::check::{FeatureInfo, TypeInfo};
use crate::lower::{FunctionNotes, Notes, spell_invisible};
use crate::source::Source;
use crate::target::listing::{Item, Routine};

/// The column the `;` of a comment starts in, counting from 0.
const COLUMN: usize = 36;

/// The listing for a program: a header, each feature's names and code, then every other
/// routine under `runtime`, in layout order. `texts` are the program's texts, for the comments
/// that show them.
pub(crate) fn listing(
    name: &str,
    src: &Source,
    types: &TypeInfo,
    notes: &Notes,
    routines: &[Routine],
    texts: &[Vec<u8>],
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
        items(&mut out, src, routine, Some(fnotes), texts);
        out.push('\n');
    }
    out.push_str("runtime\n\n");
    let rest: Vec<&Routine> = routines
        .iter()
        .filter(|r| !notes.functions.contains_key(&r.name))
        .collect();
    for (k, routine) in rest.iter().enumerate() {
        if k > 0 {
            out.push('\n');
        }
        out.push_str(&format!("routine {}\n", routine.name));
        items(&mut out, src, routine, None, texts);
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
    // A loop variable is listed once per (name, type): two loops may share a name and a slot
    // but differ in type, and then each type is named.
    let mut loops: Vec<(&str, String)> = Vec::new();
    for var in &info.loops {
        let pair = (var.name.as_str(), var.ty.describe());
        if !seen.contains(&pair.0) && !loops.contains(&pair) {
            loops.push(pair);
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

/// The items in the order they print: labels directly followed by a `Line` print after that
/// line's header, so a jump to the label lands under the line it belongs to. A label encodes
/// nothing, so the order is equivalent (as built, M6 final review).
/// A failure label (`is_fail`) is never moved: its `failure` header belongs right before it.
fn in_print_order(items: &[Item], is_fail: impl Fn(u32) -> bool) -> Vec<&Item> {
    let mut out = Vec::with_capacity(items.len());
    let mut labels: Vec<&Item> = Vec::new();
    for item in items {
        match item {
            Item::Label { local: Some(n), .. } if is_fail(*n) => {
                out.append(&mut labels);
                out.push(item);
            }
            Item::Label { .. } => labels.push(item),
            Item::Line(_) => {
                out.push(item);
                out.append(&mut labels);
            }
            Item::Inst { .. } => {
                out.append(&mut labels);
                out.push(item);
            }
        }
    }
    out.append(&mut labels);
    out
}

/// A source line as a header shows it: trimmed, with invisible characters spelled as the
/// diagnostics and the comments spell them.
fn header(src: &Source, n: u32) -> String {
    spell_invisible(src.line(n).trim())
}

fn items(
    out: &mut String,
    src: &Source,
    routine: &Routine,
    notes: Option<&FunctionNotes>,
    texts: &[Vec<u8>],
) {
    let mut current: Option<u32> = None;
    let mut printed: Vec<u32> = Vec::new();
    // A feature's entry code, before its first line, belongs to the feature's own line.
    let first = routine
        .items
        .iter()
        .find(|i| !matches!(i, Item::Label { .. }));
    if let (Some(n), Some(Item::Inst { .. })) = (notes.map(|n| n.line), first) {
        out.push_str(&format!("line {n}: {}\n", header(src, n)));
        printed.push(n);
        current = Some(n);
    }
    let is_fail = |n: u32| notes.is_some_and(|f| f.fails.contains_key(&n));
    for item in in_print_order(&routine.items, is_fail) {
        match item {
            Item::Line(n) => {
                if current != Some(*n) {
                    let again = if printed.contains(n) {
                        " (continued)"
                    } else {
                        ""
                    };
                    out.push_str(&format!("line {n}: {}{again}\n", header(src, *n)));
                    printed.push(*n);
                    current = Some(*n);
                }
            }
            Item::Label { text, local } => {
                let fail = local.and_then(|n| notes?.fails.get(&n));
                if let Some(f) = fail {
                    out.push_str(&format!("failure {}, line {}\n", f.code, f.line));
                }
                out.push_str(&format!("  {text}:\n"));
            }
            Item::Inst { text, refs } => {
                out.push_str(&format!("    {text}"));
                if let Some(c) = comment(refs, notes, texts) {
                    let pad = COLUMN.saturating_sub(4 + text.chars().count()).max(1);
                    out.push_str(&format!("{:pad$}; {c}", ""));
                }
                out.push('\n');
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::check::check;
    use crate::lower::lower_explained;
    use crate::source::Source;
    use crate::target::listing::{Item, Ref, Routine};

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
        super::listing("x.gloss", &src, &types, &notes, &routines, &module.texts)
    }

    #[test]
    fn a_features_entry_code_lists_under_its_own_line() {
        let out = explain("feature start\n  show 1\n");
        let want = "line 1: feature start\n    push rbp\n    mov rbp, rsp\n    sub rsp, 8\n    \
            mov [rbp-8], rcx\nline 2: show 1\n";
        assert!(out.contains(want), "{out}");
    }

    #[test]
    fn a_features_implicit_return_lists_as_the_feature_continued() {
        let out = explain("feature start\n  show 1\n");
        let want = "    add rsp, 8\nline 1: feature start (continued)\n    mov rcx, [rbp-8]\n    \
            mov rsp, rbp\n    pop rbp\n    ret\n";
        assert!(out.contains(want), "{out}");
    }

    #[test]
    fn a_features_end_guard_lists_as_the_feature_continued() {
        let out = explain(PROGRAM);
        let want = "    ret\nline 1: feature total_of (continued)\n    call flush\n";
        assert!(out.contains(want), "{out}");
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
        for (at, name) in [("[rbp+32]", "a"), ("[rbp+24]", "b"), ("[rbp+16]", "c")] {
            let line = out
                .lines()
                .find(|l| l.starts_with(&format!("    mov rax, {at}")));
            assert!(
                line.is_some_and(|l| l.ends_with(&format!("; {name}"))),
                "{out}"
            );
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
    fn two_loops_with_one_name_and_two_types_list_both() {
        let out = explain(
            "feature start\n  for each x in (list of 1, 2)\n    show x\n  \
             for each x in (list of \"a\", \"b\")\n    show x\n",
        );
        assert_eq!(out.matches("\n  x: ").count(), 2, "{out}");
        let want = "\n  x: number, loop variable on line 2, at [rbp-";
        assert!(out.contains(want), "{out}");
        let want = "\n  x: text, loop variable on line 4, at [rbp-";
        assert!(out.contains(want), "{out}");
    }

    #[test]
    fn line_headers_spell_invisible_characters_as_comments_do() {
        let out = explain("feature start\n  show \"a\u{202E}b\"\n");
        assert!(out.contains("line 2: show \"a\\u{202E}b\"\n"), "{out}");
        assert!(!out.contains('\u{202E}'), "{out}");
        // The comment on the same code spells it the same way.
        assert!(out.contains("; \"a\\u{202E}b\""), "{out}");
    }

    #[test]
    fn a_header_spells_a_tab_as_unshowable_does() {
        // `chars::unshowable` covers every control character, so a tab is spelled. A tab in a
        // real program is a parse error, so this checks the header helper on a bare source.
        let src = Source::new("a\tb\u{200B}\n");
        assert_eq!(super::header(&src, 1), "a\\u{0009}b\\u{200B}");
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
    fn a_repeat_while_top_label_lists_under_the_loops_line() {
        let text = "feature start\n  set n to 0\n  repeat while n is less than 3\n    add 1 to n\n  show n\n";
        let out = explain(text);
        let at = out
            .find("line 3: repeat while n is less than 3\n")
            .expect("header");
        let label = out.find("  .L0:\n").expect("top label");
        let jump = out.find("    jmp .L0").expect("back edge");
        assert!(at < label && label < jump, "{out}");
        // Nothing of line 2 sits between the header and the label.
        assert!(!out[at..label].contains("[rbp-"), "{out}");
    }

    #[test]
    fn an_ifs_end_label_lists_under_the_next_statement() {
        let text = "feature start\n  set n to 1\n  if n is 1\n    show 1\n  show 2\n";
        let out = explain(text);
        let header = out.find("line 5: show 2\n").expect("next header");
        let label = out.find("  .L0:\n").expect("end label");
        assert!(header < label, "{out}");
        assert!(out[label..].starts_with("  .L0:\n    "), "{out}");
    }

    #[test]
    fn labels_before_a_line_print_after_its_header() {
        let src = Source::new("feature a\n  show 1\n  show 2\n");
        let inst = || Item::Inst {
            text: "nop".into(),
            refs: vec![],
        };
        let label = |n: u32| Item::Label {
            text: format!(".L{n}"),
            local: Some(n),
        };
        let items = vec![
            Item::Line(2),
            inst(),
            label(1),
            label(2),
            Item::Line(3),
            inst(),
            label(3),
            Item::Line(3),
            inst(),
        ];
        let routine = Routine {
            name: "f_a".into(),
            items,
        };
        let mut out = String::new();
        super::items(&mut out, &src, &routine, None, &[]);
        let want =
            "line 2: show 1\n    nop\nline 3: show 2\n  .L1:\n  .L2:\n    nop\n  .L3:\n    nop\n";
        assert_eq!(out, want);
    }

    #[test]
    fn an_immediate_repeat_of_a_line_prints_nothing() {
        let src = Source::new("feature a\n  show 1\n  show 2\n");
        let inst = || Item::Inst {
            text: "nop".into(),
            refs: vec![],
        };
        let items = vec![Item::Line(2), Item::Line(2), inst(), Item::Line(3), inst()];
        let mut out = String::new();
        let routine = Routine {
            name: "f_a".into(),
            items,
        };
        super::items(&mut out, &src, &routine, None, &[]);
        assert_eq!(out, "line 2: show 1\n    nop\nline 3: show 2\n    nop\n");
    }

    const OVERFLOW: &str = "feature bump\n  needs price as number\n  set total to 0\n  add price to total\n  give total\n\n\
         feature start\n  show (bump with 1)\n";

    #[test]
    fn an_overflow_jump_names_its_failure() {
        let src = Source::new(OVERFLOW);
        let program = crate::parse_source(&src).expect("parses");
        let types = check(&program, &src).types.expect("checks clean");
        let (_, notes) = lower_explained(&program, &types, &src).expect("lowers");
        let fails = &notes.functions["f_bump"].fails;
        let note = fails.values().find(|f| f.code == 4).expect("failure 4");
        let out = explain(OVERFLOW);
        let jo = out.lines().find(|l| l.contains(" jo ")).expect("jo line");
        let want = format!("; failure 4: {}", note.message);
        assert!(jo.ends_with(&want), "{out}");
    }

    #[test]
    fn a_text_address_shows_the_text() {
        let out = explain("feature start\n  show \"hi \"\"x\"\"\"\n");
        let lea = out.lines().find(|l| l.contains("lea ")).expect("lea line");
        assert!(lea.ends_with("; \"hi \"\"x\"\"\""), "{out}");
    }

    #[test]
    fn a_failure_block_has_a_header() {
        let out = explain(OVERFLOW);
        assert!(out.contains("\nfailure 4, line 4\n  .L0:\n"), "{out}");
    }

    #[test]
    fn the_comment_column() {
        let out = explain(OVERFLOW);
        let line = out.lines().find(|l| l.contains("; total")).expect("slot");
        assert_eq!(line.find(';'), Some(36), "{out}");
        // 31 and 32 characters of text make lines of 35 and 36 before the comment.
        for (n, pad) in [(31, 1), (32, 1), (30, 2)] {
            let got = commented(&"x".repeat(n), &[Ref::System("write")]);
            let want = format!("    {}{}; write\n", "x".repeat(n), " ".repeat(pad));
            assert_eq!(got, want);
        }
        let long = commented(&"x".repeat(40), &[Ref::System("write")]);
        assert_eq!(long, format!("    {} ; write\n", "x".repeat(40)));
    }

    fn synthetic(items: Vec<Item>) -> String {
        use crate::lower::{FailNote, FunctionNotes};
        let mut notes = FunctionNotes {
            feature: "a".into(),
            line: 1,
            slots: Default::default(),
            inputs: vec![],
            fails: Default::default(),
        };
        let note = FailNote {
            code: 4,
            line: 2,
            message: "m".into(),
        };
        notes.fails.insert(5, note);
        let src = Source::new("feature a\n  show 1\n");
        let routine = Routine {
            name: "f_a".into(),
            items,
        };
        let mut out = String::new();
        super::items(&mut out, &src, &routine, Some(&notes), &[]);
        out
    }

    #[test]
    fn a_jump_to_a_plain_label_has_no_comment() {
        let items = vec![Item::Inst {
            text: "jmp .L1".into(),
            refs: vec![Ref::Jump(1)],
        }];
        assert_eq!(synthetic(items), "line 1: feature a\n    jmp .L1\n");
    }

    #[test]
    fn a_failure_label_has_its_header_without_a_jump() {
        let label = |local| Item::Label {
            text: ".L5".into(),
            local,
        };
        assert_eq!(
            synthetic(vec![label(Some(5))]),
            "failure 4, line 2\n  .L5:\n"
        );
        assert_eq!(synthetic(vec![label(Some(6))]), "  .L5:\n");
        assert_eq!(synthetic(vec![label(None)]), "  .L5:\n");
    }

    fn commented(text: &str, refs: &[Ref]) -> String {
        let src = Source::new("feature a\n  show 1\n");
        let mut out = String::new();
        let item = Item::Inst {
            text: text.into(),
            refs: refs.to_vec(),
        };
        let routine = Routine {
            name: "f_a".into(),
            items: vec![item],
        };
        super::items(&mut out, &src, &routine, None, &[]);
        out
    }

    fn example_explanations() -> Vec<(String, String)> {
        let mut dir: Vec<_> = std::fs::read_dir("examples")
            .expect("examples")
            .map(|e| e.expect("entry").path())
            .filter(|p| p.extension().is_some_and(|e| e == "gloss"))
            .collect();
        dir.sort();
        assert!(dir.len() >= 12);
        dir.iter()
            .map(|p| {
                let text = std::fs::read_to_string(p).expect("reads");
                let (diags, checked) = crate::check_program(&text);
                let checked = checked.unwrap_or_else(|| panic!("{p:?}: {diags:?}"));
                let out = checked.explanation("x.gloss").expect("explains");
                (p.display().to_string(), out)
            })
            .collect()
    }

    #[test]
    fn every_routine_is_listed() {
        for (path, out) in example_explanations() {
            let text = std::fs::read_to_string(&path).expect("reads");
            let (_, checked) = crate::check_program(&text);
            let checked = checked.expect("checks");
            let module = crate::lower::lower(&checked.program, &checked.types, &checked.src)
                .expect("lowers");
            let want: Vec<String> = crate::target::x86_64::program::assemble(&module)
                .expect("assembles")
                .routines
                .into_iter()
                .map(|r| r.name)
                .collect();
            let got: Vec<String> = out
                .lines()
                .filter_map(|l| {
                    l.strip_prefix("routine ").map(String::from).or_else(|| {
                        l.strip_prefix("feature ")
                            .and_then(|r| r.split(" (line").next())
                            .map(|f| format!("f_{f}"))
                    })
                })
                .collect();
            // Features come first in the module, so the listing keeps the layout order.
            assert_eq!(got, want, "{path}");
        }
    }

    #[test]
    fn runtime_routines_have_no_slot_comments() {
        let names: Vec<&str> = (0..=500)
            .filter_map(crate::target::x86_64::linux::syscall_name)
            .collect();
        for (path, out) in example_explanations() {
            let (_, runtime) = out.split_once("\nruntime\n").expect("has a runtime");
            for line in runtime.lines().filter(|l| l.starts_with("    ")) {
                if let Some((_, c)) = line.split_once("; ") {
                    assert!(c.starts_with('"') || names.contains(&c), "{path}: {line}");
                }
            }
        }
    }

    /// Every `.gloss` program in `dir`, sorted, as its path and source text.
    fn programs(dir: &str) -> Vec<(String, String)> {
        let mut paths: Vec<_> = std::fs::read_dir(dir)
            .expect("folder")
            .map(|e| e.expect("entry").path())
            .filter(|p| p.extension().is_some_and(|e| e == "gloss"))
            .collect();
        paths.sort();
        assert!(!paths.is_empty(), "{dir}");
        paths
            .iter()
            .map(|p| (p.display().to_string(), std::fs::read_to_string(p).unwrap()))
            .collect()
    }

    /// An instruction line of the listing with its comment cut off: the comment starts at the
    /// first `;` in column 36 or later (instruction text has no `;`, and a quoted text in a
    /// comment may, so only the first counts).
    fn without_comment(line: &str) -> &str {
        let at = line
            .char_indices()
            .enumerate()
            .find(|(col, (_, c))| *col >= 36 && *c == ';')
            .map(|(_, (i, _))| i);
        line[..at.unwrap_or(line.len())].trim_end()
    }

    /// `.L<n>` operands written back as the assembler's `L<routine>_<n>`.
    fn own_labels(text: &str, routine: &str) -> String {
        text.split(' ')
            .map(|word| match word.strip_prefix(".L") {
                Some(n) if !n.is_empty() && n.bytes().all(|b| b.is_ascii_digit()) => {
                    format!("L{routine}_{n}")
                }
                _ => word.to_string(),
            })
            .collect::<Vec<_>>()
            .join(" ")
    }

    /// The printed listing's instruction lines, per routine, in order.
    fn printed(out: &str) -> Vec<(String, Vec<String>)> {
        let mut all: Vec<(String, Vec<String>)> = Vec::new();
        for line in out.lines() {
            let header = line.strip_prefix("routine ").map(String::from).or_else(|| {
                line.strip_prefix("feature ")
                    .and_then(|r| r.split(" (line ").next())
                    .map(|f| format!("f_{f}"))
            });
            if let Some(name) = header {
                all.push((name, Vec::new()));
            } else if line.starts_with("    ") {
                let (name, list) = all.last_mut().expect("an instruction under a header");
                list.push(own_labels(without_comment(line).trim_start(), name));
            }
        }
        all
    }

    #[test]
    fn the_printed_instructions_are_the_assembled_ones() {
        let mut dirs = programs("examples");
        dirs.extend(programs("tests/explain_cases"));
        for (path, text) in dirs {
            let (diags, checked) = crate::check_program(&text);
            let checked = checked.unwrap_or_else(|| panic!("{path}: {diags:?}"));
            let out = checked.explanation("x.gloss").expect("explains");
            let module = crate::lower::lower(&checked.program, &checked.types, &checked.src)
                .expect("lowers");
            let want: Vec<(String, Vec<String>)> =
                crate::target::x86_64::program::assemble(&module)
                    .expect("assembles")
                    .routines
                    .into_iter()
                    .map(|r| {
                        let texts = r
                            .code
                            .iter()
                            .filter(|x| {
                                !matches!(
                                    x,
                                    crate::target::x86_64::inst::X::Label(_)
                                        | crate::target::x86_64::inst::X::Comment(_)
                                )
                            })
                            .map(crate::target::x86_64::inst::text)
                            .collect();
                        (r.name, texts)
                    })
                    .collect();
            let got = printed(&out);
            let names =
                |v: &[(String, Vec<String>)]| v.iter().map(|r| r.0.clone()).collect::<Vec<_>>();
            assert_eq!(names(&got), names(&want), "{path}");
            for (g, w) in got.iter().zip(&want) {
                assert_eq!(g.1, w.1, "{path}: routine {}", g.0);
            }
        }
    }

    #[test]
    fn a_comment_is_cut_at_its_semicolon_even_when_it_quotes_one() {
        let line = "    mov [rbp-8], rax                ; \"a; b\"";
        assert_eq!(without_comment(line), "    mov [rbp-8], rax");
        let long = format!("    call {} ; x; y", "q".repeat(40));
        assert_eq!(
            without_comment(&long),
            format!("    call {}", "q".repeat(40))
        );
        assert_eq!(own_labels("jmp .L12", "f_a"), "jmp Lf_a_12");
        assert_eq!(own_labels("call x86_64.L1x", "f_a"), "call x86_64.L1x");
    }

    #[test]
    fn a_failure_label_is_not_moved_past_a_line() {
        let label = |n| Item::Label {
            text: format!(".L{n}"),
            local: Some(n),
        };
        let items = [label(1), label(2), Item::Line(3)];
        let order = super::in_print_order(&items, |n| n == 1);
        assert_eq!(order, [&items[0], &items[2], &items[1]]);
    }
}
