//! Lowering: the checked program and its `TypeInfo` become a Gloss IR module.

mod cond;
mod copy;
mod count;
mod expr;
mod fail;
mod list;
mod record;
mod stmt;
mod table;
mod test;
#[cfg(test)]
mod tests;

use std::collections::BTreeMap;

use crate::ast::program::{FeatureDecl, Program};
use crate::check::{LoopVar, Ty, TypeInfo};
use crate::diag::Diagnostic;
use crate::ir::{Data, Function, Inst, Label, Module, Reg};
use crate::runtime;
use crate::source::Source;

/// What a lowering is for: the program as it runs, or the test executable `gloss test`
/// builds, where the statements this milestone has not built are refused.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Mode {
    Real,
    Test,
}

/// What lowering one feature needs: the module for its texts, the routines it calls, and
/// the code so far.
pub(crate) struct Lowerer<'a> {
    pub module: &'a mut Module,
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
}

impl Lowerer<'_> {
    /// A hidden slot, for a value a statement keeps while it runs.
    pub fn new_temp(&mut self) -> u32 {
        self.temps += 1;
        self.slots.len() as u32 + self.temps
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
        module: &'a mut Module,
        (src, program, types): (&'a Source, &'a Program, &'a TypeInfo),
        (vars, loops): (&[(String, Ty, u32)], &[LoopVar]),
        inputs: Vec<String>,
    ) -> Self {
        let slots = assign_slots(vars, loops);
        Lowerer {
            module,
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
        }
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
    module: &mut Module,
    context: (&Source, &Program, &TypeInfo),
    feature: &FeatureDecl,
    called: &mut Vec<String>,
) -> Result<Function, Diagnostic> {
    let types = context.2;
    let info = types.features.get(&feature.name);
    let (vars, loops) = info.map_or((&[][..], &[][..]), |i| (&i.vars[..], &i.loops[..]));
    let inputs = feature.needs.iter().map(|i| i.name.clone()).collect();
    let mut l = Lowerer::new(module, context, (vars, loops), inputs);
    l.stmts(&feature.body)?;
    if info.is_some_and(|i| !i.gives.is_empty()) {
        l.fall_off_end(&feature.name);
    } else {
        l.emit(Inst::Return);
    }
    let (slots, body) = l.finish(called);
    Ok(Function {
        name: function_name(&feature.name),
        params: feature.needs.len() as u32,
        slots,
        body,
    })
}

/// Fills in every `StackCheck`: the callee's frame is `8 * (slots + 3)` bytes (a return
/// address, a saved frame pointer, a saved `b` and the slots), plus 64 for the helpers that
/// run below it (M2-R12). The callee's hidden slots are known only after it is lowered.
fn size_stack_checks(module: &mut Module) {
    let slots: BTreeMap<String, u32> = module
        .functions
        .iter()
        .map(|f| (f.name.clone(), f.slots))
        .collect();
    for f in &mut module.functions {
        for i in 1..f.body.len() {
            if let (Inst::Call(name), Inst::StackCheck { bytes, .. }) =
                (&f.body[i].clone(), &mut f.body[i - 1])
            {
                *bytes = 8 * (slots[name] + 3) + 64;
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
    lower_program(program, types, src, Mode::Real)
}

/// Lowers a checked program for `gloss test`: every feature, and each test body as `t_<i>`.
pub fn lower_tests(
    program: &Program,
    types: &TypeInfo,
    src: &Source,
) -> Result<Module, Diagnostic> {
    lower_program(program, types, src, Mode::Test)
}

fn lower_program(
    program: &Program,
    types: &TypeInfo,
    src: &Source,
    mode: Mode,
) -> Result<Module, Diagnostic> {
    let mut module = Module::default();
    let mut called = vec!["_start".to_string()];
    let context = (src, program, types);
    for feature in program.features() {
        let f = lower_feature(&mut module, context, feature, &mut called)?;
        module.functions.push(f);
    }
    if mode == Mode::Test {
        for (i, test) in program.tests().enumerate() {
            let f = test::lower_test(&mut module, context, i, test, &mut called)?;
            module.functions.push(f);
        }
    }
    let list = crate::permissions::render(&crate::permissions::list(program));
    let text = module.text(list.as_bytes());
    module.functions.push(permissions_text(text));
    module
        .functions
        .push(table::tables_init(program, &mut called));
    size_stack_checks(&mut module);
    let backend = match mode {
        Mode::Real => runtime::Backend::Real,
        Mode::Test => runtime::Backend::Test,
    };
    runtime::add_reachable_for(&mut module, &called, backend);
    Ok(module)
}
