//! The host primitives: output, input, memory and exit.

use super::files::{EISDIR, ENOTDIR, join};
use super::machine::{Ended, Machine, STACK_SIZE};
use crate::ir::{CWD, NOT_FOUND, OpenMode, Prim};

/// What `Write` and `Read` give for a file descriptor the program does not have.
const EBADF: i64 = -9;
/// What a `Write` that a test makes fail, and what such a `Read` gives.
const EIO: i64 = -5;
/// What a test's `failing_open` gives: Linux's permission error.
const EACCES: i64 = -13;

/// A descriptor `Open` handed out.
pub enum OpenFile {
    /// Read only, with how much has been read.
    Read(Target, usize),
    Folder(String),
    Create(Target),
    Probe,
}

/// The file a descriptor is on. Like an inode, it follows the file when `Rename` moves it;
/// when `Rename` replaces it, the descriptor keeps the old bytes, which no name reaches.
pub enum Target {
    Path(String),
    Detached(Vec<u8>),
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
            Prim::Sync => self.sync(),
            Prim::Rename => self.rename(),
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
        if fd >= 3 {
            self.write_file(fd, addr, len);
            return;
        }
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
        if fd >= 3 {
            self.read_file(fd, addr, len);
            return;
        }
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
        if self.faults.failing_open.as_deref() == Some(&full) {
            return Err(EACCES);
        }
        let is_file = self.files.files.contains_key(&full);
        let slash = path.ends_with('/');
        if is_file && slash {
            return Err(ENOTDIR);
        }
        match mode {
            OpenMode::Read if is_file => Ok(OpenFile::Read(Target::Path(full), 0)),
            OpenMode::Folder if is_file => Err(ENOTDIR),
            OpenMode::Folder if self.files.is_folder(&full) => Ok(OpenFile::Folder(full)),
            OpenMode::Probe if self.files.exists(&full) => Ok(OpenFile::Probe),
            OpenMode::Create if slash => Err(EISDIR),
            OpenMode::Create => self
                .files
                .create(&full)
                .map(|()| OpenFile::Create(Target::Path(full))),
            _ => Err(NOT_FOUND),
        }
    }

    fn sync(&mut self) {
        let fd = self.pop();
        self.a = match self.fds.get(&fd) {
            Some(
                OpenFile::Read(Target::Path(path), _)
                | OpenFile::Folder(path)
                | OpenFile::Create(Target::Path(path)),
            ) => {
                self.files.synced.push(path.clone());
                0
            }
            // `fsync` on an `O_PATH` descriptor fails, so a probe cannot be synced.
            Some(OpenFile::Probe) | None => EBADF,
            Some(_) => 0,
        };
    }

    /// Like `renameat` on a folder descriptor: both names are in that folder. Open
    /// descriptors follow their file to its new name.
    fn rename(&mut self) {
        let to = self.pop() as u64;
        let from = self.pop() as u64;
        let dir = self.pop();
        let (from, to) = (self.cstr(from), self.cstr(to));
        self.a = match self.rename_in(dir, &from, &to) {
            Ok(()) => 0,
            Err(error) => error,
        };
    }

    fn rename_in(&mut self, dir: i64, from: &str, to: &str) -> Result<(), i64> {
        let folder = self.folder(dir)?.to_string();
        let (full_from, full_to) = (join(&folder, from), join(&folder, to));
        let replaced = self.files.files.get(&full_to).cloned();
        if self.files.rename(&folder, from, to)? && full_from != full_to {
            self.follow(&full_from, &full_to, replaced);
        }
        Ok(())
    }

    /// After the file `from` moved to `to`: descriptors on the file it replaced are detached
    /// with its bytes, and those on `from` follow it.
    fn follow(&mut self, from: &str, to: &str, replaced: Option<Vec<u8>>) {
        for file in self.fds.values_mut() {
            let (OpenFile::Read(target, _) | OpenFile::Create(target)) = file else {
                continue;
            };
            match target {
                Target::Path(path) if path == to => {
                    if let Some(bytes) = &replaced {
                        *target = Target::Detached(bytes.clone());
                    }
                }
                Target::Path(path) if path == from => *path = to.to_string(),
                _ => {}
            }
        }
    }

    fn write_file(&mut self, fd: i64, addr: u64, len: u64) {
        if !matches!(self.fds.get(&fd), Some(OpenFile::Create(_))) {
            self.a = EBADF;
            return;
        }
        if self.faults.failing_file_write {
            self.a = EIO;
            return;
        }
        self.check(addr, len, false);
        let bytes: Vec<u8> = (0..len).map(|i| self.memory.read_byte(addr + i)).collect();
        if let Some(OpenFile::Create(target)) = self.fds.get_mut(&fd) {
            match target {
                Target::Path(path) => self.files.files.entry(path.clone()).or_default(),
                Target::Detached(content) => content,
            }
            .extend(bytes);
        }
        self.a = len as i64;
    }

    fn read_file(&mut self, fd: i64, addr: u64, len: u64) {
        let Some(OpenFile::Read(target, pos)) = self.fds.get(&fd) else {
            self.a = EBADF;
            return;
        };
        let content = match target {
            Target::Path(path) => self.files.files.get(path).map_or(&[][..], Vec::as_slice),
            Target::Detached(content) => content,
        };
        let chunk: Vec<u8> = content
            .iter()
            .skip(*pos)
            .take(len as usize)
            .copied()
            .collect();
        self.check(addr, chunk.len() as u64, true);
        for (i, &byte) in chunk.iter().enumerate() {
            self.memory.write_byte(addr + i as u64, byte);
        }
        if let Some(OpenFile::Read(_, pos)) = self.fds.get_mut(&fd) {
            *pos += chunk.len();
        }
        self.a = chunk.len() as i64;
    }
}
