use super::Ty;

impl Ty {
    /// How the type reads in messages.
    pub fn describe(&self) -> String {
        match self {
            Ty::Number => "number".into(),
            Ty::Text => "text".into(),
            Ty::Truth => "truth".into(),
            Ty::Record(name) => name.clone(),
            Ty::List(item) => format!("list of {}", item.describe()),
            Ty::Table { table, .. } => format!("table {table}"),
            Ty::Nothing => "nothing".into(),
            Ty::Error => "an error".into(),
        }
    }

    /// The item type of a list or table.
    pub fn item(&self) -> Option<Ty> {
        match self {
            Ty::List(item) => Some((**item).clone()),
            Ty::Table { record, .. } => Some(Ty::Record(record.clone())),
            _ => None,
        }
    }

    /// The type with its article, as in "n is a number".
    pub fn described(&self) -> String {
        match self {
            Ty::Number | Ty::Truth => format!("a {}", self.describe()),
            _ => self.describe(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn describes_and_itemises() {
        let list = Ty::List(Box::new(Ty::Number));
        assert_eq!(list.describe(), "list of number");
        let table = Ty::Table {
            table: "es".into(),
            record: "E".into(),
        };
        assert_eq!(table.describe(), "table es");
        assert_eq!(Ty::Nothing.describe(), "nothing");
        assert_eq!(list.item(), Some(Ty::Number));
        assert_eq!(table.item(), Some(Ty::Record("E".into())));
        assert_eq!(Ty::Text.item(), None);
        assert_eq!(Ty::Number.described(), "a number");
        assert_eq!(Ty::Text.described(), "text");
    }
}
