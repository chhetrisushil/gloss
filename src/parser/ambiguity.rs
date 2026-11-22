//! `whole of n / 2` can be read two ways; the parser keeps going on the default reading,
//! remembers the other one, and reports both once the whole expression is known, so the
//! suggestions keep everything around the ambiguous part.

use super::Parser;
use super::expr::{number_fact_on_right_edge, op_text};
use crate::ast::print;
use crate::ast::{ArithOp, Expr, ExprId, ExprKind as K};
use crate::diag::Diagnostic;
use crate::lexer::Tok;

pub(super) struct Ambiguity {
    /// The node in the tree that holds the default reading.
    node: ExprId,
    /// What stands in its place under the other reading.
    other: Expr,
    line: u32,
    /// The fact and what follows it, as written, for the start of the message.
    short: String,
}

impl Parser<'_> {
    /// Runs one expression parse; when the outermost one finishes, an ambiguity found inside
    /// it is reported with the whole expression written out both ways.
    pub(super) fn outermost(
        &mut self,
        level: fn(&mut Self) -> Result<Expr, Diagnostic>,
    ) -> Result<Expr, Diagnostic> {
        self.scope += 1;
        let result = level(self);
        self.scope -= 1;
        if self.scope > 0 {
            return result;
        }
        match (result, self.ambiguity.take()) {
            (Ok(whole), Some(found)) => Err(self.two_ways(&found, whole)),
            (result, _) => result,
        }
    }

    /// Parses the operand after `op` and joins it to `left`. A `whole` or `remainder` fact on
    /// the right edge of `left` could take the operand too; the default reading keeps it out
    /// (the fact gets parentheses) and the other reading is remembered.
    pub(super) fn extend(
        &mut self,
        op: ArithOp,
        left: Expr,
        level: fn(&mut Self) -> Result<Expr, Diagnostic>,
    ) -> Result<Expr, Diagnostic> {
        if matches!(self.peek(), Some(Tok::Newline) | None) {
            return Err(self.err(format!("{} needs a value after it", op_text(op))));
        }
        let right = level(self)?;
        let Some(fact) = number_fact_on_right_edge(&left).cloned() else {
            return Ok(self.binary(op, left, right));
        };
        let other = self.reading_with(&left, &fact, op, &right);
        let mut default = left;
        default.edit_node(fact.id, &mut |n| {
            let inner = n.clone();
            n.kind = K::Paren(Box::new(inner));
        });
        let short = format!(
            "{} {} {}",
            print::expr(&fact),
            op_text(op),
            print::expr(&right)
        );
        let node = self.binary(op, default, right);
        if self.ambiguity.is_none() {
            let (node, line) = (node.id, fact.line);
            self.ambiguity = Some(Ambiguity {
                node,
                other,
                line,
                short,
            });
        }
        Ok(node)
    }

    /// `left` with the fact taking `op right` into its operand, in parentheses.
    fn reading_with(&mut self, left: &Expr, fact: &Expr, op: ArithOp, right: &Expr) -> Expr {
        let kind = match &fact.kind {
            K::Fact { fact: f, of } => K::Fact {
                fact: *f,
                of: Box::new(self.grouped(op, of, right)),
            },
            K::Remainder { of, by } => K::Remainder {
                of: of.clone(),
                by: Box::new(self.grouped(op, by, right)),
            },
            _ => fact.kind.clone(),
        };
        let mut other = left.clone();
        other.edit_node(fact.id, &mut |n| n.kind = kind.clone());
        other
    }

    fn grouped(&mut self, op: ArithOp, a: &Expr, b: &Expr) -> Expr {
        let binary = self.binary(op, a.clone(), b.clone());
        self.mk(a.line, K::Paren(Box::new(binary)))
    }

    fn two_ways(&self, found: &Ambiguity, whole: Expr) -> Diagnostic {
        let first = print::expr(&whole);
        let mut changed = whole;
        changed.edit_node(found.node, &mut |n| *n = found.other.clone());
        let message = format!(
            "{} can be read two ways. Write {first} or {}",
            found.short,
            print::expr(&changed)
        );
        Diagnostic::error(self.src, found.line, message)
    }
}
