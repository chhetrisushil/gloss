pub mod print;
pub mod sexpr;
pub mod stmt;

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

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CompareOp {
    Is,
    IsNot,
    Greater,
    Less,
    AtLeast,
    AtMost,
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
}
