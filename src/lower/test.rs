//! Test bodies: each test is lowered to a function `t_<i>` of no inputs, in source order.
use super::{Lowerer, Mode};
use crate::ast::program::{Program, TestDecl};
use crate::diag::Diagnostic;
use crate::ir::{Cond, Function, Inst, Module, Prim, Reg};
use crate::source::Source;

/// The name of the IR function for test `i`.
pub(super) fn test_name(i: usize) -> String {
    format!("t_{i}")
}

/// Lowers test `i`, finished as a feature is: its failure blocks, its frame and the routines
/// it calls.
pub(super) fn lower_test(
    (module, mode): (&mut Module, Mode),
    context: (&Source, &Program, &crate::check::TypeInfo),
    i: usize,
    test: &TestDecl,
    called: &mut Vec<String>,
) -> Result<Function, Diagnostic> {
    let info = &context.2.tests[i];
    let mut l = Lowerer::new(
        (module, mode),
        context,
        (&info.vars, &info.loops),
        Vec::new(),
    );
    l.stmts(&test.body)?;
    l.emit(Inst::Return);
    let (slots, body) = l.finish(called);
    Ok(Function {
        name: test_name(i),
        params: 0,
        slots,
        body,
    })
}

/// `test_main` and the `_start` that runs it, which replaces the runtime's. The slots of
/// `test_main` are 1 the tests passed, 2 the tests failed, 3 the status of the child just run.
///
/// Per test the driver flushes, forks, and in the child shows the title, runs `t_<i>` and
/// reports a pass (which exits); the parent waits and counts: 0 a pass, 1 a failure (the child
/// reported it), anything else a test that ended without a result. `tables_init` runs once,
/// before the loop, so every child inherits fresh tables.
pub(super) fn lower_driver(
    (module, mode): (&mut Module, Mode),
    context: (&Source, &Program, &crate::check::TypeInfo),
    called: &mut Vec<String>,
) -> [Function; 2] {
    let mut main = Lowerer::new((&mut *module, mode), context, (&[], &[]), Vec::new());
    let (passed, failed, status) = (main.new_temp(), main.new_temp(), main.new_temp());
    main.emit(Inst::Const(Reg::A, 0));
    main.emit(Inst::StoreSlot(passed, Reg::A));
    main.emit(Inst::StoreSlot(failed, Reg::A));
    for (i, test) in context.1.tests().enumerate() {
        let title = super::fail::spell_invisible(&test.title);
        let (parent, lost, pass, fail, next) = (
            main.new_label(),
            main.new_label(),
            main.new_label(),
            main.new_label(),
            main.new_label(),
        );
        main.call("flush");
        main.emit(Inst::System(Prim::Fork));
        main.emit(Inst::Const(Reg::B, 0));
        main.emit(Inst::Compare);
        main.emit(Inst::JumpIf(Cond::Ne, parent));
        main.push_text(title.as_bytes());
        main.call("test_begin");
        main.emit(Inst::Call(test_name(i)));
        main.call("test_pass");
        main.emit(Inst::Mark(parent));
        // A fork that failed gave a negative result, and there is no child to wait for (a
        // wait on it would be for a process group): the test is lost with that status.
        main.emit(Inst::StoreSlot(status, Reg::A));
        main.emit(Inst::Const(Reg::B, 0));
        main.emit(Inst::Compare);
        main.emit(Inst::JumpIf(Cond::Lt, lost));
        main.emit(Inst::Push(Reg::A));
        main.emit(Inst::System(Prim::Wait));
        main.emit(Inst::StoreSlot(status, Reg::A));
        for (value, to) in [(0, pass), (1, fail)] {
            main.emit(Inst::Const(Reg::B, value));
            main.emit(Inst::Compare);
            main.emit(Inst::JumpIf(Cond::Eq, to));
        }
        main.emit(Inst::Mark(lost));
        main.push_text(title.as_bytes());
        main.emit(Inst::LoadSlot(Reg::A, status));
        main.emit(Inst::Push(Reg::A));
        main.call("test_lost");
        main.emit(Inst::Mark(fail));
        main.bump(failed);
        main.emit(Inst::Jump(next));
        main.emit(Inst::Mark(pass));
        main.bump(passed);
        main.emit(Inst::Mark(next));
    }
    for slot in [passed, failed] {
        main.emit(Inst::LoadSlot(Reg::A, slot));
        main.emit(Inst::Push(Reg::A));
    }
    main.call("test_summary");
    main.emit(Inst::LoadSlot(Reg::A, failed));
    main.emit(Inst::Const(Reg::B, 0));
    main.emit(Inst::Compare);
    main.emit(Inst::SetIf(Cond::Gt));
    main.emit(Inst::Return);
    let (slots, body) = main.finish(called);
    let main_fn = Function {
        name: "test_main".into(),
        params: 0,
        slots,
        body,
    };
    let mut start = Lowerer::new((module, mode), context, (&[], &[]), Vec::new());
    let result = start.new_temp();
    // An inherited ignored SIGCHLD would make every Wait fail: reset it before any fork.
    start.emit(Inst::System(Prim::DefaultChildSignal));
    for routine in ["heap_init", "stack_init", "tables_init", "test_main"] {
        start.call(routine);
    }
    start.emit(Inst::StoreSlot(result, Reg::A));
    start.call("flush");
    start.emit(Inst::LoadSlot(Reg::A, result));
    start.emit(Inst::Push(Reg::A));
    start.emit(Inst::System(Prim::Exit));
    let (slots, body) = start.finish(called);
    let start_fn = Function {
        name: "_start".into(),
        params: 0,
        slots,
        body,
    };
    [main_fn, start_fn]
}

impl Lowerer<'_> {
    /// Adds 1 to the number in `slot`.
    fn bump(&mut self, slot: u32) {
        self.emit(Inst::LoadSlot(Reg::A, slot));
        self.emit(Inst::AddImm(Reg::A, 1));
        self.emit(Inst::StoreSlot(slot, Reg::A));
    }
}
