//! The host primitives: output, input, memory and exit.

use super::machine::{Ended, Machine, STACK_SIZE};
use crate::ir::Prim;

/// What `Write` and `Read` give for a file descriptor the program does not have.
const EBADF: i64 = -9;

impl Machine<'_> {
    /// Runs a primitive: pops its arguments and leaves the result in `a`. Gives the end of
    /// the program when the primitive is `Exit`.
    pub fn system(&mut self, prim: Prim) -> Option<Ended> {
        match prim {
            Prim::Write => self.write(),
            Prim::Read => self.read(),
            Prim::Map => {
                let len = self.pop();
                self.a = self.memory.map(len) as i64;
            }
            Prim::Exit => return Some(Ended::Exited(self.pop() as i32)),
            Prim::StackSize => self.a = STACK_SIZE,
        }
        None
    }

    /// Pops `addr` and `len` of a transfer, whose `fd` was pushed first; gives `fd`, too.
    fn pop_transfer(&mut self) -> (i64, u64, u64) {
        let len = self.pop();
        let addr = self.pop() as u64;
        let fd = self.pop();
        assert!(len >= 0, "irsim: transfer of {len} bytes");
        (fd, addr, len as u64)
    }

    fn write(&mut self) {
        let (fd, addr, len) = self.pop_transfer();
        if fd != 1 && fd != 2 {
            self.a = EBADF;
            return;
        }
        self.check(addr, len, false);
        let bytes: Vec<u8> = (0..len).map(|i| self.memory.read_byte(addr + i)).collect();
        let out = if fd == 1 {
            &mut self.stdout
        } else {
            &mut self.stderr
        };
        out.extend(bytes);
        self.a = len as i64;
    }

    fn read(&mut self) {
        let (fd, addr, len) = self.pop_transfer();
        if fd != 0 {
            self.a = EBADF;
            return;
        }
        self.check(addr, len, true);
        let left = &self.stdin[self.stdin_pos..];
        let count = left.len().min(len as usize);
        for (i, &byte) in left[..count].to_vec().iter().enumerate() {
            self.memory.write_byte(addr + i as u64, byte);
        }
        self.stdin_pos += count;
        self.a = count as i64;
    }
}
