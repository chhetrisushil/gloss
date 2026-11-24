//! Tables in memory: `Global::Tables` holds the address of a block of two words per table,
//! in declaration order: the table's list, then its loaded flag.
use super::Lowerer;
use crate::ast::program::Program;
use crate::ir::{Data, Function, Global, Inst, Reg};

/// The size in bytes of one table's entry in the block.
const ENTRY: i32 = 16;
/// A table's list starts with room for this many rows, as `list of` does.
const ROOM: i64 = 4;

impl Lowerer<'_> {
    /// The index of the table `name`, when it names one; a variable never reuses a table's name.
    pub fn table_index(&self, name: &str) -> Option<usize> {
        self.program.tables().position(|t| t.name == name)
    }

    /// Puts the list of table `index` in `a`.
    pub fn load_table(&mut self, index: usize) {
        self.emit(Inst::AddressOf(Reg::A, Data::Global(Global::Tables)));
        for offset in [0, ENTRY * index as i32] {
            self.emit(Inst::Load {
                to: Reg::A,
                base: Reg::A,
                offset,
            });
        }
    }

    /// `clear t`: a new empty list replaces the table's (a list a caller copied earlier keeps
    /// its rows), and the loaded flag becomes 1.
    pub fn clear_table(&mut self, index: usize) {
        let fresh = self.new_temp();
        self.emit(Inst::Const(Reg::A, ROOM));
        self.emit(Inst::Push(Reg::A));
        self.emit(Inst::Const(Reg::A, i64::from(self.line)));
        self.emit(Inst::Push(Reg::A));
        self.call("list_new");
        self.emit(Inst::StoreSlot(fresh, Reg::A));
        self.emit(Inst::AddressOf(Reg::A, Data::Global(Global::Tables)));
        self.emit(Inst::Load {
            to: Reg::A,
            base: Reg::A,
            offset: 0,
        });
        self.emit(Inst::LoadSlot(Reg::B, fresh));
        let at = ENTRY * index as i32;
        self.emit(Inst::Store {
            base: Reg::A,
            offset: at,
            from: Reg::B,
        });
        self.emit(Inst::Const(Reg::B, 1));
        self.emit(Inst::Store {
            base: Reg::A,
            offset: at + 8,
            from: Reg::B,
        });
    }

    /// A slot holding the list `name` names: a variable's own slot, or a hidden one loaded
    /// from the table.
    pub fn list_slot(&mut self, name: &str) -> u32 {
        let Some(index) = self.table_index(name) else {
            return self.slots[name];
        };
        let slot = self.new_temp();
        self.load_table(index);
        self.emit(Inst::StoreSlot(slot, Reg::A));
        slot
    }
}

/// `tables_init`: allocates the block, gives each table a new empty list and a loaded flag of 0
/// (a fresh block is zeroed). A program with no tables allocates nothing. Slot 1 is the block.
pub fn tables_init(program: &Program, called: &mut Vec<String>) -> Function {
    let n = program.tables().count() as i32;
    let mut body = vec![Inst::Enter(1)];
    if n > 0 {
        for name in ["alloc", "list_new"] {
            called.push(name.to_string());
        }
        let push = |body: &mut Vec<Inst>, value: i64| {
            body.push(Inst::Const(Reg::A, value));
            body.push(Inst::Push(Reg::A));
        };
        push(&mut body, i64::from(ENTRY * n));
        push(&mut body, 0);
        body.push(Inst::Call("alloc".to_string()));
        body.push(Inst::StoreSlot(1, Reg::A));
        for i in 0..n {
            push(&mut body, ROOM);
            push(&mut body, 0);
            body.push(Inst::Call("list_new".to_string()));
            body.push(Inst::LoadSlot(Reg::B, 1));
            body.push(Inst::Store {
                base: Reg::B,
                offset: ENTRY * i,
                from: Reg::A,
            });
        }
        body.push(Inst::LoadSlot(Reg::A, 1));
        body.push(Inst::AddressOf(Reg::B, Data::Global(Global::Tables)));
        body.push(Inst::Store {
            base: Reg::B,
            offset: 0,
            from: Reg::A,
        });
    }
    body.push(Inst::Return);
    Function {
        name: "tables_init".to_string(),
        params: 0,
        slots: 1,
        body,
    }
}
