//! Lowering: the checked program and its `TypeInfo` become a Gloss IR module.

mod cond;
mod count;
mod expr;
mod fail;
mod list;
mod record;
mod stmt;
#[cfg(test)]
mod tests;

use std::collections::BTreeMap;

use crate::ast::program::{FeatureDecl, Program};
use crate::check::TypeInfo;
use crate::diag::Diagnostic;
use crate::ir::{Function, Inst, Label, Module, Reg};
use crate::runtime;
use crate::source::Source;

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

fn lower_feature(
    module: &mut Module,
    src: &Source,
    program: &Program,
    types: &TypeInfo,
    feature: &FeatureDecl,
    called: &mut Vec<String>,
) -> Result<Function, Diagnostic> {
    let slots = assign_slots(types, &feature.name);
    let named = slots.len() as u32;
    let mut l = Lowerer {
        module,
        src,
        program,
        called: Vec::new(),
        body: vec![Inst::Enter(named)],
        types,
        slots,
        vars: variable_types(types, &feature.name),
        inputs: feature.needs.iter().map(|i| i.name.clone()).collect(),
        labels: 0,
        fails: Vec::new(),
        line: 0,
        loops: Vec::new(),
        temps: 0,
    };
    l.stmts(&feature.body)?;
    let gives = types
        .features
        .get(&feature.name)
        .is_some_and(|i| !i.gives.is_empty());
    if gives {
        l.fall_off_end(&feature.name);
    } else {
        l.emit(Inst::Return);
    }
    l.emit_fails();
    let count = named + l.temps;
    l.body[0] = Inst::Enter(count);
    for name in l.called {
        if !called.contains(&name) {
            called.push(name);
        }
    }
    Ok(Function {
        name: function_name(&feature.name),
        params: feature.needs.len() as u32,
        slots: count,
        body: l.body,
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

/// The type of each variable and loop variable of `feature`.
fn variable_types(types: &TypeInfo, feature: &str) -> BTreeMap<String, crate::check::Ty> {
    let Some(info) = types.features.get(feature) else {
        return BTreeMap::new();
    };
    let vars = info.vars.iter().map(|(n, ty, _)| (n.clone(), ty.clone()));
    let loops = info.loops.iter().map(|l| (l.name.clone(), l.ty.clone()));
    vars.chain(loops).collect()
}

/// Slots are numbered from 1: variables first, then loop variables. A name gets one slot, so
/// two loops that use the same name share it; the count is the number of unique variable and
/// loop names, inputs excluded (they are read with `LoadArg`).
fn assign_slots(types: &TypeInfo, feature: &str) -> BTreeMap<String, u32> {
    let mut slots = BTreeMap::new();
    if let Some(info) = types.features.get(feature) {
        let names = info
            .vars
            .iter()
            .map(|(name, ..)| name)
            .chain(info.loops.iter().map(|l| &l.name));
        for name in names {
            let next = slots.len() as u32 + 1;
            slots.entry(name.clone()).or_insert(next);
        }
    }
    slots
}

/// Lowers a checked program to an IR module, with the runtime it reaches.
pub fn lower(program: &Program, types: &TypeInfo, src: &Source) -> Result<Module, Diagnostic> {
    if program.tables().next().is_some() {
        return Err(Diagnostic::program("tables arrive in milestone 3"));
    }
    let mut module = Module::default();
    let mut called = vec!["_start".to_string()];
    for feature in program.features() {
        let f = lower_feature(&mut module, src, program, types, feature, &mut called)?;
        module.functions.push(f);
    }
    size_stack_checks(&mut module);
    runtime::add_reachable(&mut module, &called);
    Ok(module)
}
