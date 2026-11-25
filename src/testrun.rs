//! Running a test executable on the host: a private folder, streamed output, one exit status.
//! This is host code, not generated code, so it may decode an `ExitStatus`.

use std::os::unix::fs::DirBuilderExt;
use std::os::unix::process::ExitStatusExt;
use std::path::{Path, PathBuf};
use std::process::ExitStatus;
use std::sync::atomic::{AtomicU32, Ordering};

/// Whether this host can run the executables the compiler builds.
pub fn host_runs_tests() -> bool {
    host_can_run(std::env::consts::OS, std::env::consts::ARCH)
}

/// The decision behind `host_runs_tests`, for any operating system and processor.
fn host_can_run(os: &str, arch: &str) -> bool {
    os == "linux" && arch == "x86_64"
}

/// A fresh private folder (mode 0700) under the system temporary folder, removed on drop.
pub struct PrivateDir {
    path: PathBuf,
}

static COUNTER: AtomicU32 = AtomicU32::new(0);

impl PrivateDir {
    /// Creates the folder under a name no other folder has.
    pub fn new() -> std::io::Result<PrivateDir> {
        Self::create_in(
            &std::env::temp_dir(),
            &mut std::iter::from_fn(|| Some(candidate_name())),
        )
    }

    /// Creates the folder in `base` under the first of `names` that is free.
    fn create_in(
        base: &Path,
        names: &mut dyn Iterator<Item = String>,
    ) -> std::io::Result<PrivateDir> {
        for name in names {
            let path = base.join(name);
            match std::fs::DirBuilder::new().mode(0o700).create(&path) {
                Ok(()) => return Ok(PrivateDir { path }),
                Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => {}
                Err(e) => return Err(e),
            }
        }
        Err(std::io::Error::other("no folder name was free"))
    }

    pub fn path(&self) -> &Path {
        &self.path
    }
}

/// A folder name that another process cannot predict: the process id, the time in
/// nanoseconds, a counter and a random number (std's randomly keyed hasher).
fn candidate_name() -> String {
    use std::hash::{BuildHasher, Hasher};
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_nanos());
    let n = COUNTER.fetch_add(1, Ordering::Relaxed);
    let random = std::collections::hash_map::RandomState::new()
        .build_hasher()
        .finish();
    format!(
        "gloss-test-{}-{nanos}-{n}-{random:016x}",
        std::process::id()
    )
}

impl Drop for PrivateDir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.path);
    }
}

/// 0 and 1 are the test run's own; any other end is 1 with a message.
pub fn status_for(ended: ExitStatus) -> (i32, Option<String>) {
    let code = match (ended.code(), ended.signal()) {
        (Some(code), _) => code,
        (None, Some(signal)) => 256 + signal,
        (None, None) => 256,
    };
    match code {
        0 | 1 => (code, None),
        n => (
            1,
            Some(format!("the test run ended without finishing (status {n})")),
        ),
    }
}

#[cfg(test)]
mod tests;
