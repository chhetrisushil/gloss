//! The interpreter's state: registers, the stack and the call frames.

use super::memory::Memory;
use crate::ir::{Cond, Module, Reg};

/// The top of the simulated stack, which grows down.
const STACK_TOP: u64 = 0x7fff_0000_0000;

/// One active call.
pub struct Frame {
    pub func: usize,
    pub pc: usize,
    pub slots: Vec<i64>,
    /// The caller's `b`, which survives the call.
    saved_b: i64,
    /// The stack pointer once the arguments are dropped.
    return_sp: u64,
}

pub struct Machine<'m> {
    pub module: &'m Module,
    memory: Memory,
    frames: Vec<Frame>,
    sp: u64,
    pub a: i64,
    pub b: i64,
    /// The operands of the last `Compare`.
    pub compared: (i64, i64),
}

impl<'m> Machine<'m> {
    pub fn new(module: &'m Module) -> Machine<'m> {
        Machine {
            module,
            memory: Memory::new(module),
            frames: Vec::new(),
            sp: STACK_TOP,
            a: 0,
            b: 0,
            compared: (0, 0),
        }
    }

    pub fn push(&mut self, value: i64) {
        self.sp -= 8;
        self.memory.write(self.sp, value);
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
        let (a, b) = self.compared;
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
        let return_sp = self.sp + 8 * f.params as u64;
        // A return address and a saved frame pointer, as on hardware.
        self.sp -= 8 * (f.slots as u64 + 2);
        self.frames.push(Frame {
            func,
            pc: 0,
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

    /// Runs `name` with `args` pushed left to right, until it returns.
    pub fn execute(&mut self, name: &str, args: &[i64]) -> i64 {
        for &arg in args {
            self.push(arg);
        }
        self.enter(name);
        loop {
            if let Some(result) = self.step() {
                return result;
            }
        }
    }
}
