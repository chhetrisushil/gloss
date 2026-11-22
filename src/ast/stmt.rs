use super::{Expr, Line};

#[derive(Clone, Debug, PartialEq)]
pub enum Note {
    Line(String),
    Block(Vec<String>),
}

#[derive(Clone, Debug, PartialEq)]
pub enum Target {
    Var(String),
    Field {
        field: String,
        of: Box<Target>,
    },
    Item {
        position: Box<Expr>,
        of: Box<Target>,
    },
}

impl Target {
    /// The variable or table name the target starts from.
    pub fn root(&self) -> &str {
        match self {
            Target::Var(name) => name,
            Target::Field { of, .. } | Target::Item { of, .. } => of.root(),
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct Stmt {
    pub line: Line,
    pub blank_before: bool,
    pub kind: StmtKind,
}

#[derive(Clone, Debug, PartialEq)]
pub enum StmtKind {
    Note(Note),
    Set {
        target: Target,
        value: Expr,
    },
    Add {
        value: Expr,
        target: Target,
    },
    Subtract {
        value: Expr,
        target: Target,
    },
    Multiply {
        target: Target,
        by: Expr,
    },
    Divide {
        target: Target,
        by: Expr,
    },
    Append {
        value: Expr,
        list: String,
    },
    Show(Vec<Expr>),
    Ask {
        question: Expr,
        into: String,
    },
    If {
        branches: Vec<(Expr, Vec<Stmt>)>,
        otherwise: Option<Vec<Stmt>>,
    },
    ForEachIn {
        var: String,
        list: Expr,
        body: Vec<Stmt>,
    },
    ForEachRange {
        var: String,
        from: Expr,
        to: Expr,
        body: Vec<Stmt>,
    },
    RepeatWhile {
        cond: Expr,
        body: Vec<Stmt>,
    },
    Skip,
    Stop,
    Give(Expr),
    Run {
        feature: String,
        inputs: Vec<Expr>,
    },
    Load(String),
    Save(String),
    Clear(String),
    Expect(Expr),
    Input(Vec<Expr>),
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ast::{ExprId, ExprKind};
    use crate::number::Number;

    fn var(name: &str) -> Target {
        Target::Var(name.into())
    }

    #[test]
    fn target_root_follows_the_chain() {
        let position = Expr {
            id: ExprId(0),
            line: 1,
            kind: ExprKind::Number(Number(30_000)),
        };
        let nested = Target::Field {
            field: "amount".into(),
            of: Box::new(Target::Item {
                position: Box::new(position),
                of: Box::new(var("expenses")),
            }),
        };
        assert_eq!(nested.root(), "expenses");
        assert_eq!(var("total").root(), "total");
    }
}
