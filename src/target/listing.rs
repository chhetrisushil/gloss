//! A target-neutral listing of the code: routines of line markers, labels and instructions.

/// One function or helper and what it assembles to.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Routine {
    pub name: String,
    pub items: Vec<Item>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Item {
    /// A `Line` marker: the code after it belongs to source line N.
    Line(u32),
    /// A label; `local` is the IR label number of the function's own label, printed `.L<n>`.
    Label {
        text: String,
        local: Option<u32>,
    },
    Inst {
        text: String,
        refs: Vec<Ref>,
    },
}

/// What an instruction refers to, besides registers.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Ref {
    Slot(u32),
    Input(u32),
    Text(u32),
    Jump(u32),
    System(&'static str),
}
