mod block;
pub mod expr;
mod stmt;
mod top;
mod value;

pub use stmt::parse_stmts_str;
pub use top::parse;

use crate::ast::{Expr, ExprId, ExprKind};
use crate::diag::Diagnostic;
use crate::lexer::{Kw, Sym, Tok, Token, lex};
use crate::source::Source;

/// The deepest an expression may nest (parentheses, negation, facts of facts).
const MAX_DEPTH: u32 = 200;

pub struct Parser<'a> {
    toks: &'a [Token],
    pos: usize,
    src: &'a Source,
    next_id: u32,
    depth: u32,
}

impl<'a> Parser<'a> {
    pub fn new(toks: &'a [Token], src: &'a Source) -> Self {
        Parser {
            toks,
            pos: 0,
            src,
            next_id: 0,
            depth: 0,
        }
    }

    pub fn peek(&self) -> Option<&Tok> {
        self.peek_at(0)
    }

    pub fn peek_at(&self, ahead: usize) -> Option<&Tok> {
        self.toks.get(self.pos + ahead).map(|t| &t.tok)
    }

    /// The line of the current token, else of the last one.
    pub fn line(&self) -> u32 {
        let current = self.toks.get(self.pos).or_else(|| self.toks.last());
        current.map_or(1, |t| t.line)
    }

    pub fn bump(&mut self) -> Option<Token> {
        let token = self.toks.get(self.pos).cloned();
        if token.is_some() {
            self.pos += 1;
        }
        token
    }

    pub fn eat_kw(&mut self, k: Kw) -> bool {
        let found = self.peek() == Some(&Tok::Kw(k));
        if found {
            self.pos += 1;
        }
        found
    }

    pub fn eat_sym(&mut self, s: Sym) -> bool {
        let found = self.peek() == Some(&Tok::Sym(s));
        if found {
            self.pos += 1;
        }
        found
    }

    pub fn err(&self, problem: impl Into<String>) -> Diagnostic {
        Diagnostic::error(self.src, self.line(), problem)
    }

    /// Runs one self-recursive level of parsing, refusing to go deeper than `MAX_DEPTH`
    /// so that absurdly nested input is reported instead of overflowing the stack.
    pub fn nested<T>(
        &mut self,
        level: impl FnOnce(&mut Self) -> Result<T, Diagnostic>,
    ) -> Result<T, Diagnostic> {
        if self.depth >= MAX_DEPTH {
            return Err(self.err("This expression is nested too deeply"));
        }
        self.depth += 1;
        let result = level(self);
        self.depth -= 1;
        result
    }

    pub fn mk(&mut self, line: u32, kind: ExprKind) -> Expr {
        let id = ExprId(self.next_id);
        self.next_id += 1;
        Expr { id, line, kind }
    }
}

/// Parses one expression that fills the whole text.
pub fn parse_expr_str(text: &str) -> Result<Expr, Diagnostic> {
    let src = Source::new(text);
    let toks = lex(&src)?;
    let mut parser = Parser::new(&toks, &src);
    let expr = parser.parse_expr()?;
    match parser.peek() {
        Some(Tok::Newline) | None => Ok(expr),
        Some(other) => Err(parser.err(format!("Unexpected {other} after the expression"))),
    }
}
