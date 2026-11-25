//! The `_start` of a trace executable: the real one without `args_check`, which a traced
//! program (it takes no flags) has no use for.
use super::{Lowerer, Mode};
use crate::ast::print;
use crate::ast::program::Program;
use crate::ast::stmt::Target;
use crate::ast::{Expr, ExprKind};
use crate::check::{NameKind, Ty};
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

impl<'a> Cell<'a> {
    /// The cell `target` names (a field of a record variable, an item, or a field of an
    /// item), whose whole item number is in slot `item` when it is an item.
    pub fn of(target: &'a Target, item: Option<u32>, at: u32) -> Self {
        let field = match target {
            Target::Field { field, .. } => Some(field.as_str()),
            _ => None,
        };
        Cell {
            name: target.root(),
            item,
            field,
            at,
        }
    }
}

/// What the writers of a `set` event differ in: the keys after `name`, the slot `from` is read
/// from, and the made flag that guards it (a variable's first set has no `from`).
struct SetPieces<'a> {
    name: &'a str,
    item: Option<u32>,
    field: Option<&'a str>,
    old: u32,
    made: Option<u32>,
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
        let pieces = SetPieces {
            name,
            item: None,
            field: None,
            old: self.slots[name],
            made: (!from_always).then(|| self.made[name]),
        };
        self.write_set(line, &pieces, new, &ty);
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
        let pieces = SetPieces {
            name: cell.name,
            item: cell.item,
            field: cell.field,
            old,
            made: None,
        };
        self.write_set(self.line, &pieces, new, ty);
        self.emit(Inst::LoadSlot(Reg::A, new));
    }

    /// Writes one `set` event: `name`, the `item` and `field` keys when there are any, `from`
    /// (the value in slot `pieces.old`, left out while the made flag in `pieces.made` is 0),
    /// then `to` from slot `new`.
    fn write_set(&mut self, line: u32, pieces: &SetPieces, new: u32, ty: &Ty) {
        self.ev_open(line, "set");
        self.ev_key("name");
        self.ev_fixed_text(pieces.name);
        if let Some(index) = pieces.item {
            self.ev_key("item");
            self.ev_int_from_slot(index);
        }
        if let Some(field) = pieces.field {
            self.ev_key("field");
            self.ev_fixed_text(field);
        }
        let first = pieces.made.map(|flag| {
            let first = self.new_label();
            self.emit(Inst::LoadSlot(Reg::A, flag));
            self.jump_if_a_is(0, first);
            first
        });
        self.ev_key("from");
        self.ev_value_from_slot(pieces.old, ty);
        if let Some(first) = first {
            self.emit(Inst::Mark(first));
        }
        self.ev_key("to");
        self.ev_value_from_slot(new, ty);
        self.ev_close();
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

    /// Writes the `check` event of `cond`, whose truth is in `a`: the condition's text, the
    /// variables and field reads it names read again now (a plain load, never a call or an
    /// item, so nothing runs twice or fails), and the result. Leaves the truth in `a`.
    pub(super) fn check_event(&mut self, cond: &Expr) {
        let truth = self.new_temp();
        self.emit(Inst::StoreSlot(truth, Reg::A));
        let mut named = Vec::new();
        self.condition_values(cond, &mut named);
        let mut values = Vec::new();
        for (text, e) in named {
            let ty = self.types.expr_types[e.id.0 as usize].clone();
            let (Some(ty), Ok(())) = (ty, self.expr(e)) else {
                continue;
            };
            let slot = self.new_temp();
            self.emit(Inst::StoreSlot(slot, Reg::A));
            values.push((text, slot, ty));
        }
        self.ev_open(cond.line, "check");
        self.ev_key("condition");
        self.ev_fixed_text(&print::expr(cond));
        self.ev_raw(",\"values\":{");
        for (i, (text, slot, ty)) in values.iter().enumerate() {
            let comma = if i == 0 { "" } else { "," };
            self.ev_raw(&format!("{comma}\"{}\":", json_escape_host(text)));
            self.ev_value_from_slot(*slot, ty);
        }
        self.ev_raw("}");
        self.ev_key("result");
        let (no, end) = (self.new_label(), self.new_label());
        self.emit(Inst::LoadSlot(Reg::A, truth));
        self.jump_if_a_is(0, no);
        self.ev_fixed_text("yes");
        self.emit(Inst::Jump(end));
        self.emit(Inst::Mark(no));
        self.ev_fixed_text("no");
        self.emit(Inst::Mark(end));
        self.ev_close();
        self.emit(Inst::LoadSlot(Reg::A, truth));
    }

    /// The variables, loop variables, inputs and field reads of them that `e` names, each
    /// once by printed form, in order of first appearance. An item is left out with
    /// everything inside it; the root of a field read is not a value of its own.
    fn condition_values<'e>(&self, e: &'e Expr, out: &mut Vec<(String, &'e Expr)>) {
        let plain = |x: &Expr| {
            matches!(x.kind, ExprKind::Name(_))
                && matches!(
                    self.types.names.get(&x.id),
                    Some(NameKind::Variable | NameKind::LoopVariable | NameKind::Input)
                )
        };
        let read = match &e.kind {
            ExprKind::ItemOf { .. } => return,
            ExprKind::Name(_) => plain(e),
            ExprKind::FieldOf { of, .. } if plain(of) => true,
            _ => false,
        };
        if read {
            let text = print::expr(e);
            if !out.iter().any(|(t, _)| *t == text) {
                out.push((text, e));
            }
            return;
        }
        for child in crate::ast::walk::children(e) {
            self.condition_values(child, out);
        }
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
