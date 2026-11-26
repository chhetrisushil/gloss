//! Lowering: the checked program and its `TypeInfo` become a Gloss IR module.

mod cond;
mod copy;
mod count;
mod expr;
mod fail;
mod list;
mod notes;
mod record;
mod stmt;
mod table;
mod test;
#[cfg(test)]
mod tests;
mod trace;

use std::collections::BTreeMap;

use crate::ast::program::{FeatureDecl, Program};
use crate::check::{LoopVar, Ty, TypeInfo};
use crate::diag::Diagnostic;
use crate::ir::{Data, Function, Global, Inst, Label, Module, Reg};
use crate::runtime;
use crate::source::Source;

pub(crate) use fail::spell_invisible;
pub use notes::{FailNote, FunctionNotes, Notes};

/// What a lowering is for: the program as it runs, the test executable `gloss test` builds
/// (line stores, the driver, pretend files and the test statements `expect`, `input`,
/// `shown`), or the executable `gloss trace` runs (the real program with its own `_start`).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Mode {
    Real,
    Test,
    Trace,
}

/// What lowering one feature needs: the module for its texts, the routines it calls, and
/// the code so far.
pub(crate) struct Lowerer<'a> {
    pub module: &'a mut Module,
    pub mode: Mode,
    pub src: &'a Source,
    pub program: &'a Program,
    pub called: Vec<String>,
    pub body: Vec<Inst>,
    pub types: &'a TypeInfo,
    /// Slot numbers by variable name; inputs are read with `LoadArg` by position.
    pub slots: BTreeMap<String, u32>,
    /// The type of each variable and loop variable.
    pub vars: BTreeMap<String, crate::check::Ty>,
    pub inputs: Vec<String>,
    pub labels: u32,
    pub fails: Vec<fail::Fail>,
    /// The source line of the statement being lowered.
    pub line: u32,
    /// The `skip` and `stop` targets of the loops being lowered, innermost last.
    pub loops: Vec<(Label, Label)>,
    /// Hidden slots handed out so far, numbered after the named ones.
    pub temps: u32,
    /// A trace build's "made" flag slot of each variable: 0 until a `set` or `ask` makes it
    /// (frames start zeroed), 1 after. Empty outside a trace build.
    pub made: BTreeMap<String, u32>,
}

impl Lowerer<'_> {
    /// A hidden slot, for a value a statement keeps while it runs.
    pub fn new_temp(&mut self) -> u32 {
        self.temps += 1;
        self.slots.len() as u32 + self.temps
    }

    /// Starts the code of the statement on `line`. A test build also stores the line in the
    /// test state, where the shown buffer's allocations read it; `a` and `b` are free here.
    pub fn mark_line(&mut self, line: u32) {
        self.line = line;
        self.emit(Inst::Line(line));
        self.store_line();
    }

    /// In a test build, stores the line being lowered in the test state (see `mark_line`).
    fn store_line(&mut self) {
        match self.mode {
            Mode::Test => {
                self.emit(Inst::Const(Reg::A, i64::from(self.line)));
                self.emit(Inst::AddressOf(Reg::B, Data::Global(Global::TestState)));
                self.emit(Inst::Store {
                    base: Reg::B,
                    offset: runtime::CURRENT_LINE as i32 * 8,
                    from: Reg::A,
                });
            }
            Mode::Real | Mode::Trace => {}
        }
    }

    /// After a call returns, a test build takes back the line word the callee moved, keeping
    /// the result in `a` (`b` is the callee's to clobber).
    pub fn restore_line(&mut self) {
        match self.mode {
            Mode::Test => {
                self.emit(Inst::Push(Reg::A));
                self.store_line();
                self.emit(Inst::Pop(Reg::A));
            }
            Mode::Real | Mode::Trace => {}
        }
    }

    pub fn emit(&mut self, inst: Inst) {
        self.body.push(inst);
    }

    /// Calls a runtime routine, noting it so the runtime brings it in.
    pub fn call(&mut self, name: &str) {
        if !self.called.iter().any(|c| c == name) {
            self.called.push(name.to_string());
        }
        self.emit(Inst::Call(name.to_string()));
    }

    /// Puts the address of the text `bytes` in `a`.
    pub fn text_address(&mut self, bytes: &[u8]) {
        let k = self.module.text(bytes);
        self.emit(Inst::AddressOf(Reg::A, crate::ir::Data::Text(k)));
    }

    /// Pushes the address of the text `bytes`.
    pub fn push_text(&mut self, bytes: &[u8]) {
        self.text_address(bytes);
        self.emit(Inst::Push(Reg::A));
    }
}

/// The name of the IR function for a feature; the prefix keeps it clear of runtime names.
fn function_name(feature: &str) -> String {
    format!("f_{feature}")
}

impl<'a> Lowerer<'a> {
    /// A lowerer for a function whose variables are `vars` and `loops`, reading `inputs` by
    /// position.
    fn new(
        (module, mode): (&'a mut Module, Mode),
        (src, program, types): (&'a Source, &'a Program, &'a TypeInfo),
        (vars, loops): (&[(String, Ty, u32)], &[LoopVar]),
        inputs: Vec<String>,
    ) -> Self {
        let slots = assign_slots(vars, loops);
        let mut l = Lowerer {
            module,
            mode,
            src,
            program,
            called: Vec::new(),
            body: vec![Inst::Enter(slots.len() as u32)],
            types,
            slots,
            vars: variable_types(vars),
            inputs,
            labels: 0,
            fails: Vec::new(),
            line: 0,
            loops: Vec::new(),
            temps: 0,
            made: BTreeMap::new(),
        };
        if mode == Mode::Trace {
            for (name, ..) in vars {
                let flag = l.new_temp();
                l.made.insert(name.clone(), flag);
            }
        }
        l
    }

    /// Ends the function: the failure blocks, the frame (the named slots and the hidden
    /// ones) and the routines it called, added to `called`. Gives the slot count and body.
    fn finish(mut self, called: &mut Vec<String>) -> (u32, Vec<Inst>) {
        self.emit_fails();
        let count = self.slots.len() as u32 + self.temps;
        self.body[0] = Inst::Enter(count);
        for name in self.called {
            if !called.contains(&name) {
                called.push(name);
            }
        }
        (count, self.body)
    }
}

fn lower_feature(
    (module, mode): (&mut Module, Mode),
    context: (&Source, &Program, &TypeInfo),
    feature: &FeatureDecl,
    called: &mut Vec<String>,
    notes: Option<&mut Notes>,
) -> Result<Function, Diagnostic> {
    let types = context.2;
    let info = types.features.get(&feature.name);
    let (vars, loops) = info.map_or((&[][..], &[][..]), |i| (&i.vars[..], &i.loops[..]));
    let inputs = feature.needs.iter().map(|i| i.name.clone()).collect();
    let mut l = Lowerer::new((module, mode), context, (vars, loops), inputs);
    l.stmts(&feature.body)?;
    if info.is_some_and(|i| !i.gives.is_empty()) {
        l.fall_off_end(&feature.name, feature.line);
    } else {
        l.emit(Inst::Line(feature.line));
        l.emit(Inst::Return);
    }
    let name = function_name(&feature.name);
    if let Some(notes) = notes {
        notes.functions.insert(
            name.clone(),
            FunctionNotes {
                feature: feature.name.clone(),
                line: feature.line,
                slots: l.slots.iter().map(|(n, k)| (*k, n.clone())).collect(),
                inputs: l.inputs.clone(),
                fails: l
                    .fails
                    .iter()
                    .map(|f| (f.label_number(), f.note()))
                    .collect(),
            },
        );
    }
    let (slots, body) = l.finish(called);
    Ok(Function {
        name,
        params: feature.needs.len() as u32,
        slots,
        body,
    })
}

/// Fills in every `StackCheck`: the callee's frame is `8 * (slots + 3)` bytes (a return
/// address, a saved frame pointer, a saved `b` and the slots), plus 64 for the helpers that
/// run below it (M2-R12). The callee's hidden slots are known only after it is lowered.
/// The deeper chains of runtime helpers (a trace build's `json_value` or failure message
/// writing: about 550 bytes of frames and arguments at most, measured in task 6) run in the
/// `STACK_MARGIN` of 64 KiB that `stack_limit` leaves below the checked limit, not in these 64
/// bytes, so the margin needs no change for trace builds.
fn size_stack_checks(module: &mut Module) {
    let slots: BTreeMap<String, u32> = module
        .functions
        .iter()
        .map(|f| (f.name.clone(), f.slots))
        .collect();
    for f in &mut module.functions {
        // A trace build writes the `enter` event between a check and its call, so the check
        // belongs to the next call of a user feature, whatever lies between (runtime calls).
        let mut open: Option<usize> = None;
        for i in 0..f.body.len() {
            match &f.body[i] {
                Inst::StackCheck { .. } => open = Some(i),
                Inst::Call(name) if name.starts_with("f_") => {
                    if let Some(check) = open.take() {
                        let bytes = 8 * (slots[name] + 3) + 64;
                        if let Inst::StackCheck { bytes: b, .. } = &mut f.body[check] {
                            *b = bytes;
                        }
                    }
                }
                _ => {}
            }
        }
    }
}

/// The type of each variable in `vars`. A loop variable gets its type while its loop's body
/// is lowered (`loop_body`), because two loops may reuse a name with different types.
fn variable_types(vars: &[(String, Ty, u32)]) -> BTreeMap<String, Ty> {
    vars.iter()
        .map(|(n, ty, _)| (n.clone(), ty.clone()))
        .collect()
}

/// Slots are numbered from 1: variables first, then loop variables. A name gets one slot, so
/// two loops that use the same name share it; the count is the number of unique variable and
/// loop names, inputs excluded (they are read with `LoadArg`).
fn assign_slots(vars: &[(String, Ty, u32)], loops: &[LoopVar]) -> BTreeMap<String, u32> {
    let mut slots = BTreeMap::new();
    let names = vars
        .iter()
        .map(|v| &v.0)
        .chain(loops.iter().map(|l| &l.name));
    for name in names {
        let next = slots.len() as u32 + 1;
        slots.entry(name.clone()).or_insert(next);
    }
    slots
}

/// `permissions_text`: leaves in `a` the address of text `text`, the program's permission list.
fn permissions_text(text: u32) -> Function {
    Function {
        name: "permissions_text".to_string(),
        params: 0,
        slots: 0,
        body: vec![
            Inst::Enter(0),
            Inst::AddressOf(Reg::A, Data::Text(text)),
            Inst::Return,
        ],
    }
}

/// Lowers a checked program to an IR module, with the runtime it reaches.
pub fn lower(program: &Program, types: &TypeInfo, src: &Source) -> Result<Module, Diagnostic> {
    lower_explained(program, types, src).map(|(m, _)| m)
}

/// `lower`, and the names `gloss explain` shows: each feature's slots and inputs.
pub fn lower_explained(
    program: &Program,
    types: &TypeInfo,
    src: &Source,
) -> Result<(Module, Notes), Diagnostic> {
    let mut notes = Notes::default();
    let module = lower_program(program, types, src, Mode::Real, Some(&mut notes))?;
    Ok((module, notes))
}

/// Lowers a checked program for `gloss test`: every feature, and each test body as `t_<i>`.
pub fn lower_tests(
    program: &Program,
    types: &TypeInfo,
    src: &Source,
) -> Result<Module, Diagnostic> {
    lower_program(program, types, src, Mode::Test, None)
}

/// Lowers a checked program for `gloss trace`: the real program, with a `_start` of its own.
pub fn lower_trace(
    program: &Program,
    types: &TypeInfo,
    src: &Source,
) -> Result<Module, Diagnostic> {
    lower_program(program, types, src, Mode::Trace, None)
}

fn lower_program(
    program: &Program,
    types: &TypeInfo,
    src: &Source,
    mode: Mode,
    mut notes: Option<&mut Notes>,
) -> Result<Module, Diagnostic> {
    let mut module = Module::default();
    // A test or trace build has its own `_start`, put in before the runtime's would be.
    let mut called = match mode {
        Mode::Real => vec!["_start".to_string()],
        Mode::Test | Mode::Trace => Vec::new(),
    };
    let context = (src, program, types);
    for feature in program.features() {
        let f = lower_feature(
            (&mut module, mode),
            context,
            feature,
            &mut called,
            notes.as_deref_mut(),
        )?;
        module.functions.push(f);
    }
    match mode {
        Mode::Real => {}
        Mode::Test => {
            for (i, test) in program.tests().enumerate() {
                let f = test::lower_test((&mut module, mode), context, i, test, &mut called)?;
                module.functions.push(f);
            }
            let driver = test::lower_driver((&mut module, mode), context, &mut called);
            module.functions.extend(driver);
        }
        Mode::Trace => {
            let start = trace::lower_start((&mut module, mode), context, &mut called);
            module.functions.push(start);
        }
    }
    // The runtime's `args_check` is the only caller, and a test or trace executable takes no
    // flags.
    match mode {
        Mode::Real => {
            let list = crate::permissions::render(&crate::permissions::list(program));
            let text = module.text(list.as_bytes());
            module.functions.push(permissions_text(text));
        }
        Mode::Test | Mode::Trace => {}
    }
    module
        .functions
        .push(table::tables_init(program, &mut called, mode));
    size_stack_checks(&mut module);
    let backend = match mode {
        Mode::Real => runtime::Backend::Real,
        Mode::Test => runtime::Backend::Test,
        Mode::Trace => runtime::Backend::Trace,
    };
    runtime::add_reachable_for(&mut module, &called, backend);
    Ok(module)
}
