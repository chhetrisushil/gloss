use super::stmt::{Note, Stmt};
use super::{Line, PlainType, Type};

#[derive(Clone, Debug, PartialEq)]
pub struct Program {
    pub items: Vec<Item>,
}

#[derive(Clone, Debug, PartialEq)]
pub enum Item {
    /// `attached` is true when no blank line separates the note from the next item.
    Note {
        line: Line,
        note: Note,
        attached: bool,
    },
    Record(RecordDecl),
    Table(TableDecl),
    Feature(FeatureDecl),
    Test(TestDecl),
}

#[derive(Clone, Debug, PartialEq)]
pub enum RecordEntry {
    Field(FieldDecl),
    Note(Note),
}

#[derive(Clone, Debug, PartialEq)]
pub struct RecordDecl {
    pub line: Line,
    pub name: String,
    pub entries: Vec<RecordEntry>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct FieldDecl {
    pub line: Line,
    pub name: String,
    pub ty: PlainType,
}

#[derive(Clone, Debug, PartialEq)]
pub struct TableDecl {
    pub line: Line,
    pub name: String,
    pub record: String,
    pub path: String,
}

#[derive(Clone, Debug, PartialEq)]
pub struct InputDecl {
    pub line: Line,
    pub name: String,
    pub ty: Type,
}

#[derive(Clone, Debug, PartialEq)]
pub struct FeatureDecl {
    pub line: Line,
    pub name: String,
    pub needs: Vec<InputDecl>,
    pub body: Vec<Stmt>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct TestDecl {
    pub line: Line,
    pub title: String,
    pub body: Vec<Stmt>,
}

impl RecordDecl {
    pub fn fields(&self) -> impl Iterator<Item = &FieldDecl> {
        self.entries.iter().filter_map(|e| match e {
            RecordEntry::Field(f) => Some(f),
            RecordEntry::Note(_) => None,
        })
    }
}

impl Program {
    pub fn records(&self) -> impl Iterator<Item = &RecordDecl> {
        self.items.iter().filter_map(|i| match i {
            Item::Record(r) => Some(r),
            _ => None,
        })
    }

    pub fn tables(&self) -> impl Iterator<Item = &TableDecl> {
        self.items.iter().filter_map(|i| match i {
            Item::Table(t) => Some(t),
            _ => None,
        })
    }

    pub fn features(&self) -> impl Iterator<Item = &FeatureDecl> {
        self.items.iter().filter_map(|i| match i {
            Item::Feature(f) => Some(f),
            _ => None,
        })
    }

    pub fn tests(&self) -> impl Iterator<Item = &TestDecl> {
        self.items.iter().filter_map(|i| match i {
            Item::Test(t) => Some(t),
            _ => None,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn helpers_pick_out_each_kind_of_item() {
        let field = |name: &str| FieldDecl {
            line: 2,
            name: name.into(),
            ty: PlainType::Text,
        };
        let record = RecordDecl {
            line: 1,
            name: "S".into(),
            entries: vec![
                RecordEntry::Note(Note::Line("why".into())),
                RecordEntry::Field(field("name")),
            ],
        };
        let feature = FeatureDecl {
            line: 3,
            name: "f".into(),
            needs: vec![],
            body: vec![],
        };
        let test = TestDecl {
            line: 4,
            title: "t".into(),
            body: vec![],
        };
        let table = TableDecl {
            line: 5,
            name: "s".into(),
            record: "S".into(),
            path: "s.csv".into(),
        };
        let items = vec![
            Item::Record(record.clone()),
            Item::Feature(feature),
            Item::Test(test),
            Item::Table(table),
        ];
        let program = Program { items };
        assert_eq!(program.records().count(), 1);
        assert_eq!(program.tables().next().unwrap().path, "s.csv");
        assert_eq!(program.features().next().unwrap().name, "f");
        assert_eq!(program.tests().next().unwrap().title, "t");
        assert_eq!(record.fields().count(), 1);
    }
}
