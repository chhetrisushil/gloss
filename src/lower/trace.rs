//! The `_start` of a trace executable: the real one without `args_check`, which a traced
//! program (it takes no flags) has no use for.
use super::{Lowerer, Mode};
use crate::ast::program::Program;
use crate::ir::{Function, Inst, Module, Prim, Reg};
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
