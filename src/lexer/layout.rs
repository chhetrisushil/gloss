use super::{Sym, Tok, Token, scan_line};
use crate::diag::Diagnostic;
use crate::source::Source;

/// Turns source text into tokens, with layout made explicit.
pub fn lex(src: &Source) -> Result<Vec<Token>, Diagnostic> {
    let mut out: Vec<Token> = Vec::new();
    let mut level = 0usize;
    let mut blank = false;
    let mut n = 1;
    let count = src.line_count();
    let tok = |tok: Tok, line: u32| Token {
        tok,
        line,
        blank_before: false,
        digit: None,
    };
    let is_blank = |k: u32| src.line(k).trim_matches(' ').is_empty();
    let lead = |k: u32| src.line(k).len() - src.line(k).trim_start_matches(' ').len();
    while n <= count {
        if is_blank(n) {
            blank = true;
            n += 1;
            continue;
        }
        let (spaces, rest) = indent(src, n, src.line(n))?;
        let new = spaces / 2;
        if new > level + 1 {
            let msg = "This line is indented more than one level deeper than the line above";
            return Err(Diagnostic::error(src, n, msg));
        }
        let pushed_indent = new == level + 1;
        if pushed_indent {
            out.push(tok(Tok::Indent, n));
        }
        for _ in new..level {
            out.push(tok(Tok::Dedent, n));
        }
        level = new;
        let trimmed = rest.trim_end_matches(' ');
        let mut last = n;
        let mut line;
        if trimmed == "note" {
            while let Some(k) = (last + 1..=count).find(|&k| !is_blank(k)) {
                if lead(k) <= spaces {
                    break;
                }
                last = k;
            }
            reject_tabs_in_notes(src, n + 1..=last)?;
            let body = (n + 1..=last)
                .map(|k| {
                    let text = src.line(k);
                    text[lead(k).min(spaces + 2)..]
                        .trim_end_matches(' ')
                        .to_string()
                })
                .collect();
            line = vec![tok(Tok::NoteBlock(body), n)];
        } else if let Some(note) = trimmed.strip_prefix("note ") {
            reject_tabs_in_notes(src, n..=n)?;
            line = vec![tok(Tok::Note(note.to_string()), n)];
        } else {
            line = scan_line(src, n, rest).map_err(|d| table_file_hint(d, rest))?;
            while let Some(&Token {
                tok: Tok::Sym(Sym::Comma),
                line: comma,
                ..
            }) = line.last()
            {
                let cont = (last + 1..=count).find(|&k| !is_blank(k));
                if let Some(k) = cont {
                    indent(src, k, src.line(k))?;
                }
                let Some(k) = cont.filter(|&k| lead(k) == spaces + 2) else {
                    let msg = "This line ends with a comma, so the next line must continue it, indented one level deeper";
                    return Err(Diagnostic::error(src, comma, msg));
                };
                line.extend(scan_line(src, k, src.line(k).trim_start_matches(' '))?);
                last = k;
            }
        }
        if !pushed_indent && !out.is_empty() {
            line[0].blank_before = blank;
        }
        blank = false;
        out.extend(line);
        out.push(tok(Tok::Newline, last));
        n = last + 1;
    }
    for _ in 0..level {
        out.push(tok(Tok::Dedent, count));
    }
    Ok(out)
}

/// A tab anywhere in a comment is rejected, like a tab anywhere else.
fn reject_tabs_in_notes(
    src: &Source,
    lines: std::ops::RangeInclusive<u32>,
) -> Result<(), Diagnostic> {
    match lines.into_iter().find(|&k| src.line(k).contains('\t')) {
        Some(k) => Err(Diagnostic::error(
            src,
            k,
            "Tabs are not allowed; use spaces",
        )),
        None => Ok(()),
    }
}

/// A table line whose file name is not in quotes stops the scanner at the first odd
/// character; answer with the quotes rule and the line as it should be written.
fn table_file_hint(error: Diagnostic, line: &str) -> Diagnostic {
    let unquoted = error.problem.ends_with("is not part of Gloss");
    let parts = line
        .strip_prefix("table ")
        .and_then(|t| t.split_once(" in "));
    // The scanner stops at the first odd character, so its first occurrence is the culprit.
    let culprit = error.problem.chars().next().and_then(|c| line.find(c));
    match parts {
        Some((head, file))
            if unquoted
                && !file.trim().is_empty()
                && !file.contains(['"', '\''])
                && culprit.is_some_and(|at| at >= "table ".len() + head.len() + " in ".len()) =>
        {
            let fixed = format!("table {} in \"{}\"", head.trim(), file.trim());
            Diagnostic {
                problem: "The file name is written in quotes".into(),
                ..error
            }
            .suggest(fixed)
        }
        _ => error,
    }
}

/// Splits a line into its indentation width and the text after it.
fn indent<'a>(src: &Source, n: u32, text: &'a str) -> Result<(usize, &'a str), Diagnostic> {
    let rest = text.trim_start_matches(' ');
    let spaces = text.len() - rest.len();
    if rest.starts_with('\t') {
        return Err(Diagnostic::error(
            src,
            n,
            "Tabs are not allowed; indent with two spaces",
        ));
    }
    if spaces % 2 != 0 {
        return Err(Diagnostic::error(
            src,
            n,
            "Indentation must be a multiple of two spaces",
        ));
    }
    Ok((spaces, rest))
}

#[cfg(test)]
mod tests {
    use crate::lexer::{Kw, Sym, Tok, lex};
    use crate::source::Source;

    fn kinds(s: &str) -> Vec<Tok> {
        lex(&Source::new(s))
            .unwrap()
            .into_iter()
            .map(|t| t.tok)
            .collect()
    }
    fn problem(s: &str) -> (Option<u32>, String) {
        let d = lex(&Source::new(s)).unwrap_err();
        (d.line, d.problem)
    }

    #[test]
    fn indent_and_dedent() {
        assert_eq!(
            kinds("feature start\n  show \"hi\"\n"),
            vec![
                Tok::Kw(Kw::Feature),
                Tok::Name("start".into()),
                Tok::Newline,
                Tok::Indent,
                Tok::Kw(Kw::Show),
                Tok::Text("hi".into()),
                Tok::Newline,
                Tok::Dedent,
            ]
        );
        let t = kinds("feature a\n  if x\n    stop\nfeature b\n  stop\n");
        assert_eq!(t.iter().filter(|t| **t == Tok::Dedent).count(), 3);
        assert_eq!(t.iter().filter(|t| **t == Tok::Indent).count(), 3);
    }

    #[test]
    fn blank_lines_are_flagged_not_emitted() {
        let toks = lex(&Source::new("feature a\n  stop\n\n\n  skip\n")).unwrap();
        let skip = toks.iter().find(|t| t.tok == Tok::Kw(Kw::Skip)).unwrap();
        assert!(skip.blank_before);
        let stop = toks.iter().find(|t| t.tok == Tok::Kw(Kw::Stop)).unwrap();
        assert!(!stop.blank_before);
    }

    #[test]
    fn notes() {
        assert_eq!(
            kinds("note smallest  program\n"),
            vec![Tok::Note("smallest  program".into()), Tok::Newline]
        );
        assert_eq!(
            kinds("note\n  Walks the list.\n\n    Indented more.\nfeature x\n"),
            vec![
                Tok::NoteBlock(vec![
                    "Walks the list.".into(),
                    "".into(),
                    "  Indented more.".into()
                ]),
                Tok::Newline,
                Tok::Kw(Kw::Feature),
                Tok::Name("x".into()),
                Tok::Newline,
            ]
        );
        assert_eq!(kinds("notes\n")[0], Tok::Name("notes".into()));
    }

    #[test]
    fn continuation_after_comma() {
        let t = kinds("feature a\n  show \"a\",\n    \"b\"\n");
        assert_eq!(
            &t[3..],
            &[
                Tok::Indent,
                Tok::Kw(Kw::Show),
                Tok::Text("a".into()),
                Tok::Sym(Sym::Comma),
                Tok::Text("b".into()),
                Tok::Newline,
                Tok::Dedent,
            ]
        );
    }

    #[test]
    fn layout_errors() {
        assert_eq!(
            problem("feature a\n\tshow 1\n"),
            (
                Some(2),
                "Tabs are not allowed; indent with two spaces".into()
            )
        );
        assert_eq!(
            problem("feature a\n   show 1\n"),
            (
                Some(2),
                "Indentation must be a multiple of two spaces".into()
            )
        );
        assert_eq!(
            problem("feature a\n    show 1\n"),
            (
                Some(2),
                "This line is indented more than one level deeper than the line above".into()
            )
        );
        assert_eq!(problem("feature a\n  show 1,\n  2\n"), (Some(2), "This line ends with a comma, so the next line must continue it, indented one level deeper".into()));
        assert_eq!(problem("feature a\n  show 1,\n"), (Some(2), "This line ends with a comma, so the next line must continue it, indented one level deeper".into()));
    }

    fn num(n: i64) -> Tok {
        Tok::Number(crate::number::Number(n * 10_000))
    }

    #[test]
    fn continuation_can_run_over_three_lines() {
        let t = kinds("feature a\n  show 1,\n    2,\n    3\n");
        assert_eq!(
            &t[4..],
            &[
                Tok::Kw(Kw::Show),
                num(1),
                Tok::Sym(Sym::Comma),
                num(2),
                Tok::Sym(Sym::Comma),
                num(3),
                Tok::Newline,
                Tok::Dedent,
            ]
        );
    }

    #[test]
    fn note_block_at_end_of_file() {
        assert_eq!(
            &kinds("feature a\n  note\n    why\n")[3..],
            &[
                Tok::Indent,
                Tok::NoteBlock(vec!["why".into()]),
                Tok::Newline,
                Tok::Dedent,
            ]
        );
    }

    #[test]
    fn note_block_before_a_dedent() {
        let t = kinds("feature a\n  stop\n  note\n    why\nfeature b\n  stop\n");
        let at = t
            .iter()
            .position(|t| matches!(t, Tok::NoteBlock(_)))
            .unwrap();
        assert_eq!(t[at], Tok::NoteBlock(vec!["why".into()]));
        assert_eq!(
            &t[at + 1..at + 4],
            &[Tok::Newline, Tok::Dedent, Tok::Kw(Kw::Feature)]
        );
    }

    #[test]
    fn a_blank_line_right_after_an_indent_is_not_flagged() {
        let toks = lex(&Source::new("feature a\n\n  stop\n")).unwrap();
        let stop = toks.iter().find(|t| t.tok == Tok::Kw(Kw::Stop)).unwrap();
        assert!(!stop.blank_before);
    }

    #[test]
    fn blank_lines_at_end_of_file_add_nothing() {
        let plain = kinds("feature a\n  stop\n");
        assert_eq!(kinds("feature a\n  stop\n\n\n  \n"), plain);
    }

    #[test]
    fn continuation_lines_get_the_indentation_errors() {
        assert_eq!(
            problem("feature a\n  show 1,\n     2\n"),
            (
                Some(3),
                "Indentation must be a multiple of two spaces".into()
            )
        );
        assert_eq!(
            problem("feature a\n  show 1,\n    \t2\n"),
            (
                Some(3),
                "Tabs are not allowed; indent with two spaces".into()
            )
        );
    }

    #[test]
    fn tabs_in_notes_are_rejected() {
        let tabs = (Some(1), "Tabs are not allowed; use spaces".to_string());
        assert_eq!(problem("note a\tb\n"), tabs);
        assert_eq!(
            problem("feature a\n  note\n    one\n    t\two\n  stop\n"),
            (Some(4), "Tabs are not allowed; use spaces".into())
        );
    }

    #[test]
    fn unquoted_table_file_names_are_answered_with_the_quotes_rule() {
        let d = lex(&Source::new("table t of E in data.csv\n")).unwrap_err();
        assert_eq!(
            d.render(),
            "line 1: table t of E in data.csv. The file name is written in quotes. Did you mean table t of E in \"data.csv\"?"
        );
        assert_eq!(problem("set x to 1 # 2\n").1, "# is not part of Gloss");
    }

    #[test]
    fn the_file_name_hint_is_only_given_when_it_is_right() {
        let hash = lex(&Source::new("table t# of E in data.csv\n")).unwrap_err();
        assert_eq!(hash.problem, "# is not part of Gloss");
        assert_eq!(hash.suggestion, None);
        let quoted = lex(&Source::new("table t of E in 'data.csv'\n")).unwrap_err();
        assert_eq!(quoted.problem, "' is not part of Gloss");
        assert_eq!(quoted.suggestion, None);
    }
}
