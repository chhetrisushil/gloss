use std::collections::HashMap;

use super::{Ctx, similar};
use crate::ast::{ItemType, Type};
use crate::diag::Diagnostic;

#[derive(Clone, Copy, PartialEq)]
enum Kind {
    Record,
    Table,
    Feature,
}

impl Kind {
    fn noun(self) -> &'static str {
        match self {
            Kind::Record => "a record",
            Kind::Table => "a table",
            Kind::Feature => "a feature",
        }
    }
}

pub fn check_program(ctx: &mut Ctx) {
    start_rules(ctx);
    program_names(ctx);
    fields(ctx);
    input_names(ctx);
    test_titles(ctx);
    table_files(ctx);
    unknown_records(ctx);
}

fn start_rules(ctx: &mut Ctx) {
    match ctx.program.features().find(|f| f.name == "start") {
        None => ctx.diags.push(Diagnostic::program(
            "This program has no feature start, so it has nowhere to begin",
        )),
        Some(f) => {
            if let Some(n) = f.needs.first() {
                let msg = "start cannot have a needs line; it is where the program begins";
                ctx.diags.push(Diagnostic::error(ctx.src, n.line, msg));
            }
        }
    }
}

fn program_names(ctx: &mut Ctx) {
    let mut names: Vec<(&str, Kind, u32)> = Vec::new();
    names.extend(
        ctx.program
            .records()
            .map(|r| (&*r.name, Kind::Record, r.line)),
    );
    names.extend(
        ctx.program
            .tables()
            .map(|t| (&*t.name, Kind::Table, t.line)),
    );
    names.extend(
        ctx.program
            .features()
            .map(|f| (&*f.name, Kind::Feature, f.line)),
    );
    names.sort_by_key(|n| n.2);
    for (i, &(name, kind, line)) in names.iter().enumerate() {
        let earlier = &names[..i];
        let problem = if let Some(&(_, k, l)) = earlier.iter().find(|n| n.0 == name) {
            format!("{name} is already used on line {l} for {}", k.noun())
        } else if let Some(&(other, ..)) = earlier
            .iter()
            .find(|&&(o, k, _)| collides(name, kind, o, k))
        {
            format!("{name} and {other} differ only in case; rename one")
        } else {
            continue;
        };
        ctx.diags.push(Diagnostic::error(ctx.src, line, problem));
    }
}

/// Equal when lowercased, except a record and a name that differs only in the first letter.
fn collides(a: &str, ka: Kind, b: &str, kb: Kind) -> bool {
    if a.to_lowercase() != b.to_lowercase() {
        return false;
    }
    let tail = |s: &str| s.chars().skip(1).collect::<String>();
    let exempt = (ka == Kind::Record) != (kb == Kind::Record) && tail(a) == tail(b);
    !exempt
}

/// An input is a name inside its feature, so it cannot reuse a feature, table or record name.
fn input_names(ctx: &mut Ctx) {
    for need in ctx.program.features().flat_map(|f| &f.needs) {
        if let Some(noun) = ctx.declared_as(&need.name) {
            let msg = format!("{} is {noun}; an input cannot reuse its name", need.name);
            ctx.diags.push(Diagnostic::error(ctx.src, need.line, msg));
        }
    }
}

fn fields(ctx: &mut Ctx) {
    for r in ctx.program.records() {
        let mut seen: HashMap<&str, u32> = HashMap::new();
        let mut by_lower: HashMap<String, &str> = HashMap::new();
        for f in r.fields() {
            let first = by_lower.get(&f.name.to_lowercase()).copied();
            if seen.insert(&f.name, f.line).is_some() {
                let msg = format!("{} already has a field named {}", r.name, f.name);
                ctx.diags.push(Diagnostic::error(ctx.src, f.line, msg));
            } else if let Some(other) = first {
                let msg = format!("{} and {other} differ only in case; rename one", f.name);
                ctx.diags.push(Diagnostic::error(ctx.src, f.line, msg));
            }
            by_lower.entry(f.name.to_lowercase()).or_insert(&f.name);
        }
    }
}

fn test_titles(ctx: &mut Ctx) {
    let mut seen: HashMap<&str, u32> = HashMap::new();
    for t in ctx.program.tests() {
        if let Some(first) = seen.get(t.title.as_str()) {
            let msg = format!("Another test already has this title (line {first})");
            ctx.diags.push(Diagnostic::error(ctx.src, t.line, msg));
        } else {
            seen.insert(&t.title, t.line);
        }
    }
}

/// Table files are fixed, relative and inside the program's folder. Every
/// rule rejects a spelling so each file has exactly one: that is what lets
/// duplicate files be found by comparing paths.
fn path_problem(path: &str) -> Option<String> {
    let mut parts = path.split('/');
    if path.starts_with('/') {
        Some(String::from(
            "The file name must not start with /; table files are found from the folder the program runs in",
        ))
    } else if path.contains('\\') {
        Some("Use / between folders, not \\".into())
    } else if parts.clone().any(|p| p == "..") {
        Some(String::from(
            "The file name must not use ..; table files stay inside the folder the program runs in",
        ))
    } else if parts.clone().any(|p| p == ".") {
        Some("The file name must not use . as a folder; write x.csv".into())
    } else if parts.clone().any(str::is_empty) {
        Some("The file name has an empty part; write folders as data/expenses.csv".into())
    } else if path.chars().any(unshowable) {
        Some(String::from(
            "The file name contains a character that cannot be shown, such as a tab or a control character",
        ))
    } else if parts
        .next_back()
        .is_some_and(|p| ends_with_ignoring_case(p, ".gloss-new"))
    {
        Some(String::from(
            "The file name must not end in .gloss-new; that ending is kept for files Gloss writes while saving",
        ))
    } else {
        later_path_problem(path)
    }
}

/// The rules added after the first seven: names that would read as a command-line option,
/// that hide a space, or that end in an ending Gloss keeps for its own files.
fn later_path_problem(path: &str) -> Option<String> {
    if path.starts_with('-') {
        return Some("The file name must not start with -".into());
    }
    if path
        .split('/')
        .any(|p| p.starts_with(' ') || p.ends_with(' '))
    {
        return Some(
            "A part of the file name starts or ends with a space; the permission list could not show it clearly".into(),
        );
    }
    let last = path.rsplit('/').next().unwrap_or_default();
    let ending = [".gloss", ".allow", ".trace", ".svg"]
        .into_iter()
        .find(|e| ends_with_ignoring_case(last, e))?;
    Some(format!(
        "The file name must not end in {ending}; that ending is kept for files Gloss reads or writes"
    ))
}

/// Case-insensitive, because a case-insensitive file system treats `x.GLOSS` as `x.gloss`.
fn ends_with_ignoring_case(name: &str, ending: &str) -> bool {
    name.to_lowercase().ends_with(ending)
}

/// Control characters, invisible characters and every space except the plain one: the
/// Default_Ignorable_Code_Points and the format (Cf) characters, written out as ranges, and
/// the Unicode space separators. Such a name would look like another file in the permission list.
fn unshowable(c: char) -> bool {
    const INVISIBLE: &[(u32, u32)] = &[
        (0x00AD, 0x00AD),
        (0x034F, 0x034F),
        (0x0600, 0x0605),
        (0x061C, 0x061C),
        (0x06DD, 0x06DD),
        (0x070F, 0x070F),
        (0x0890, 0x0891),
        (0x08E2, 0x08E2),
        (0x115F, 0x1160),
        (0x17B4, 0x17B5),
        (0x180B, 0x180F),
        (0x200B, 0x200F),
        (0x202A, 0x202E),
        (0x2060, 0x206F),
        (0x3164, 0x3164),
        (0xFE00, 0xFE0F),
        (0xFEFF, 0xFEFF),
        (0xFFA0, 0xFFA0),
        (0xFFF0, 0xFFFB),
        (0x110BD, 0x110BD),
        (0x110CD, 0x110CD),
        (0x13430, 0x1343F),
        (0x1BCA0, 0x1BCA3),
        (0x1D173, 0x1D17A),
        (0xE0000, 0xE0FFF),
    ];
    const SPACES: &[(u32, u32)] = &[
        (0x00A0, 0x00A0),
        (0x1680, 0x1680),
        (0x2000, 0x200A),
        (0x202F, 0x202F),
        (0x205F, 0x205F),
        (0x3000, 0x3000),
    ];
    let code = c as u32;
    let within = |ranges: &[(u32, u32)]| ranges.iter().any(|&(lo, hi)| (lo..=hi).contains(&code));
    c.is_control() || within(INVISIBLE) || within(SPACES)
}

fn table_files(ctx: &mut Ctx) {
    let mut files: HashMap<&str, (&str, u32)> = HashMap::new();
    let mut by_case: HashMap<String, &str> = HashMap::new();
    for t in ctx.program.tables() {
        let problem = if let Some(p) = path_problem(&t.path) {
            p
        } else if let Some(&(other, line)) = files.get(t.path.as_str()) {
            let path = &t.path;
            format!(
                "{path} is already the file of table {other} (line {line}); one file belongs to one table"
            )
        } else {
            files.insert(&t.path, (&t.name, t.line));
            if let Some(other) = by_case.insert(t.path.to_lowercase(), &t.path) {
                let msg = format!(
                    "{} and {other} differ only in case; on some systems they are the same file",
                    t.path
                );
                ctx.diags.push(Diagnostic::warning(ctx.src, t.line, msg));
            }
            continue;
        };
        ctx.diags.push(Diagnostic::error(ctx.src, t.line, problem));
    }
}

fn unknown_records(ctx: &mut Ctx) {
    let used = ctx.program.tables().map(|t| (t.line, &t.record)).chain(
        ctx.program
            .features()
            .flat_map(|f| &f.needs)
            .filter_map(|n| match &n.ty {
                Type::Item(ItemType::Record(r)) | Type::List(ItemType::Record(r)) => {
                    Some((n.line, r))
                }
                _ => None,
            }),
    );
    let known: Vec<&str> = ctx.program.records().map(|r| r.name.as_str()).collect();
    for (line, name) in used {
        if known.iter().any(|k| k == name) {
            continue;
        }
        let mut d = Diagnostic::error(ctx.src, line, format!("There is no record named {name}"));
        if let Some(s) = similar::nearest(name, known.iter().copied()) {
            d = d.suggest(s);
        }
        ctx.diags.push(d);
    }
}

#[cfg(test)]
mod tests {
    fn diags(s: &str) -> Vec<String> {
        crate::check_source(s).iter().map(|d| d.render()).collect()
    }
    const START: &str = "feature start\n  show 1\n";

    #[test]
    fn start_rules() {
        assert_eq!(
            diags("feature main\n  show 1\n"),
            ["This program has no feature start, so it has nowhere to begin."]
        );
        assert_eq!(
            diags("feature start\n  needs a as number\n  show 1\n"),
            [
                "line 2: needs a as number. start cannot have a needs line; it is where the program begins."
            ]
        );
    }

    #[test]
    fn duplicates_and_case() {
        let p = format!("{START}feature report\n  show 1\nfeature report\n  show 1\n");
        assert_eq!(
            diags(&p),
            ["line 5: feature report. report is already used on line 3 for a feature."]
        );
        let p = format!("{START}feature myTotal\n  show 1\nfeature mytotal\n  show 1\n");
        assert_eq!(
            diags(&p),
            ["line 5: feature mytotal. mytotal and myTotal differ only in case; rename one."]
        );
        let p = format!("{START}record E\n  a as text\n  a as number\n");
        assert_eq!(
            diags(&p),
            ["line 5: a as number. E already has a field named a."]
        );
        let p = format!("{START}test \"x\"\n  show 1\ntest \"x\"\n  show 1\n");
        assert_eq!(
            diags(&p),
            ["line 5: test \"x\". Another test already has this title (line 3)."]
        );
    }

    #[test]
    fn paths_differing_only_in_case_warn_on_the_second_table() {
        let two = format!(
            "{START}record E\n  a as text\ntable t of E in \"Data.csv\"\ntable u of E in \"data.csv\"\n"
        );
        assert_eq!(
            diags(&two),
            [
                "line 6: warning: table u of E in \"data.csv\". data.csv and Data.csv differ only in case; on some systems they are the same file."
            ]
        );
    }

    #[test]
    fn table_paths_are_a_security_boundary() {
        let t = |path: &str| {
            diags(&format!(
                "{START}record E\n  a as text\ntable t of E in \"{path}\"\n"
            ))
        };
        assert_eq!(
            t("/etc/passwd"),
            [
                r#"line 5: table t of E in "/etc/passwd". The file name must not start with /; table files are found from the folder the program runs in."#
            ]
        );
        assert_eq!(
            t("../x.csv"),
            [
                r#"line 5: table t of E in "../x.csv". The file name must not use ..; table files stay inside the folder the program runs in."#
            ]
        );
        assert_eq!(
            t("data/../../x.csv"),
            [
                r#"line 5: table t of E in "data/../../x.csv". The file name must not use ..; table files stay inside the folder the program runs in."#
            ]
        );
        assert_eq!(
            t("a//b.csv"),
            [
                r#"line 5: table t of E in "a//b.csv". The file name has an empty part; write folders as data/expenses.csv."#
            ]
        );
        assert_eq!(
            t(""),
            [
                r#"line 5: table t of E in "". The file name has an empty part; write folders as data/expenses.csv."#
            ]
        );
        assert_eq!(
            t("data/"),
            [
                r#"line 5: table t of E in "data/". The file name has an empty part; write folders as data/expenses.csv."#
            ]
        );
        assert_eq!(
            t(r"a\b.csv"),
            [r#"line 5: table t of E in "a\b.csv". Use / between folders, not \."#]
        );
        assert_eq!(
            t("./x.csv"),
            [
                r#"line 5: table t of E in "./x.csv". The file name must not use . as a folder; write x.csv."#
            ]
        );
        assert_eq!(
            t("-rf.csv"),
            [r#"line 5: table t of E in "-rf.csv". The file name must not start with -."#]
        );
        assert_eq!(
            t("data/ x.csv"),
            [
                r#"line 5: table t of E in "data/ x.csv". A part of the file name starts or ends with a space; the permission list could not show it clearly."#
            ]
        );
        for ending in [".gloss", ".allow", ".trace", ".svg"] {
            assert_eq!(
                t(&format!("x{ending}")),
                [format!(
                    "line 5: table t of E in \"x{ending}\". The file name must not end in {ending}; that ending is kept for files Gloss reads or writes."
                )]
            );
        }
        assert_eq!(
            t("x.GLOSS"),
            [
                r#"line 5: table t of E in "x.GLOSS". The file name must not end in .gloss; that ending is kept for files Gloss reads or writes."#
            ]
        );
        assert_eq!(t("x.Gloss-New").len(), 1);
        assert!(t("data/expenses.csv").is_empty());
        assert!(t("a..b.csv").is_empty());
        let two = format!(
            "{START}record E\n  a as text\ntable t of E in \"x.csv\"\ntable u of E in \"x.csv\"\n"
        );
        assert_eq!(
            diags(&two),
            [
                r#"line 6: table u of E in "x.csv". x.csv is already the file of table t (line 5); one file belongs to one table."#
            ]
        );
    }

    #[test]
    fn unshowable_characters_and_reserved_ending_are_refused() {
        let t = |path: &str| {
            diags(&format!(
                "{START}record E\n  a as text\ntable t of E in \"{path}\"\n"
            ))
        };
        let shown = "The file name contains a character that cannot be shown, such as a tab or a control character";
        for path in ["\0x", "\x1b[2Kx", "\u{202E}x", "x\u{200B}.csv"] {
            assert_eq!(
                t(path),
                [format!("line 5: table t of E in \"{path}\". {shown}.")],
                "{path:?}"
            );
        }
        // A tab or carriage return inside quotes is refused earlier, by the lexer.
        assert_eq!(
            t("a\tb"),
            ["line 5: table t of E in \"a\tb\". Tabs are not allowed; use spaces."]
        );
        assert_eq!(
            t("a\rb"),
            ["line 5: table t of E in \"a\rb\". A carriage return is not allowed inside text."]
        );
        let reserved = "The file name must not end in .gloss-new; that ending is kept for files Gloss writes while saving";
        assert_eq!(
            t("x.csv.gloss-new"),
            [format!(
                "line 5: table t of E in \"x.csv.gloss-new\". {reserved}."
            )]
        );
    }

    #[test]
    fn unknown_records_suggest() {
        let p = format!("{START}record Expense\n  a as text\ntable t of Expens in \"x.csv\"\n");
        assert_eq!(
            diags(&p),
            [
                r#"line 5: table t of Expens in "x.csv". There is no record named Expens. Did you mean Expense?"#
            ]
        );
    }

    #[test]
    fn collides_exemption_and_cross_kind() {
        let p = format!("{START}record Expense\n  a as text\nfeature expense\n  show 1\n");
        assert!(diags(&p).is_empty());
        let p = format!("{START}record Expense\n  a as text\nrecord EXPENSE\n  a as text\n");
        assert_eq!(
            diags(&p),
            ["line 5: record EXPENSE. EXPENSE and Expense differ only in case; rename one."]
        );
        let p = format!(
            "{START}record E\n  a as text\ntable report of E in \"x.csv\"\nfeature report\n  show 1\n"
        );
        assert_eq!(
            diags(&p),
            ["line 6: feature report. report is already used on line 5 for a table."]
        );
    }

    #[test]
    fn unknown_list_input_suggests() {
        let p = format!(
            "{START}record Expense\n  a as text\nfeature f\n  needs x as list of Expens\n  show 1\n"
        );
        assert_eq!(
            diags(&p),
            [
                "line 6: needs x as list of Expens. There is no record named Expens. Did you mean Expense?"
            ]
        );
    }
}
