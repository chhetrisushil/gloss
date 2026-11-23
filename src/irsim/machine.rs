//! The interpreter's state: registers, the stack and the call frames.

use super::memory::Memory;
use crate::ir::{Cond, Data, Function, Global, Inst, Label, Module, Reg};
use std::collections::HashMap;

/// The top of the simulated stack, which grows down.
const STACK_TOP: u64 = 0x7fff_0000_0000;

/// The soft stack limit `Prim::StackSize` reports.
pub const STACK_SIZE: i64 = 8 * 1024 * 1024;

/// How the program stopped.
pub enum Ended {
    /// The outermost function returned this result.
    Returned(i64),
    /// `Prim::Exit` ran with this status.
    Exited(i32),
}

/// One active call.
pub struct Frame {
    pub func: usize,
    pub pc: usize,
    pub args: Vec<i64>,
    pub slots: Vec<i64>,
    /// The caller's `b`, which survives the call.
    saved_b: i64,
    /// The stack pointer once the arguments are dropped.
    return_sp: u64,
}

pub struct Machine<'m> {
    pub module: &'m Module,
    pub memory: Memory,
    /// Each function's label positions, by function index.
    labels: Vec<HashMap<Label, usize>>,
    frames: Vec<Frame>,
    pub sp: u64,
    pub a: i64,
    pub b: i64,
    /// The operands of a `Compare` the current instruction may use.
    pub compared: Option<(i64, i64)>,
    /// The operands of the `Compare` just run, usable by the next instruction.
    pub fresh: Option<(i64, i64)>,
    pub stdin: Vec<u8>,
    /// How much of `stdin` `Prim::Read` has handed out.
    pub stdin_pos: usize,
    pub stdout: Vec<u8>,
    pub stderr: Vec<u8>,
}

fn label_positions(function: &Function) -> HashMap<Label, usize> {
    let mut positions = HashMap::new();
    for (i, inst) in function.body.iter().enumerate() {
        if let Inst::Mark(label) = inst {
            positions.insert(*label, i);
        }
    }
    positions
}

impl<'m> Machine<'m> {
    pub fn new(module: &'m Module) -> Machine<'m> {
        Machine {
            module,
            memory: Memory::new(module),
            labels: module.functions.iter().map(label_positions).collect(),
            frames: Vec::new(),
            sp: STACK_TOP,
            a: 0,
            b: 0,
            compared: None,
            fresh: None,
            stdin: Vec::new(),
            stdin_pos: 0,
            stdout: Vec::new(),
            stderr: Vec::new(),
        }
    }

    pub fn push(&mut self, value: i64) {
        self.sp -= 8;
        self.memory.write(self.sp, value);
    }

    /// Panics unless the `len` bytes at `addr` may be read, or written when `write` is set.
    pub fn check(&self, addr: u64, len: u64, write: bool) {
        let in_stack = addr >= self.sp && addr.checked_add(len).is_some_and(|e| e <= STACK_TOP);
        if !in_stack && !self.memory.allows(addr, len, write) {
            let what = if write { "write" } else { "read" };
            panic!("irsim: {what} of {len} bytes at {addr:#x} is outside mapped memory");
        }
    }

    /// The address `base` plus `offset` names.
    pub fn effective(&self, base: Reg, offset: i32) -> u64 {
        (self.reg(base) as u64).wrapping_add(offset as i64 as u64)
    }

    /// Whether a callee frame of `bytes` would lie below `Global::StackLimit` (unsigned).
    pub fn stack_overflows(&self, bytes: u32) -> bool {
        let limit = self
            .memory
            .read(self.memory.address_of(Data::Global(Global::StackLimit)));
        self.sp.wrapping_sub(bytes as u64) < limit as u64
    }

    pub fn pop(&mut self) -> i64 {
        let value = self.memory.read(self.sp);
        self.sp += 8;
        value
    }

    pub fn reg(&self, r: Reg) -> i64 {
        match r {
            Reg::A => self.a,
            Reg::B => self.b,
        }
    }

    pub fn holds(&self, cond: Cond) -> bool {
        let Some((a, b)) = self.compared else {
            panic!("irsim: JumpIf and SetIf must directly follow Compare");
        };
        match cond {
            Cond::Eq => a == b,
            Cond::Ne => a != b,
            Cond::Lt => a < b,
            Cond::Le => a <= b,
            Cond::Gt => a > b,
            Cond::Ge => a >= b,
        }
    }

    pub fn set(&mut self, r: Reg, value: i64) {
        match r {
            Reg::A => self.a = value,
            Reg::B => self.b = value,
        }
    }

    pub fn frame(&mut self) -> &mut Frame {
        self.frames.last_mut().expect("no active frame")
    }

    pub fn jump(&mut self, label: Label) {
        let func = self.frame().func;
        let target = self.labels[func][&label];
        self.frame().pc = target;
    }

    fn function_index(&self, name: &str) -> usize {
        match self.module.functions.iter().position(|f| f.name == name) {
            Some(index) => index,
            None => panic!("irsim: no function named {name}"),
        }
    }

    /// Starts a call to the function `name`, whose arguments are already pushed.
    pub fn enter(&mut self, name: &str) {
        let func = self.function_index(name);
        let f = &self.module.functions[func];
        let params = f.params as u64;
        let args = (1..=params)
            .map(|i| self.memory.read(self.sp + 8 * (params - i)))
            .collect();
        let return_sp = self.sp + 8 * params;
        // A return address, a saved frame pointer and a saved `b`, then the slots: the x86
        // frame of ruling M2-R12, so `StackPointer` agrees with hardware.
        self.sp -= 8 * (f.slots as u64 + 3);
        self.frames.push(Frame {
            func,
            pc: 0,
            args,
            slots: vec![0; f.slots as usize],
            saved_b: self.b,
            return_sp,
        });
    }

    /// Pops the current frame; gives the result when it was the outermost.
    pub fn leave(&mut self) -> Option<i64> {
        let frame = self.frames.pop().expect("no active frame");
        self.sp = frame.return_sp;
        self.b = frame.saved_b;
        self.frames.is_empty().then_some(self.a)
    }

    /// Runs `name` with `args` pushed left to right, until it returns or exits.
    pub fn execute(&mut self, name: &str, args: &[i64]) -> Ended {
        for &arg in args {
            self.push(arg);
        }
        let func = self.function_index(name);
        assert_eq!(
            self.module.functions[func].params as usize,
            args.len(),
            "irsim: {name} called with the wrong number of arguments"
        );
        self.enter(name);
        loop {
            if let Some(ended) = self.step() {
                return ended;
            }
        }
    }
}
