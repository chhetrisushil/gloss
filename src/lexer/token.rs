use crate::number::Number;
use std::fmt;

macro_rules! keywords {
    ($($v:ident => $w:literal),* $(,)?) => {
        /// Reserved words and built-in fact names.
        #[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
        pub enum Kw { $($v),* }

        impl Kw {
            pub const ALL: [Kw; 63] = [$(Kw::$v),*];

            /// Matches lowercase words only.
            pub fn from_word(s: &str) -> Option<Kw> {
                match s { $($w => Some(Kw::$v),)* _ => None }
            }

            pub fn as_str(self) -> &'static str {
                match self { $(Kw::$v => $w),* }
            }
        }
    };
}

keywords! {
    Add => "add", And => "and", Append => "append", As => "as", Ask => "ask", At => "at", By => "by",
    Clear => "clear", Count => "count", Divide => "divide", Each => "each", Empty => "empty",
    Expect => "expect", Feature => "feature", First => "first", For => "for", From => "from",
    Give => "give", Greater => "greater", If => "if", In => "in", Input => "input",
    Into => "into", Is => "is", Item => "item", Last => "last", Least => "least",
    Less => "less", List => "list", Load => "load", More => "more", Most => "most",
    Multiply => "multiply", Needs => "needs", No => "no", Not => "not", Note => "note",
    Number => "number", Of => "of", Or => "or", Otherwise => "otherwise", Places => "places",
    Record => "record", Remainder => "remainder", Repeat => "repeat", Run => "run",
    Save => "save", Set => "set", Show => "show", Shown => "shown", Skip => "skip",
    Stop => "stop", Subtract => "subtract", Table => "table", Test => "test", Text => "text",
    Than => "than", To => "to", Truth => "truth", While => "while", Whole => "whole",
    With => "with", Yes => "yes",
}

impl Kw {
    pub fn is_fact(self) -> bool {
        matches!(
            self,
            Kw::Count | Kw::First | Kw::Last | Kw::Item | Kw::Whole | Kw::Remainder | Kw::Shown
        )
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Sym {
    Plus,
    Minus,
    Star,
    Slash,
    LParen,
    RParen,
    Comma,
}

impl Sym {
    pub fn as_str(self) -> &'static str {
        match self {
            Sym::Plus => "+",
            Sym::Minus => "-",
            Sym::Star => "*",
            Sym::Slash => "/",
            Sym::LParen => "(",
            Sym::RParen => ")",
            Sym::Comma => ",",
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub enum Tok {
    Kw(Kw),
    Name(String),
    RecordName(String),
    Number(Number),
    Text(String),
    Sym(Sym),
    Newline,
    Indent,
    Dedent,
    Note(String),
    NoteBlock(Vec<String>),
}

impl fmt::Display for Tok {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Tok::Kw(k) => f.write_str(k.as_str()),
            Tok::Name(s) | Tok::RecordName(s) => f.write_str(s),
            Tok::Number(n) => f.write_str(&n.show()),
            Tok::Text(s) => write!(f, "\"{}\"", s.replace('"', "\"\"")),
            Tok::Sym(s) => f.write_str(s.as_str()),
            Tok::Newline => f.write_str("the end of the line"),
            Tok::Indent | Tok::Dedent => f.write_str("a change of indentation"),
            Tok::Note(_) | Tok::NoteBlock(_) => f.write_str("note"),
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct Token {
    pub tok: Tok,
    pub line: u32,
    pub blank_before: bool,
    /// For a number written as one digit, such as `2` but not `2.0` or `02`, that digit.
    pub digit: Option<u8>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tokens_display_as_they_read_in_messages() {
        let shown = |t: Tok| t.to_string();
        assert_eq!(shown(Tok::Kw(Kw::Otherwise)), "otherwise");
        assert_eq!(shown(Tok::Name("total".into())), "total");
        assert_eq!(shown(Tok::RecordName("Expense".into())), "Expense");
        assert_eq!(shown(Tok::Number(Number(1_205_000))), "120.5");
        assert_eq!(shown(Tok::Text("say \"hi\"".into())), "\"say \"\"hi\"\"\"");
        assert_eq!(shown(Tok::Sym(Sym::Slash)), "/");
        assert_eq!(shown(Tok::Newline), "the end of the line");
        assert_eq!(shown(Tok::Indent), "a change of indentation");
        assert_eq!(shown(Tok::Dedent), "a change of indentation");
        assert_eq!(shown(Tok::Note("x".into())), "note");
        assert_eq!(shown(Tok::NoteBlock(vec!["x".into()])), "note");
    }

    #[test]
    fn keyword_list_matches_language_spec() {
        let spec = include_str!("../../spec/v0.md");
        let words: Vec<&str> = spec
            .lines()
            .skip_while(|l| !l.contains("Keywords can never be used as names"))
            .nth(1)
            .unwrap()
            .trim()
            .trim_matches('`')
            .split(' ')
            .collect();
        let facts: Vec<&str> = spec
            .lines()
            .skip_while(|l| !l.contains("**Built-in fact names**"))
            .nth(1)
            .unwrap()
            .trim()
            .trim_matches('`')
            .split(' ')
            .collect();
        assert_eq!(words.len() + facts.len(), Kw::ALL.len());
        for w in words.iter().chain(facts.iter()) {
            let k = Kw::from_word(w).unwrap_or_else(|| panic!("{w} missing from Kw"));
            assert_eq!(k.as_str(), *w);
            assert_eq!(k.is_fact(), facts.contains(w), "{w}");
        }
    }
}
