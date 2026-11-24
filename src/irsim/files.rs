//! The in-memory file system the file primitives work on. Paths are relative and
//! normalised: components joined by `/`, the current folder is the empty path.

use crate::ir::{ESCAPES, LINK, NOT_FOUND};
use std::collections::{BTreeMap, BTreeSet};

/// What Linux gives for a file used as a folder; not in the IR's vocabulary.
pub const ENOTDIR: i64 = -20;
/// What Linux gives for creating a file where a folder is.
pub const EISDIR: i64 = -21;
/// What Linux gives for renaming `.` or `..`.
pub const EBUSY: i64 = -16;
/// What Linux gives for a bad argument; here a name that is not one component.
pub const EINVAL: i64 = -22;

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
    /// `sync <path>` and `rename <from> -> <to>`, in the order they happened.
    pub log: Vec<String>,
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
                // A file followed by `/` or `/.` is not a folder, and `.` must exist.
                "." => self.need_folder(&parts.join("/"))?,
                "" if self.files.contains_key(&parts.join("/")) => return Err(ENOTDIR),
                "" => {}
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

    /// Like `renameat` in the folder `folder`, which is not resolved again: `from` and `to`
    /// are single names, links are moved and replaced, never followed, and a file replaces
    /// a file or a link. Gives whether a file (not a link) moved.
    pub fn rename(&mut self, folder: &str, from: &str, to: &str) -> Result<bool, i64> {
        if from.is_empty() || to.is_empty() {
            return Err(NOT_FOUND);
        }
        if [from, to].iter().any(|n| *n == "." || *n == "..") {
            return Err(EBUSY);
        }
        if [from, to].iter().any(|n| n.contains('/')) {
            return Err(EINVAL);
        }
        let (from, to) = (join(folder, from), join(folder, to));
        if self.folders.contains(&from) {
            panic!("irsim: rename of a folder is not modelled");
        }
        if self.folders.contains(&to) {
            return Err(EISDIR);
        }
        if let Some(bytes) = self.files.remove(&from) {
            self.links.remove(&to);
            self.files.insert(to, bytes);
            Ok(true)
        } else if self.links.remove(&from) {
            self.files.remove(&to);
            self.links.insert(to);
            Ok(false)
        } else {
            Err(NOT_FOUND)
        }
    }
}

/// The path of `name` in the folder `folder`.
pub fn join(folder: &str, name: &str) -> String {
    if folder.is_empty() {
        name.to_string()
    } else {
        format!("{folder}/{name}")
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
    fn a_file_followed_by_slash_or_dot_is_enotdir() {
        let f = sample();
        assert_eq!(f.resolve("", "d/e/x/."), Err(ENOTDIR));
        assert_eq!(f.resolve("", "d/e/x/"), Err(ENOTDIR));
        assert_eq!(f.resolve("", "d/e/x/.."), Err(ENOTDIR));
        assert_eq!(f.resolve("", "d/e/gone/."), Err(NOT_FOUND));
    }

    #[test]
    fn empty_dot_and_trailing_slash_paths() {
        let f = sample();
        assert_eq!(f.resolve("d", ""), Err(NOT_FOUND));
        assert_eq!(f.resolve("d", "."), Ok("d".to_string()));
        assert_eq!(f.resolve("", "d/e/"), Ok("d/e".to_string()));
        assert_eq!(f.resolve("", "."), Ok(String::new()));
    }

    #[test]
    fn rename_replaces_files_and_links_and_moves_links() {
        let mut f = sample();
        f.files.insert("d/y".to_string(), b"y".to_vec());
        assert_eq!(f.rename("d/e", "x", "x2"), Ok(true));
        assert_eq!(f.rename("d", "y", "l"), Ok(true));
        assert_eq!(f.files["d/l"], b"y".to_vec());
        assert!(!f.links.contains("d/l"));
        assert_eq!(f.rename("", "l", "m"), Ok(false));
        assert!(f.links.contains("m") && !f.links.contains("l"));
    }

    #[test]
    fn rename_takes_single_names_of_files_and_not_onto_folders() {
        let mut f = sample();
        assert_eq!(f.rename("d", "e/x", "z"), Err(EINVAL));
        assert_eq!(f.rename("d/e", "x", ".."), Err(EBUSY));
        assert_eq!(f.rename("d/e", ".", "x"), Err(EBUSY));
        f.files.insert("t".to_string(), Vec::new());
        assert_eq!(f.rename("", "t", "d"), Err(EISDIR));
    }

    #[test]
    #[should_panic(expected = "rename of a folder is not modelled")]
    fn renaming_a_folder_is_not_modelled() {
        let _ = sample().rename("", "d", "z");
    }
}
