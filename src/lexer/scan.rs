use super::token::{Kw, Sym, Tok, Token};
use crate::diag::Diagnostic;
use crate::number::{Number, NumberError, OUT_OF_RANGE};
use crate::source::Source;

/// Scans one line of text into tokens. Indentation, notes and continuation
/// are the caller's concern; every token has `blank_before: false`.
pub fn scan_line(src: &Source, line: u32, text: &str) -> Result<Vec<Token>, Diagnostic> {
    let err = |msg: String| Diagnostic::error(src, line, msg);
    let is_word = |c: char| c.is_ascii_alphanumeric() || c == '_';
    let mut out = Vec::new();
    let mut i = 0;
    while let Some(c) = text[i..].chars().next() {
        let start = i;
        i += c.len_utf8();
        let mut digit = None;
        let tok = match c {
            ' ' => continue,
            '\t' => return Err(err("Tabs are not allowed; use spaces".into())),
            c if c.is_ascii_alphabetic() => {
                i += text[i..].find(|c| !is_word(c)).unwrap_or(text.len() - i);
                let word = &text[start..i];
                match Kw::from_word(word) {
                    Some(k) => Tok::Kw(k),
                    None if c.is_ascii_uppercase() => Tok::RecordName(word.into()),
                    None => Tok::Name(word.into()),
                }
            }
            c if c.is_ascii_digit() => {
                i += text[i..]
                    .find(|c: char| !(c.is_ascii_digit() || c == '.'))
                    .unwrap_or(text.len() - i);
                let lexeme = &text[start..i];
                // The most negative number cannot be written without its minus sign.
                let lowest = || match out.last().map(|t: &Token| &t.tok) {
                    Some(Tok::Sym(Sym::Minus)) => Number::parse_text(&format!("-{lexeme}")).ok(),
                    _ => None,
                };
                let parsed = Number::parse_literal(lexeme);
                let n = parsed.or_else(|e| lowest().ok_or(e)).map_err(|e| {
                    err(match e {
                        NumberError::Malformed => {
                            format!("{lexeme} is not a number Gloss can read")
                        }
                        NumberError::TooManyPlaces => {
                            "Numbers have at most four decimal places".into()
                        }
                        NumberError::OutOfRange => OUT_OF_RANGE.into(),
                    })
                })?;
                if text[i..].chars().next().is_some_and(is_word) {
                    return Err(err("Put a space between a number and a word".into()));
                }
                digit = lexeme.parse().ok().filter(|_| lexeme.len() == 1);
                Tok::Number(n)
            }
            '"' => {
                let mut s = String::new();
                loop {
                    match text[i..].chars().next() {
                        None => return Err(err("Text is missing its closing quote".into())),
                        Some('"') if text[i + 1..].starts_with('"') => {
                            s.push('"');
                            i += 2;
                        }
                        Some('"') => {
                            i += 1;
                            break;
                        }
                        Some('\t') => return Err(err("Tabs are not allowed; use spaces".into())),
                        Some('\r') => {
                            return Err(err("A carriage return is not allowed inside text".into()));
                        }
                        Some(ch) => {
                            s.push(ch);
                            i += ch.len_utf8();
                        }
                    }
                }
                Tok::Text(s)
            }
            '+' => Tok::Sym(Sym::Plus),
            '-' => Tok::Sym(Sym::Minus),
            '*' => Tok::Sym(Sym::Star),
            '/' => Tok::Sym(Sym::Slash),
            '(' => Tok::Sym(Sym::LParen),
            ')' => Tok::Sym(Sym::RParen),
            ',' => Tok::Sym(Sym::Comma),
            '=' | '<' | '>' => {
                let word = match c {
                    '=' => "is",
                    '<' => "is less than",
                    _ => "is greater than",
                };
                return Err(err(format!(
                    "{c} is not part of Gloss. Comparisons are written in words, such as {word}"
                )));
            }
            c => return Err(err(format!("{c} is not part of Gloss"))),
        };
        out.push(Token {
            tok,
            line,
            blank_before: false,
            digit,
        });
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn scan(text: &str) -> Vec<Tok> {
        scan_line(&Source::new(text), 1, text)
            .unwrap()
            .into_iter()
            .map(|t| t.tok)
            .collect()
    }
    fn problem(text: &str) -> String {
        scan_line(&Source::new(text), 1, text).unwrap_err().problem
    }

    #[test]
    fn words_numbers_symbols() {
        assert_eq!(
            scan("set total to total + 1"),
            vec![
                Tok::Kw(Kw::Set),
                Tok::Name("total".into()),
                Tok::Kw(Kw::To),
                Tok::Name("total".into()),
                Tok::Sym(Sym::Plus),
                Tok::Number(Number(10_000)),
            ]
        );
        assert_eq!(
            scan("Expense with item_2 (x9)"),
            vec![
                Tok::RecordName("Expense".into()),
                Tok::Kw(Kw::With),
                Tok::Name("item_2".into()),
                Tok::Sym(Sym::LParen),
                Tok::Name("x9".into()),
                Tok::Sym(Sym::RParen),
            ]
        );
        assert_eq!(scan("120.50"), vec![Tok::Number(Number(1_205_000))]);
    }

    #[test]
    fn text_with_doubled_quotes() {
        assert_eq!(
            scan(r#""say ""hi""""#),
            vec![Tok::Text(r#"say "hi""#.into())]
        );
        assert_eq!(
            scan(r#""" , "a,b""#),
            vec![
                Tok::Text(String::new()),
                Tok::Sym(Sym::Comma),
                Tok::Text("a,b".into()),
            ]
        );
    }

    #[test]
    fn errors() {
        assert_eq!(problem(r#"show "abc"#), "Text is missing its closing quote");
        assert_eq!(
            problem("if a = 3"),
            "= is not part of Gloss. Comparisons are written in words, such as is"
        );
        assert_eq!(
            problem("if a < 3"),
            "< is not part of Gloss. Comparisons are written in words, such as is less than"
        );
        assert_eq!(
            problem("set x to 0.12345"),
            "Numbers have at most four decimal places"
        );
        assert_eq!(
            problem("set x to 99999999999999999"),
            "This number is outside the range Gloss can hold, about 922 trillion either way"
        );
        assert_eq!(
            problem("set x to 2nd"),
            "Put a space between a number and a word"
        );
        assert_eq!(problem("set x to 5."), "5. is not a number Gloss can read");
        assert_eq!(problem("show\t1"), "Tabs are not allowed; use spaces");
        assert_eq!(problem("show 1 # 2"), "# is not part of Gloss");
        assert_eq!(problem("show \"a\tb\""), "Tabs are not allowed; use spaces");
        assert_eq!(
            problem("show \"a\rb\""),
            "A carriage return is not allowed inside text"
        );
    }

    #[test]
    fn non_ascii_is_fine_inside_text_only() {
        assert_eq!(scan("\"héllo\""), vec![Tok::Text("héllo".into())]);
        assert_eq!(problem("show é"), "é is not part of Gloss");
    }

    #[test]
    fn a_capitalised_keyword_is_a_record_name() {
        assert_eq!(scan("Set"), vec![Tok::RecordName("Set".into())]);
    }

    #[test]
    fn greater_than_sign_is_answered_in_words() {
        assert_eq!(
            problem("if a > 3"),
            "> is not part of Gloss. Comparisons are written in words, such as is greater than"
        );
    }
}
