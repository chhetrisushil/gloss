pub mod print;
pub mod program;
pub mod sexpr;
pub mod stmt;
mod stmt_sexpr;

pub type Line = u32;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct ExprId(pub u32);

#[derive(Clone, Debug, PartialEq)]
pub struct Expr {
    pub id: ExprId,
    pub line: Line,
    pub kind: ExprKind,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PlainType {
    Number,
    Text,
    Truth,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ItemType {
    Plain(PlainType),
    Record(String),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Type {
    Item(ItemType),
    List(ItemType),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Fact {
    Count,
    First,
    Last,
    Whole,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Conversion {
    ToNumber,
    ToText,
    ToTextPlaces(u8),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ArithOp {
    Add,
    Sub,
    Mul,
    Div,
}

impl ArithOp {
    /// The operator as it is written.
    pub fn symbol(self) -> &'static str {
        match self {
            ArithOp::Add => "+",
            ArithOp::Sub => "-",
            ArithOp::Mul => "*",
            ArithOp::Div => "/",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CompareOp {
    Is,
    IsNot,
    Greater,
    Less,
    AtLeast,
    AtMost,
}

impl CompareOp {
    /// The comparison as it is written, in words.
    pub fn words(self) -> &'static str {
        match self {
            CompareOp::Is => "is",
            CompareOp::IsNot => "is not",
            CompareOp::Greater => "is greater than",
            CompareOp::Less => "is less than",
            CompareOp::AtLeast => "is at least",
            CompareOp::AtMost => "is at most",
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub enum ExprKind {
    Number(crate::number::Number),
    Text(String),
    Truth(bool),
    Name(String),
    Shown,
    EmptyList(ItemType),
    Paren(Box<Expr>),
    FieldOf {
        field: String,
        of: Box<Expr>,
    },
    Fact {
        fact: Fact,
        of: Box<Expr>,
    },
    ItemOf {
        position: Box<Expr>,
        of: Box<Expr>,
    },
    Remainder {
        of: Box<Expr>,
        by: Box<Expr>,
    },
    Negate(Box<Expr>),
    Convert {
        value: Box<Expr>,
        to: Conversion,
    },
    Binary {
        op: ArithOp,
        left: Box<Expr>,
        right: Box<Expr>,
    },
    Call {
        feature: String,
        inputs: Vec<Expr>,
    },
    BuildRecord {
        record: String,
        fields: Vec<(String, Expr)>,
    },
    BuildList(Vec<Expr>),
    Compare {
        op: CompareOp,
        left: Box<Expr>,
        right: Box<Expr>,
    },
    IsNumber {
        value: Box<Expr>,
        negated: bool,
    },
    Not(Box<Expr>),
    And(Vec<Expr>),
    Or(Vec<Expr>),
}

impl Expr {
    /// The direct sub-expressions, for editing in place.
    pub fn children_mut(&mut self) -> Vec<&mut Expr> {
        match &mut self.kind {
            ExprKind::Paren(x)
            | ExprKind::FieldOf { of: x, .. }
            | ExprKind::Fact { of: x, .. }
            | ExprKind::Negate(x)
            | ExprKind::Convert { value: x, .. }
            | ExprKind::IsNumber { value: x, .. }
            | ExprKind::Not(x) => vec![x],
            ExprKind::ItemOf { position: a, of: b }
            | ExprKind::Remainder { of: a, by: b }
            | ExprKind::Binary {
                left: a, right: b, ..
            }
            | ExprKind::Compare {
                left: a, right: b, ..
            } => vec![a, b],
            ExprKind::Call { inputs: xs, .. }
            | ExprKind::BuildList(xs)
            | ExprKind::And(xs)
            | ExprKind::Or(xs) => xs.iter_mut().collect(),
            ExprKind::BuildRecord { fields, .. } => fields.iter_mut().map(|(_, x)| x).collect(),
            _ => Vec::new(),
        }
    }

    /// A copy with every pair of parentheses removed, for comparing what two expressions mean.
    pub fn without_parens(&self) -> Expr {
        let mut copy = self.clone();
        copy.strip_parens();
        copy
    }

    fn strip_parens(&mut self) {
        while let ExprKind::Paren(inner) = &self.kind {
            *self = (**inner).clone();
        }
        self.children_mut().into_iter().for_each(Expr::strip_parens);
    }

    /// Applies `edit` to the node with this id, if it is in the tree; true when found.
    pub fn edit_node(&mut self, id: ExprId, edit: &mut dyn FnMut(&mut Expr)) -> bool {
        if self.id == id {
            edit(self);
            return true;
        }
        self.children_mut()
            .into_iter()
            .any(|child| child.edit_node(id, edit))
    }
}

pub fn item_type_text(t: &ItemType) -> String {
    match t {
        ItemType::Plain(PlainType::Number) => "number".into(),
        ItemType::Plain(PlainType::Text) => "text".into(),
        ItemType::Plain(PlainType::Truth) => "truth".into(),
        ItemType::Record(name) => name.clone(),
    }
}

pub fn type_text(t: &Type) -> String {
    match t {
        Type::Item(i) => item_type_text(i),
        Type::List(i) => format!("list of {}", item_type_text(i)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn plain(p: PlainType) -> ItemType {
        ItemType::Plain(p)
    }

    #[test]
    fn item_type_texts() {
        assert_eq!(item_type_text(&plain(PlainType::Number)), "number");
        assert_eq!(item_type_text(&plain(PlainType::Text)), "text");
        assert_eq!(item_type_text(&plain(PlainType::Truth)), "truth");
        assert_eq!(
            item_type_text(&ItemType::Record("Student".into())),
            "Student"
        );
    }

    #[test]
    fn type_texts() {
        let n = plain(PlainType::Number);
        assert_eq!(type_text(&Type::Item(n.clone())), "number");
        assert_eq!(type_text(&Type::List(n)), "list of number");
        let r = ItemType::Record("Student".into());
        assert_eq!(type_text(&Type::List(r)), "list of Student");
    }

    #[test]
    fn edit_node_changes_only_the_node_with_that_id() {
        let mut tree = crate::parser::parse_expr_str("a + (b * c)").unwrap();
        let ExprKind::Binary { right, .. } = &tree.kind else {
            panic!("a sum");
        };
        let target = right.id;
        let found = tree.edit_node(target, &mut |n| n.kind = ExprKind::Name("z".into()));
        assert!(found);
        assert_eq!(print::expr(&tree), "a + z");
        assert!(!tree.edit_node(ExprId(9999), &mut |_| panic!("no such node")));
    }
}
