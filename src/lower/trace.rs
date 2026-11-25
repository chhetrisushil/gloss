//! The `_start` of a trace executable: the real one without `args_check`, which a traced
//! program (it takes no flags) has no use for.
use super::{Lowerer, Mode};
use crate::ast::program::Program;
use crate::check::Ty;
use crate::ir::{Data, Function, Global, Inst, Module, Prim, Reg};
use crate::runtime::json_escape_host;
use crate::runtime::trace_state::{ESCAPING, OPEN};
use crate::source::Source;

/// `_start`: set up the heap, the stack and the tables, run `f_start`, flush and exit 0. The
/// events of later tasks are written around the call to `f_start`.
pub(super) fn lower_start(
    (module, mode): (&mut Module, Mode),
    context: (&Source, &Program, &crate::check::TypeInfo),
    called: &mut Vec<String>,
) -> Function {
    let mut start = Lowerer::new((module, mode), context, (&[], &[]), Vec::new());
    for routine in ["heap_init", "stack_init", "tables_init"] {
        start.call(routine);
    }
    start.emit(Inst::Call("f_start".into()));
    start.call("flush");
    start.emit(Inst::Const(Reg::A, 0));
    start.emit(Inst::Push(Reg::A));
    start.emit(Inst::System(Prim::Exit));
    let (slots, body) = start.finish(called);
    Function {
        name: "_start".into(),
        params: 0,
        slots,
        body,
    }
}

/// A field or an item a `set` event names: the list or variable `name`, the slot holding the
/// whole item number, the field, and the slot holding the cell's address.
pub(super) struct Cell<'a> {
    pub name: &'a str,
    pub item: Option<u32>,
    pub field: Option<&'a str>,
    pub at: u32,
}

/// The helpers that frame a trace event: `{"line":L,"event":"K"`, then `,"key":value` pieces,
/// then `}` and a newline. Each emits nothing outside a trace build, and clobbers `a` and `b`:
/// a caller keeps its values in hidden slots (`new_temp`). Every helper needs `ESCAPING` at 0
/// (the fixed pieces and quotes would be escaped otherwise).
impl Lowerer<'_> {
    /// Writes a fixed piece, escaped by the compiler, through `out_text`.
    pub(super) fn ev_raw(&mut self, piece: &str) {
        if self.mode == Mode::Trace {
            self.push_text(piece.as_bytes());
            self.call("out_text");
        }
    }

    /// Writes `{"line":L,"event":"kind"`.
    pub fn ev_open(&mut self, line: u32, kind: &str) {
        let kind = json_escape_host(kind);
        self.ev_raw(&format!("{{\"line\":{line},\"event\":\"{kind}\""));
    }

    /// Writes `,"key":`.
    pub fn ev_key(&mut self, key: &str) {
        self.ev_raw(&format!(",\"{}\":", json_escape_host(key)));
    }

    /// Writes `s` as a JSON string, with its quotes, escaped by the compiler.
    pub fn ev_fixed_text(&mut self, s: &str) {
        self.ev_raw(&format!("\"{}\"", json_escape_host(s)));
    }

    /// Writes the value in `slot`, of type `ty`, as a typed JSON value (`json_value`).
    pub fn ev_value_from_slot(&mut self, slot: u32, ty: &Ty) {
        if self.mode == Mode::Trace {
            self.emit(Inst::LoadSlot(Reg::A, slot));
            self.emit(Inst::Push(Reg::A));
            let desc = self.type_desc(ty);
            self.emit(Inst::AddressOf(Reg::A, Data::Desc(desc)));
            self.emit(Inst::Push(Reg::A));
            self.call("json_value");
        }
    }

    /// Writes the text in `slot` as a JSON string, with its quotes (`json_text`).
    pub fn ev_text_from_slot(&mut self, slot: u32) {
        if self.mode == Mode::Trace {
            self.emit(Inst::LoadSlot(Reg::A, slot));
            self.emit(Inst::Push(Reg::A));
            self.call("json_text");
        }
    }

    /// Writes the whole number in `slot` in decimal (`json_int`).
    pub fn ev_int_from_slot(&mut self, slot: u32) {
        if self.mode == Mode::Trace {
            self.emit(Inst::LoadSlot(Reg::A, slot));
            self.emit(Inst::Push(Reg::A));
            self.call("json_int");
        }
    }

    /// Writes `}` and a newline.
    pub fn ev_close(&mut self) {
        self.ev_raw("}\n");
    }

    /// Writes the `set` event of variable `name`, whose new value is in `a`, before the
    /// caller stores it: `from` is read from the variable's slot, which still holds the old
    /// value, and only when the made flag is 1, or always for `from_always` (an `add` or the
    /// like, whose variable exists) (D44). Sets the flag; leaves the value in `a`.
    pub(super) fn set_event(&mut self, line: u32, name: &str, from_always: bool) {
        let ty = self.vars[name].clone();
        let new = self.new_temp();
        self.emit(Inst::StoreSlot(new, Reg::A));
        self.ev_open(line, "set");
        self.ev_key("name");
        self.ev_fixed_text(name);
        let first = (!from_always).then(|| self.new_label());
        if let Some(first) = first {
            self.emit(Inst::LoadSlot(Reg::A, self.made[name]));
            self.jump_if_a_is(0, first);
        }
        self.ev_key("from");
        self.ev_value_from_slot(self.slots[name], &ty);
        if let Some(first) = first {
            self.emit(Inst::Mark(first));
        }
        self.ev_key("to");
        self.ev_value_from_slot(new, &ty);
        self.ev_close();
        self.set_made(name);
        self.emit(Inst::LoadSlot(Reg::A, new));
    }

    /// Writes the `set` event of `cell`, a field or an item that exists, whose new value of
    /// type `ty` is in `a`, before the caller stores it: `from` is read from the cell's
    /// address, which still holds the old value. Leaves the value in `a`.
    pub(super) fn cell_set_event(&mut self, cell: &Cell, ty: &Ty) {
        let (new, old) = (self.new_temp(), self.new_temp());
        self.emit(Inst::StoreSlot(new, Reg::A));
        self.emit(Inst::LoadSlot(Reg::A, cell.at));
        self.emit(Inst::Load {
            to: Reg::A,
            base: Reg::A,
            offset: 0,
        });
        self.emit(Inst::StoreSlot(old, Reg::A));
        self.ev_open(self.line, "set");
        self.ev_key("name");
        self.ev_fixed_text(cell.name);
        if let Some(index) = cell.item {
            self.ev_key("item");
            self.ev_int_from_slot(index);
        }
        if let Some(field) = cell.field {
            self.ev_key("field");
            self.ev_fixed_text(field);
        }
        self.ev_key("from");
        self.ev_value_from_slot(old, ty);
        self.ev_key("to");
        self.ev_value_from_slot(new, ty);
        self.ev_close();
        self.emit(Inst::LoadSlot(Reg::A, new));
    }

    /// Writes the `set` event of an `append` to `name`, whose list is in slot `list` and whose
    /// pushed item of type `ty` is in slot `item`: `"item"` is the new count, and only the new
    /// item is written, never the whole list (D44).
    pub(super) fn append_event(&mut self, name: &str, (list, item): (u32, u32), ty: &Ty) {
        let count = self.new_temp();
        self.emit(Inst::LoadSlot(Reg::A, list));
        self.emit(Inst::Load {
            to: Reg::A,
            base: Reg::A,
            offset: 0,
        });
        self.emit(Inst::StoreSlot(count, Reg::A));
        self.ev_open(self.line, "set");
        self.ev_key("name");
        self.ev_fixed_text(name);
        self.ev_key("item");
        self.ev_int_from_slot(count);
        self.ev_key("to");
        self.ev_value_from_slot(item, ty);
        self.ev_close();
    }

    /// Sets `name`'s made flag (see `Lowerer::made`); clobbers `a`.
    pub(super) fn set_made(&mut self, name: &str) {
        if let Some(&flag) = self.made.get(name) {
            self.emit(Inst::Const(Reg::A, 1));
            self.emit(Inst::StoreSlot(flag, Reg::A));
        }
    }

    /// Sets the `ESCAPING` word: text entering the output buffer is escaped while it is 1.
    pub(super) fn set_escaping(&mut self, on: bool) {
        self.set_trace_word(ESCAPING, on);
    }

    /// Sets the `OPEN` word: a `show` event is open while it is 1.
    pub(super) fn set_open(&mut self, on: bool) {
        self.set_trace_word(OPEN, on);
    }

    /// Stores 0 or 1 in the trace state at byte `offset`, in the way `store_line` stores the
    /// line: a few instructions inline, where a runtime helper would be a call and a routine
    /// for each of two words.
    fn set_trace_word(&mut self, offset: i32, on: bool) {
        if self.mode == Mode::Trace {
            self.emit(Inst::Const(Reg::A, i64::from(on)));
            self.emit(Inst::AddressOf(Reg::B, Data::Global(Global::TraceState)));
            self.emit(Inst::Store {
                base: Reg::B,
                offset,
                from: Reg::A,
            });
        }
    }
}
