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
    // First used by the events of later tasks.
    #[allow(dead_code)]
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
    // First used by the events of later tasks.
    #[allow(dead_code)]
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
