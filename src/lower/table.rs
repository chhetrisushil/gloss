//! Tables in memory: `Global::Tables` holds the address of a block of two words per table,
//! in declaration order: the table's list, then its loaded flag.
use crate::ast::program::Program;
use crate::ir::{Data, Function, Global, Inst, Reg};

/// The size in bytes of one table's entry in the block.
const ENTRY: i32 = 16;
/// A table's list starts with room for this many rows, as `list of` does.
const ROOM: i64 = 4;

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
