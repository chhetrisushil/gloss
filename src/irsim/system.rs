//! The host primitives: output, input, memory and exit.

use super::files::{EISDIR, ENOTDIR};
use super::machine::{Ended, Machine, STACK_SIZE};
use crate::ir::{CWD, NOT_FOUND, OpenMode, Prim};

/// What `Write` and `Read` give for a file descriptor the program does not have.
const EBADF: i64 = -9;
/// What a `Write` that a test makes fail, and what such a `Read` gives.
const EIO: i64 = -5;

/// A descriptor `Open` handed out.
pub enum OpenFile {
    Read,
    Folder(String),
    Create,
    Probe,
}

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
            Prim::Exit => return Some(Ended::Exited(self.pop() as i32 & 0xff)),
            Prim::StackSize => self.a = STACK_SIZE,
            Prim::Open(mode) => self.open(mode),
            Prim::Close => {
                let fd = self.pop();
                self.a = match self.fds.remove(&fd) {
                    Some(_) => 0,
                    None if (0..3).contains(&fd) => 0,
                    None => EBADF,
                };
            }
            // Until files are synced and renamed, these fail.
            Prim::Rename => {
                (0..3).for_each(|_| _ = self.pop());
                self.a = NOT_FOUND;
            }
            Prim::Sync => {
                self.pop();
                self.a = NOT_FOUND;
            }
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

    /// Writes to `fd` 1 or 2, always in full. Unlike Linux it does not model SIGPIPE or a
    /// closed standard output: the design spec has a write to a closed pipe kill the
    /// program quietly, which only a real run shows.
    fn write(&mut self) {
        let (fd, addr, len) = self.pop_transfer();
        self.writes += 1;
        if fd != 1 && fd != 2 {
            self.a = EBADF;
            return;
        }
        if self.faults.failing_fds.contains(&fd) {
            self.a = EIO;
            return;
        }
        let len = len.min(self.faults.short_write.unwrap_or(len));
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
        if self.faults.failing_read {
            self.a = EIO;
            return;
        }
        let left = &self.stdin[self.stdin_pos..];
        let count = left.len().min(len as usize);
        // Like read(2), touch only the bytes copied.
        self.check(addr, count as u64, true);
        let left = &self.stdin[self.stdin_pos..];
        for (i, &byte) in left[..count].to_vec().iter().enumerate() {
            self.memory.write_byte(addr + i as u64, byte);
        }
        self.stdin_pos += count;
        self.a = count as i64;
    }

    /// Reads the NUL-terminated bytes at `addr` as a path.
    fn cstr(&mut self, addr: u64) -> String {
        let mut bytes = Vec::new();
        loop {
            self.check(addr + bytes.len() as u64, 1, false);
            match self.memory.read_byte(addr + bytes.len() as u64) {
                0 => return String::from_utf8_lossy(&bytes).into_owned(),
                byte => bytes.push(byte),
            }
        }
    }

    /// The path of the folder descriptor `dir`, which may be `CWD`.
    fn folder(&self, dir: i64) -> Result<&str, i64> {
        match self.fds.get(&dir) {
            _ if dir == CWD => Ok(""),
            Some(OpenFile::Folder(base)) => Ok(base),
            _ => Err(EBADF),
        }
    }

    fn open(&mut self, mode: OpenMode) {
        let path = self.pop() as u64;
        let dir = self.pop();
        let path = self.cstr(path);
        self.a = match self.open_path(dir, &path, mode) {
            Ok(file) => {
                let fd = (3..).find(|fd| !self.fds.contains_key(fd)).unwrap();
                self.fds.insert(fd, file);
                fd
            }
            Err(error) => error,
        };
    }

    fn open_path(&mut self, dir: i64, path: &str, mode: OpenMode) -> Result<OpenFile, i64> {
        let full = self.files.resolve(self.folder(dir)?, path)?;
        let is_file = self.files.files.contains_key(&full);
        let slash = path.ends_with('/');
        if is_file && slash {
            return Err(ENOTDIR);
        }
        match mode {
            OpenMode::Read if is_file => Ok(OpenFile::Read),
            OpenMode::Folder if is_file => Err(ENOTDIR),
            OpenMode::Folder if self.files.is_folder(&full) => Ok(OpenFile::Folder(full)),
            OpenMode::Probe if self.files.exists(&full) => Ok(OpenFile::Probe),
            OpenMode::Create if slash => Err(EISDIR),
            OpenMode::Create => self.files.create(&full).map(|()| OpenFile::Create),
            _ => Err(NOT_FOUND),
        }
    }
}
