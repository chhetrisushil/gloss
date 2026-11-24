//! The in-memory file system the file primitives work on. Paths are relative and
//! normalised: components joined by `/`, the current folder is the empty path.

use crate::ir::{ESCAPES, LINK, NOT_FOUND};
use std::collections::{BTreeMap, BTreeSet};

/// What Linux gives for a file used as a folder; not in the IR's vocabulary.
pub const ENOTDIR: i64 = -20;
/// What Linux gives for creating a file where a folder is.
pub const EISDIR: i64 = -21;

/// The files a program sees, and what it did to them.
#[derive(Clone, Default, Debug, PartialEq, Eq)]
pub struct Files {
    /// File contents, by path.
    pub files: BTreeMap<String, Vec<u8>>,
    /// Folders other than the current one, which always exists.
    pub folders: BTreeSet<String>,
    /// Paths that are symbolic links; whatever they point to is never reached.
    pub links: BTreeSet<String>,
    /// The paths passed to `Sync`, in order.
    pub synced: Vec<String>,
}

impl Files {
    pub fn is_folder(&self, path: &str) -> bool {
        path.is_empty() || self.folders.contains(path)
    }

    /// Whether `path` names a file or a folder.
    pub fn exists(&self, path: &str) -> bool {
        self.files.contains_key(path) || self.is_folder(path)
    }

    /// Resolves `path` beneath the folder `base` without following links, as `openat2` with
    /// `RESOLVE_BENEATH | RESOLVE_NO_SYMLINKS` does: an absolute path or a `..` that leaves
    /// `base` gives `ESCAPES`, a link at any depth gives `LINK`, and a missing parent gives
    /// `NOT_FOUND` and a file used as a folder gives `ENOTDIR`. An empty path gives
    /// `NOT_FOUND`; `.` and a trailing `/` name the folder itself. Gives the full
    /// normalised path.
    pub fn resolve(&self, base: &str, path: &str) -> Result<String, i64> {
        if path.starts_with('/') {
            return Err(ESCAPES);
        }
        if path.is_empty() {
            return Err(NOT_FOUND);
        }
        let mut parts: Vec<&str> = base.split('/').filter(|c| !c.is_empty()).collect();
        let floor = parts.len();
        for component in path.split('/') {
            match component {
                "" | "." => {}
                ".." => {
                    if parts.len() == floor {
                        return Err(ESCAPES);
                    }
                    self.need_folder(&parts.join("/"))?;
                    parts.pop();
                }
                name => {
                    self.need_folder(&parts.join("/"))?;
                    parts.push(name);
                    if self.links.contains(&parts.join("/")) {
                        return Err(LINK);
                    }
                }
            }
        }
        Ok(parts.join("/"))
    }

    fn need_folder(&self, path: &str) -> Result<(), i64> {
        if self.is_folder(path) {
            Ok(())
        } else if self.files.contains_key(path) {
            Err(ENOTDIR)
        } else {
            Err(NOT_FOUND)
        }
    }

    /// Creates the file `path` empty, or empties it; its folder must exist and `path` must
    /// not be a folder.
    pub fn create(&mut self, path: &str) -> Result<(), i64> {
        self.need_folder(parent(path))?;
        if self.is_folder(path) {
            return Err(EISDIR);
        }
        self.files.insert(path.to_string(), Vec::new());
        Ok(())
    }
}

/// The folder a normalised path is in.
fn parent(path: &str) -> &str {
    path.rsplit_once('/').map_or("", |(folder, _)| folder)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample() -> Files {
        let mut f = Files::default();
        f.folders.extend(["d".to_string(), "d/e".to_string()]);
        f.links.extend(["l".to_string(), "d/l".to_string()]);
        f.files.insert("d/e/x".to_string(), b"x".to_vec());
        f
    }

    #[test]
    fn paths_are_normalised_and_stay_beneath_the_folder() {
        let f = sample();
        assert_eq!(f.resolve("", "d/./e//x"), Ok("d/e/x".to_string()));
        assert_eq!(f.resolve("d", "e/../e/x"), Ok("d/e/x".to_string()));
        assert_eq!(f.resolve("d", "../x"), Err(ESCAPES));
        assert_eq!(f.resolve("", "d/../../x"), Err(ESCAPES));
        assert_eq!(f.resolve("d", "/d/e"), Err(ESCAPES));
        assert_eq!(f.resolve("", "d/gone/x"), Err(NOT_FOUND));
    }

    #[test]
    fn a_link_at_any_depth_is_refused() {
        let f = sample();
        assert_eq!(f.resolve("", "l"), Err(LINK));
        assert_eq!(f.resolve("", "l/x"), Err(LINK));
        assert_eq!(f.resolve("", "d/l/x"), Err(LINK));
        assert_eq!(f.resolve("d", "l"), Err(LINK));
    }

    #[test]
    fn create_truncates_and_needs_its_folder() {
        let mut f = sample();
        assert_eq!(f.create("d/e/x"), Ok(()));
        assert_eq!(f.files["d/e/x"], Vec::<u8>::new());
        assert_eq!(f.create("nope/x"), Err(NOT_FOUND));
    }

    #[test]
    fn a_file_used_as_a_folder_is_enotdir_and_a_folder_cannot_be_created_over() {
        let f = sample();
        assert_eq!(f.resolve("", "d/e/x/y"), Err(ENOTDIR));
        let mut f = f;
        assert_eq!(f.create("d/e/x/y"), Err(ENOTDIR));
        assert_eq!(f.create("d/e"), Err(EISDIR));
    }

    #[test]
    fn empty_dot_and_trailing_slash_paths() {
        let f = sample();
        assert_eq!(f.resolve("d", ""), Err(NOT_FOUND));
        assert_eq!(f.resolve("d", "."), Ok("d".to_string()));
        assert_eq!(f.resolve("", "d/e/"), Ok("d/e".to_string()));
        assert_eq!(f.resolve("", "."), Ok(String::new()));
    }
}
