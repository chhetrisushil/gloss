//! Saving a table: write a temporary file beside it, then rename it over the table.

use super::asm::Asm;
use crate::ir::Cond::{Eq, Lt, Ne};
use crate::ir::Reg::{A, B};
use crate::ir::{CWD, ESCAPES, Function, Inst::*, LINK, Label, NOT_FOUND, OpenMode, Prim};

/// `save_fail(line, shown, folder, code, in_folder)`: failure 7 for the table file `shown`
/// (its path, in display form), whose folder `folder` is in display form too. A symbolic link
/// (`LINK`, or `ESCAPES` for a path that leaves the folder) is named; so is a folder that is
/// missing, when `in_folder` is 1 and `code` is `NOT_FOUND`; any other error is "cannot be
/// written".
fn save_fail() -> Function {
    let mut s = Asm::default();
    let (link, end) = (s.label(), s.label());
    s.push_arg(1);
    s.push_const(7);
    s.call("fail_begin");
    s.push_arg(2);
    s.call("err_text");
    for code in [LINK, ESCAPES] {
        s.i(LoadArg(A, 4));
        s.i(Const(B, code));
        s.jump_if(Eq, link);
    }
    s.push_text(super::CANNOT_WRITE);
    s.call("err_text");
    s.i(LoadArg(A, 4));
    s.i(Const(B, NOT_FOUND));
    s.jump_if(Ne, end);
    s.i(LoadArg(A, 5));
    s.i(Const(B, 0));
    s.jump_if(Eq, end);
    s.push_text(super::ITS_FOLDER);
    s.call("err_text");
    s.push_arg(3);
    s.call("err_text");
    s.push_text(super::NOT_EXIST);
    s.call("err_text");
    s.i(Jump(end));
    s.i(Mark(link));
    s.push_text(super::THROUGH_LINK);
    s.call("err_text");
    s.i(Mark(end));
    s.call("fail_end");
    s.finish("save_fail", 5, 0)
}

// Slots of `table_save`.
const TEXT: u32 = 1;
const DIR: u32 = 2;
const FILE: u32 = 3;
const IN_FOLDER: u32 = 4;
const CODE: u32 = 5;

/// Pushes the address of the NUL-terminated bytes of the text argument `n`.
fn push_path(s: &mut Asm, n: u32) {
    s.i(LoadArg(A, n));
    s.i(AddImm(A, 8));
    s.i(Push(A));
}

/// With the result of a primitive in `a`: jumps to `fail` when it is an error.
fn failed(s: &mut Asm, fail: Label) {
    s.i(Const(B, 0));
    s.jump_if(Lt, fail);
}

/// `System(prim)` on the descriptor in `slot`, ending at `fail` on an error.
fn on_slot(s: &mut Asm, prim: Prim, slot: u32, fail: Label) {
    s.push_slot(slot);
    s.i(System(prim));
    failed(s, fail);
}

/// `table_save(list, desc, folder, name, temp, shown, folder_shown, line)`: writes the table
/// `list` (of records `desc`) as CSV. `folder`, `name` and `temp` are NUL-terminated texts,
/// the temporary name being `name` and `.gloss-new`; `shown` and `folder_shown` are the
/// display forms for failure 7. It opens the folder, looks at `name` (a symbolic link there is
/// refused), writes `temp` and syncs it, renames it over `name`, then syncs the folder. A
/// failure before the rename leaves `name` untouched. Slots: the text, the folder, the file
/// descriptor, whether the folder is being opened, the error.
fn table_save() -> Function {
    let mut s = Asm::default();
    let (fail, created, closed) = (s.label(), s.label(), s.label());
    s.push_arg(1);
    s.push_arg(2);
    s.push_arg(8);
    s.call("csv_text");
    s.i(StoreSlot(TEXT, A));
    s.i(Const(A, 1));
    s.i(StoreSlot(IN_FOLDER, A));
    s.push_const(CWD);
    push_path(&mut s, 3);
    s.i(System(Prim::Open(OpenMode::Folder)));
    s.i(StoreSlot(DIR, A));
    failed(&mut s, fail);
    s.i(Const(A, 0));
    s.i(StoreSlot(IN_FOLDER, A));
    s.push_slot(DIR);
    push_path(&mut s, 4);
    s.i(System(Prim::Open(OpenMode::Probe)));
    for code in [LINK, ESCAPES] {
        s.i(Const(B, code));
        s.jump_if(Eq, fail);
    }
    s.i(Const(B, 0));
    s.jump_if(Lt, created);
    s.i(Push(A));
    s.i(System(Prim::Close));
    s.i(Mark(created));
    s.push_slot(DIR);
    push_path(&mut s, 5);
    s.i(System(Prim::Open(OpenMode::Create)));
    s.i(StoreSlot(FILE, A));
    failed(&mut s, fail);
    s.push_slot(FILE);
    s.i(LoadSlot(A, TEXT));
    s.i(AddImm(A, 8));
    s.i(Push(A));
    s.i(LoadSlot(A, TEXT));
    s.i(Load {
        to: A,
        base: A,
        offset: 0,
    });
    s.i(Push(A));
    s.call("file_write");
    failed(&mut s, fail);
    on_slot(&mut s, Prim::Sync, FILE, fail);
    on_slot(&mut s, Prim::Close, FILE, fail);
    s.push_slot(DIR);
    push_path(&mut s, 5);
    push_path(&mut s, 4);
    s.i(System(Prim::Rename));
    failed(&mut s, fail);
    on_slot(&mut s, Prim::Sync, DIR, fail);
    on_slot(&mut s, Prim::Close, DIR, fail);
    s.i(Jump(closed));
    s.i(Mark(fail));
    s.i(StoreSlot(CODE, A));
    s.push_arg(8);
    s.push_arg(6);
    s.push_arg(7);
    s.push_slot(CODE);
    s.push_slot(IN_FOLDER);
    s.call("save_fail");
    s.i(Mark(closed));
    s.finish("table_save", 8, 5)
}

pub fn routines() -> Vec<Function> {
    vec![save_fail(), table_save()]
}

#[cfg(test)]
mod tests {
    use super::super::tests::{program, run};
    use crate::ir::Inst::*;
    use crate::ir::{Data, ESCAPES, LINK, NOT_FOUND, Reg::A};

    /// What `save_fail` does for the error `code`, in the folder step or not: the status and
    /// the error output. The file is `data/e.csv`, in the folder `data`.
    fn failed(code: i64, in_folder: i64) -> (i32, String) {
        let mut body = vec![Const(A, 7), Push(A)];
        for text in [0, 1] {
            body.extend([AddressOf(A, Data::Text(text)), Push(A)]);
        }
        body.extend([Const(A, code), Push(A), Const(A, in_folder), Push(A)]);
        body.push(Call("save_fail".into()));
        let o = run(&program(body, &[b"data/e.csv", b"data"]));
        (o.status, String::from_utf8(o.stderr).unwrap())
    }

    #[test]
    fn a_missing_folder_is_named_only_in_the_folder_step() {
        let named =
            "line 7: failure 7: data/e.csv cannot be written (its folder data does not exist)\n";
        assert_eq!(failed(NOT_FOUND, 1), (2, named.to_string()));
        let plain = "line 7: failure 7: data/e.csv cannot be written\n";
        assert_eq!(failed(NOT_FOUND, 0), (2, plain.to_string()));
        assert_eq!(failed(-13, 1), (2, plain.to_string()));
    }

    #[test]
    fn a_symbolic_link_is_named_whatever_the_step() {
        let link = "line 7: failure 7: data/e.csv is reached through a symbolic link\n";
        for (code, step) in [(LINK, 0), (LINK, 1), (ESCAPES, 0), (ESCAPES, 1)] {
            assert_eq!(failed(code, step), (2, link.to_string()));
        }
    }
}

#[cfg(test)]
mod table_tests {
    use super::super::tests::program;
    use crate::ir::Inst::*;
    use crate::ir::{Data, Desc, Module, Reg::A};
    use crate::irsim::{self, Faults, Files, Outcome};

    /// Saves an empty table with the fields `a` and `b` (the text is `a,b\n`) as `path`,
    /// which is `folder` (`.` for the current one) and `name`.
    fn save(files: &mut Files, folder: &str, name: &str, faults: Faults) -> Outcome {
        let mut descs = Module::default();
        let number = descs.desc(Desc::Number);
        let fields = vec![("a".to_string(), number), ("b".to_string(), number)];
        let at = descs.desc(Desc::Record {
            name: "E".into(),
            fields,
        });
        let temp = format!("{name}.gloss-new");
        let path = if folder == "." {
            name.to_string()
        } else {
            format!("{folder}/{name}")
        };
        let texts = [
            format!("{folder}\0"),
            format!("{name}\0"),
            format!("{temp}\0"),
        ];
        let shown = [path, folder.to_string()];
        let all: Vec<&[u8]> = texts.iter().chain(&shown).map(|t| t.as_bytes()).collect();
        let mut body = vec![
            Const(A, 4),
            Push(A),
            Const(A, 0),
            Push(A),
            Call("list_new".into()),
        ];
        body.extend([Push(A), AddressOf(A, Data::Desc(at)), Push(A)]);
        for text in 0..5 {
            body.extend([AddressOf(A, Data::Text(text)), Push(A)]);
        }
        body.extend([Const(A, 7), Push(A), Call("table_save".into())]);
        let mut m = program(body, &all);
        m.descs = descs.descs;
        irsim::run_with(&m, "_start", b"", &[], faults, files)
    }

    fn files_with(folders: &[&str], files: &[(&str, &str)]) -> Files {
        Files {
            folders: folders.iter().map(|s| s.to_string()).collect(),
            files: files
                .iter()
                .map(|(n, b)| (n.to_string(), b.as_bytes().to_vec()))
                .collect(),
            ..Files::default()
        }
    }

    #[test]
    fn a_new_table_file_is_written_through_a_temporary_one() {
        let mut files = files_with(&[], &[]);
        let o = save(&mut files, ".", "e.csv", Faults::default());
        assert_eq!((o.status, o.stderr.len()), (0, 0));
        assert_eq!(files.files, files_with(&[], &[("e.csv", "a,b\n")]).files);
        assert_eq!(files.synced, ["e.csv.gloss-new", ""]);
        let log = [
            "sync e.csv.gloss-new",
            "rename e.csv.gloss-new -> e.csv",
            "sync ",
        ];
        assert_eq!(files.log, log);
    }

    #[test]
    fn an_old_file_is_replaced_in_its_folder_and_the_file_is_synced_before_the_folder() {
        let mut files = files_with(&["data"], &[("data/e.csv", "old\n"), ("e.csv", "keep")]);
        let o = save(&mut files, "data", "e.csv", Faults::default());
        assert_eq!(o.status, 0);
        let want = files_with(&["data"], &[("data/e.csv", "a,b\n"), ("e.csv", "keep")]);
        assert_eq!(files.files, want.files);
        assert_eq!(files.synced, ["data/e.csv.gloss-new", "data"]);
        // The rename sits between the two syncs: the folder sync is what makes it durable.
        let log = [
            "sync data/e.csv.gloss-new",
            "rename data/e.csv.gloss-new -> data/e.csv",
            "sync data",
        ];
        assert_eq!(files.log, log);
    }

    fn refused(files: &mut Files, folder: &str, faults: Faults) -> String {
        let o = save(files, folder, "e.csv", faults);
        assert_eq!(o.status, 2);
        String::from_utf8(o.stderr).unwrap()
    }

    #[test]
    fn a_name_that_leaves_the_folder_is_refused_as_a_link_at_the_probe() {
        let mut files = files_with(&[], &[("e.csv", "old\n")]);
        let o = save(&mut files, ".", "..", Faults::default());
        let err = String::from_utf8(o.stderr).unwrap();
        assert_eq!(o.status, 2);
        assert_eq!(
            err,
            "line 7: failure 7: .. is reached through a symbolic link\n"
        );
        assert_eq!(files.files, files_with(&[], &[("e.csv", "old\n")]).files);
    }

    #[test]
    fn a_missing_folder_a_link_and_a_failed_write_are_failure_7_and_leave_the_table_alone() {
        let mut files = files_with(&[], &[("e.csv", "old\n")]);
        let err = refused(&mut files, "data", Faults::default());
        assert_eq!(
            err,
            "line 7: failure 7: data/e.csv cannot be written (its folder data does not exist)\n"
        );
        files.links.insert("data".into());
        let err = refused(&mut files, "data", Faults::default());
        assert_eq!(
            err,
            "line 7: failure 7: data/e.csv is reached through a symbolic link\n"
        );
        let mut files = files_with(&[], &[("e.csv", "old\n")]);
        files.links.insert("e.csv".into());
        let err = refused(&mut files, ".", Faults::default());
        assert_eq!(
            err,
            "line 7: failure 7: e.csv is reached through a symbolic link\n"
        );
        assert_eq!(files.files, files_with(&[], &[("e.csv", "old\n")]).files);
        let mut files = files_with(&[], &[("e.csv", "old\n")]);
        let faults = Faults {
            failing_file_write: true,
            ..Faults::default()
        };
        assert_eq!(
            refused(&mut files, ".", faults),
            "line 7: failure 7: e.csv cannot be written\n"
        );
        assert_eq!(files.files["e.csv"], b"old\n");
    }
}
