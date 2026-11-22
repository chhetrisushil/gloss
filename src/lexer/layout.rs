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
        let mut last = n;
        let mut line = scan_line(src, n, rest)?;
        while let Some(&Token {
            tok: Tok::Sym(Sym::Comma),
            line: comma,
            ..
        }) = line.last()
        {
            let cont = (last + 1..=count).find(|&k| !is_blank(k));
            let Some(k) = cont.filter(|&k| lead(k) == spaces + 2) else {
                let msg = "This line ends with a comma, so the next line must continue it, indented one level deeper";
                return Err(Diagnostic::error(src, comma, msg));
            };
            line.extend(scan_line(src, k, src.line(k).trim_start_matches(' '))?);
            last = k;
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
}
