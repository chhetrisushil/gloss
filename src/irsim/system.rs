//! The host primitives: output, input, memory and exit.

use super::files::{EISDIR, ENOTDIR, join};
use super::machine::{Ended, Machine, STACK_SIZE};
use crate::ir::{CWD, ESCAPES, NOT_FOUND, OpenMode, Prim};
use std::{cell::RefCell, mem::take, rc::Rc};

/// What `Write` and `Read` give for a file descriptor the program does not have.
const EBADF: i64 = -9;
/// What `Wait` gives for a pid that is not a child: Linux's `-ECHILD`.
pub(super) const ECHILD: i64 = -10;
/// What a `Write` that a test makes fail, and what such a `Read` gives.
const EIO: i64 = -5;
/// What a `Fork` that a test makes fail gives: Linux's `-EAGAIN`.
const EAGAIN: i64 = -11;
/// What a test's `failing_open` gives: Linux's permission error.
const EACCES: i64 = -13;

/// The folder of a normalised path; empty for the current folder.
fn parent(path: &str) -> &str {
    path.rsplit_once('/').map_or("", |(folder, _)| folder)
}

/// A descriptor `Open` handed out.
#[derive(Clone)]
pub enum OpenFile {
    /// Read only, with how much has been read.
    Read(Target, usize),
    Folder(String),
    Create(Target),
    /// An `O_PATH` descriptor: with its name, so a folder can be a `dirfd`.
    Probe(String),
}

/// The file a descriptor is on. Like an inode, it follows the file when `Rename` moves it;
/// when `Rename` replaces it, the descriptors keep the old bytes, which no name reaches and
/// which they share.
#[derive(Clone)]
pub enum Target {
    Path(String),
    Detached(Rc<RefCell<Vec<u8>>>),
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
                    // Unlike Linux, closing 0-2 does nothing: a later `Write` still works and
                    // `Open` never gives these numbers out.
                    None if (0..3).contains(&fd) => 0,
                    None => EBADF,
                };
            }
            Prim::Sync => self.sync(),
            Prim::Mode => self.mode(),
            Prim::SetMode => self.set_mode(),
            Prim::Rename => self.rename(),
            Prim::Fork => self.fork(),
            // No signals are modelled, so there is nothing to reset.
            Prim::DefaultChildSignal => self.a = 0,
            Prim::Wait => {
                let pid = self.pop();
                assert!(
                    pid > 0,
                    "irsim: wait for a pid of 0 or less is not modelled"
                );
                let status = self.children.remove(&pid).map_or(ECHILD, i64::from);
                self.a = if self.faults.failing_wait {
                    ECHILD
                } else {
                    status
                };
            }
        }
        None
    }

    /// Runs a clone of the machine as the child until it exits; the parent then continues. The
    /// parent keeps its own memory, registers, frames and `fds`, but shares the output, the
    /// files, the input and its position and the `writes` and `file_reads` counters (a fault budget
    /// counts across processes, like `writes`), and the pid counter, with the child.
    ///
    /// Limits: the child always runs to completion first, so a case compared with a real
    /// executable must not write in the parent between `Fork` and `Wait` (that order is a race
    /// on Linux). There are no signals: a child fault panics the whole run, and `Wait` never
    /// gives `256 + signal`. A parent holding descriptors from `Open` panics, because real
    /// children share their offsets and follow renames, which `fds` does not model.
    fn fork(&mut self) {
        if self.faults.failing_fork {
            self.a = EAGAIN;
            return;
        }
        assert!(
            self.fds.is_empty(),
            "irsim: fork with open files is not modelled"
        );
        let pid = self.next_pid;
        self.next_pid += 1;
        // Move the shared buffers to the child rather than cloning them.
        let (stdout, stderr) = (take(&mut self.stdout), take(&mut self.stderr));
        let (files, stdin) = (take(&mut self.files), take(&mut self.stdin));
        let mut child = self.clone();
        (child.stdout, child.stderr, child.files, child.stdin) = (stdout, stderr, files, stdin);
        child.a = 0;
        // As on Linux, a child has no children of its own.
        child.children.clear();
        loop {
            match child.step() {
                None => {}
                Some(Ended::Exited(status)) => {
                    self.children.insert(pid, status);
                    break;
                }
                Some(Ended::Returned(_)) => {
                    panic!("irsim: a forked child returned from the program's entry")
                }
            }
        }
        self.stdout = child.stdout;
        self.stderr = child.stderr;
        self.files = child.files;
        self.stdin = child.stdin;
        self.writes = child.writes;
        self.file_reads = child.file_reads;
        self.stdin_pos = child.stdin_pos;
        self.next_pid = child.next_pid;
        self.a = pid;
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
            Some(OpenFile::Folder(path)) => Ok(path),
            Some(OpenFile::Probe(path)) if self.files.is_folder(path) => Ok(path),
            Some(_) => Err(ENOTDIR),
            // Standard input, output and error are open files, never folders.
            None if (0..3).contains(&dir) => Err(ENOTDIR),
            None => Err(EBADF),
        }
    }

    fn open(&mut self, mode: OpenMode) {
        let permissions = if mode == OpenMode::Create {
            self.pop()
        } else {
            0
        };
        let path = self.pop() as u64;
        let dir = self.pop();
        let path = self.cstr(path);
        self.a = match self.open_path(dir, &path, mode, permissions) {
            Ok(file) => {
                let fd = (3..).find(|fd| !self.fds.contains_key(fd)).unwrap();
                self.fds.insert(fd, file);
                fd
            }
            Err(error) => error,
        };
    }

    fn open_path(
        &mut self,
        dir: i64,
        path: &str,
        mode: OpenMode,
        permissions: i64,
    ) -> Result<OpenFile, i64> {
        // Linux rejects an absolute or empty path before it looks at the descriptor.
        if path.starts_with('/') {
            return Err(ESCAPES);
        }
        if path.is_empty() {
            return Err(NOT_FOUND);
        }
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
            // Linux opens a folder for reading, too; only `Read` on it fails.
            OpenMode::Read | OpenMode::Folder if self.files.is_folder(&full) => {
                Ok(OpenFile::Folder(full))
            }
            OpenMode::Probe if self.files.exists(&full) => Ok(OpenFile::Probe(full)),
            OpenMode::Create if slash => Err(EISDIR),
            OpenMode::Create if self.faults.read_only_folders.contains(parent(&full)) => {
                Err(EACCES)
            }
            OpenMode::Create => {
                let is_new = !is_file;
                self.files.create(&full)?;
                if is_new {
                    // The process's umask, 022, clears the group and other write bits.
                    self.files.set_mode(&full, permissions as u32 & 0o755);
                }
                Ok(OpenFile::Create(Target::Path(full)))
            }
            _ => Err(NOT_FOUND),
        }
    }

    /// `fstat`: the permission bits of what the descriptor is on, probes included.
    fn mode(&mut self) {
        let fd = self.pop();
        self.a = match self.fds.get(&fd) {
            Some(
                OpenFile::Read(Target::Path(path), _)
                | OpenFile::Folder(path)
                | OpenFile::Create(Target::Path(path))
                | OpenFile::Probe(path),
            ) => i64::from(self.files.mode_of(path)),
            Some(_) => 0o644,
            None => EBADF,
        };
    }

    /// `fchmod`: sets the permission bits of the file, exactly; no umask.
    fn set_mode(&mut self) {
        let permissions = self.pop();
        let fd = self.pop();
        self.a = match self.fds.get(&fd) {
            Some(OpenFile::Create(Target::Path(path))) => {
                let path = path.clone();
                self.files.set_mode(&path, permissions as u32 & 0o777);
                0
            }
            Some(_) => 0,
            None => EBADF,
        };
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
                self.files.log.push(format!("sync {path}"));
                0
            }
            // `fsync` on an `O_PATH` descriptor fails, so a probe cannot be synced.
            Some(OpenFile::Probe(_)) | None => EBADF,
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
        // Like Linux, an empty name is refused before the folder is looked at.
        if from.is_empty() || to.is_empty() {
            return Err(NOT_FOUND);
        }
        let folder = self.folder(dir)?.to_string();
        let (full_from, full_to) = (join(&folder, from), join(&folder, to));
        let replaced = self.files.files.get(&full_to).cloned();
        let moved = self.files.rename(&folder, from, to)?;
        self.files
            .log
            .push(format!("rename {full_from} -> {full_to}"));
        if full_from != full_to {
            self.follow(&full_from, &full_to, replaced, moved);
        }
        Ok(())
    }

    /// After `from` was renamed to `to`: descriptors on the file it replaced, if any, are
    /// detached with its bytes, and, when `from` was a file, those on it follow it.
    fn follow(&mut self, from: &str, to: &str, replaced: Option<Vec<u8>>, moved: bool) {
        let detached = replaced.map(|bytes| Rc::new(RefCell::new(bytes)));
        for file in self.fds.values_mut() {
            let (OpenFile::Read(target, _) | OpenFile::Create(target)) = file else {
                continue;
            };
            match target {
                Target::Path(path) if path == to => {
                    if let Some(bytes) = &detached {
                        *target = Target::Detached(bytes.clone());
                    }
                }
                Target::Path(path) if moved && path == from => *path = to.to_string(),
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
        let len = len.min(self.faults.short_file_write.unwrap_or(len));
        self.check(addr, len, false);
        let bytes: Vec<u8> = (0..len).map(|i| self.memory.read_byte(addr + i)).collect();
        if let Some(OpenFile::Create(target)) = self.fds.get(&fd) {
            match target {
                Target::Path(path) => self
                    .files
                    .files
                    .entry(path.clone())
                    .or_default()
                    .extend(bytes),
                Target::Detached(content) => content.borrow_mut().extend(bytes),
            }
        }
        self.a = len as i64;
    }

    fn read_file(&mut self, fd: i64, addr: u64, len: u64) {
        let (target, pos) = match self.fds.get(&fd) {
            Some(OpenFile::Read(target, pos)) => (target, *pos),
            Some(OpenFile::Folder(_)) => {
                self.a = EISDIR;
                return;
            }
            _ => {
                self.a = EBADF;
                return;
            }
        };
        self.file_reads += 1;
        if self.faults.failing_file_read == Some(self.file_reads) {
            self.a = EIO;
            return;
        }
        let content = match target {
            Target::Path(path) => self.files.files.get(path).cloned().unwrap_or_default(),
            Target::Detached(content) => content.borrow().clone(),
        };
        let chunk: Vec<u8> = content
            .iter()
            .skip(pos)
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
