//! Reading a whole file into a text, and checking that it is UTF-8.

use super::asm::Asm;
use crate::ir::Cond::{Eq, Ge, Lt, Ne};
use crate::ir::Reg::{A, B};
use crate::ir::{ESCAPES, Function, Inst::*, IntOp, LINK, NOT_FOUND, OpenMode, Prim, Shift};

/// `read_fail(line, shown, code)`: failure 6 for the table file `shown` (its path, in display
/// form) when `Open` or `Read` gave the error `code`: a symbolic link (`LINK`, or `ESCAPES`
/// for a path that leaves the folder) is named, any other error is "cannot be read".
fn read_fail() -> Function {
    let mut s = Asm::default();
    let (link, end) = (s.label(), s.label());
    s.push_arg(1);
    s.push_const(6);
    s.call("fail_begin");
    s.push_arg(2);
    s.call("err_text");
    for code in [LINK, ESCAPES] {
        s.i(LoadArg(A, 3));
        s.i(Const(B, code));
        s.jump_if(Eq, link);
    }
    s.push_text(super::CANNOT_READ);
    s.i(Jump(end));
    s.i(Mark(link));
    s.push_text(super::THROUGH_LINK);
    s.i(Mark(end));
    s.call("err_text");
    s.call("fail_end");
    s.finish("read_fail", 3, 0)
}

/// The room a file's text starts with, in bytes; it doubles whenever the file is longer.
const FIRST_ROOM: i64 = 4096;

// Slots of `file_bytes`.
const FD: u32 = 1;
const TEXT: u32 = 2;
const LEN: u32 = 3;
const ROOM: u32 = 4;
const N: u32 = 5;
const AT: u32 = 6;
const LINES: u32 = 7;
const BAD: u32 = 8;

/// `a = slot x`, `b = slot y`.
fn load2(s: &mut Asm, x: u32, y: u32) {
    s.i(LoadSlot(A, x));
    s.i(LoadSlot(B, y));
}

/// Pushes the address of the bytes of the text in `TEXT`, `offset` bytes in.
fn push_text_at(s: &mut Asm, offset: u32) {
    load2(s, TEXT, offset);
    s.i(Int(IntOp::Add));
    s.i(AddImm(A, 8));
    s.i(Push(A));
}

/// `file_bytes(folder, path, shown, line)`: the whole content of the file `path` (a
/// NUL-terminated text) in the folder descriptor `folder`, as a text, or 0 when there is no
/// such file, after the notice "<shown> not found; starting with an empty table" on standard
/// error. A file that cannot be opened or read, or one that is reached through a symbolic
/// link, is failure 6 (`read_fail`), and so is one that is not UTF-8: the line is that of the
/// first bad sequence. The text grows by doubling, so a file too large for memory is failure 10.
fn file_bytes() -> Function {
    let mut s = Asm::default();
    let [again, room, done, open_failed, cannot, read_failed] = [(); 6].map(|_| s.label());
    let [count, report, step, feed] = [(); 4].map(|_| s.label());
    s.push_arg(1);
    s.i(LoadArg(A, 2));
    s.i(AddImm(A, 8));
    s.i(Push(A));
    s.i(System(Prim::Open(OpenMode::Read)));
    s.i(StoreSlot(FD, A));
    s.i(Const(B, 0));
    s.jump_if(Lt, open_failed);
    s.i(Const(A, FIRST_ROOM));
    s.i(StoreSlot(ROOM, A));
    s.i(Const(A, FIRST_ROOM + 8));
    s.i(Push(A));
    s.push_arg(4);
    s.call("alloc");
    s.i(StoreSlot(TEXT, A));
    s.i(Mark(again));
    load2(&mut s, LEN, ROOM);
    s.jump_if(Lt, room);
    s.push_slot(TEXT);
    s.push_slot(LEN);
    s.push_slot(ROOM);
    s.push_arg(4);
    s.call("grow_text");
    s.i(StoreSlot(TEXT, A));
    s.i(LoadSlot(A, ROOM));
    s.i(Shift(Shift::Left, 1));
    s.i(StoreSlot(ROOM, A));
    s.i(Mark(room));
    s.push_slot(FD);
    push_text_at(&mut s, LEN);
    load2(&mut s, ROOM, LEN);
    s.i(Int(IntOp::Sub));
    s.i(Push(A));
    s.i(System(Prim::Read));
    s.i(StoreSlot(N, A));
    s.i(Const(B, 0));
    s.jump_if(Lt, read_failed);
    s.jump_if(Eq, done);
    load2(&mut s, LEN, N);
    s.i(Int(IntOp::Add));
    s.i(StoreSlot(LEN, A));
    s.i(Jump(again));
    s.i(Mark(done));
    s.push_slot(FD);
    s.i(System(Prim::Close));
    s.i(LoadSlot(A, TEXT));
    s.i(LoadSlot(B, LEN));
    s.i(Store {
        base: A,
        offset: 0,
        from: B,
    });
    s.push_slot(TEXT);
    s.call("utf8_valid_up_to");
    s.i(StoreSlot(BAD, A));
    s.i(LoadSlot(B, LEN));
    s.jump_if(Lt, count);
    s.i(LoadSlot(A, TEXT));
    s.i(Return);
    // The line of the first bad byte: one more than the line feeds before it.
    s.i(Mark(count));
    s.i(Const(A, 1));
    s.i(StoreSlot(LINES, A));
    s.i(Const(A, 0));
    s.i(StoreSlot(AT, A));
    s.i(Mark(step));
    load2(&mut s, AT, BAD);
    s.jump_if(Ge, report);
    s.i(LoadSlot(A, AT));
    s.i(LoadSlot(B, TEXT));
    s.i(Int(IntOp::Add));
    s.i(LoadByte {
        to: A,
        base: A,
        offset: 8,
    });
    s.i(Const(B, 10));
    s.jump_if(Ne, feed);
    s.i(LoadSlot(A, LINES));
    s.i(AddImm(A, 1));
    s.i(StoreSlot(LINES, A));
    s.i(Mark(feed));
    s.i(LoadSlot(A, AT));
    s.i(AddImm(A, 1));
    s.i(StoreSlot(AT, A));
    s.i(Jump(step));
    s.i(Mark(report));
    s.push_arg(4);
    s.push_const(6);
    s.call("fail_begin");
    s.push_arg(3);
    s.call("err_text");
    s.push_text(super::NOT_UTF8);
    s.call("err_text");
    s.push_slot(LINES);
    s.call("err_int");
    s.push_text(super::CLOSE);
    s.call("err_text");
    s.call("fail_end");
    s.i(Mark(open_failed));
    s.i(StoreSlot(N, A));
    s.i(Const(B, NOT_FOUND));
    s.jump_if(Ne, cannot);
    s.call("flush");
    s.push_arg(3);
    s.call("err_text");
    s.push_text(super::NOT_FOUND_NOTICE);
    s.call("err_text");
    s.i(Const(A, 0));
    s.i(Return);
    s.i(Mark(read_failed));
    s.push_slot(FD);
    s.i(System(Prim::Close));
    s.i(Mark(cannot));
    s.push_arg(4);
    s.push_arg(3);
    s.push_slot(N);
    s.call("read_fail");
    s.finish("file_bytes", 4, 8)
}

pub fn routines() -> Vec<Function> {
    vec![read_fail(), file_bytes()]
}

#[cfg(test)]
mod tests {
    use super::super::tests::{program, run};
    use crate::ir::Inst::*;
    use crate::ir::{
        CWD,
        Cond::Eq,
        Data, ESCAPES, LINK, Label, NOT_FOUND,
        Reg::{A, B},
    };
    use crate::irsim::{self, Faults, Files, Outcome};

    /// What `read_fail` does for the error `code`: the status and the error output. The file
    /// is `data/e.csv`.
    fn failed(code: i64) -> (i32, String) {
        let mut body = vec![Const(A, 7), Push(A), AddressOf(A, Data::Text(0)), Push(A)];
        body.extend([Const(A, code), Push(A), Call("read_fail".into())]);
        let o = run(&program(body, &[b"data/e.csv"]));
        (o.status, String::from_utf8(o.stderr).unwrap())
    }

    #[test]
    fn a_symbolic_link_is_named_and_any_other_error_is_cannot_be_read() {
        let link = "line 7: failure 6: data/e.csv is reached through a symbolic link\n";
        for code in [LINK, ESCAPES] {
            assert_eq!(failed(code), (2, link.to_string()));
        }
        let plain = "line 7: failure 6: data/e.csv cannot be read\n";
        for code in [NOT_FOUND, -13, -21, -5] {
            assert_eq!(failed(code), (2, plain.to_string()));
        }
    }

    /// Reads `path` from the current folder and shows the text on standard output, or `NULL`
    /// when `file_bytes` gives 0. The failure messages call the file `shown`.
    fn read(files: &mut Files, path: &str, faults: Faults) -> Outcome {
        let texts = [format!("{path}\0"), "data/e.csv".to_string(), "NULL".into()];
        let raw: Vec<&[u8]> = texts.iter().map(|t| t.as_bytes()).collect();
        let (none, end) = (Label(1), Label(2));
        let mut body = vec![Const(A, CWD), Push(A)];
        for text in [0, 1] {
            body.extend([AddressOf(A, Data::Text(text)), Push(A)]);
        }
        body.extend([Const(A, 7), Push(A), Call("file_bytes".into())]);
        body.extend([Const(B, 0), Compare, JumpIf(Eq, none)]);
        body.extend([Push(A), Call("out_text".into()), Jump(end), Mark(none)]);
        body.extend([
            AddressOf(A, Data::Text(2)),
            Push(A),
            Call("out_text".into()),
        ]);
        body.extend([Mark(end), Call("flush".into())]);
        irsim::run_with(&program(body, &raw), "_start", b"", &[], faults, files)
    }

    fn files_with(folders: &[&str], files: &[(&str, &[u8])]) -> Files {
        Files {
            folders: folders.iter().map(|s| s.to_string()).collect(),
            files: files
                .iter()
                .map(|(n, b)| (n.to_string(), b.to_vec()))
                .collect(),
            ..Files::default()
        }
    }

    #[test]
    fn a_file_is_read_whole_whatever_its_length_against_the_room() {
        for len in [0, 1, 4095, 4096, 4097, 8192, 8193, 20000] {
            let bytes: Vec<u8> = (0..len).map(|i| b'a' + (i % 26) as u8).collect();
            let mut files = files_with(&[], &[("e.csv", &bytes)]);
            let o = read(&mut files, "e.csv", Faults::default());
            assert_eq!((o.status, o.stderr.len()), (0, 0), "length {len}");
            assert!(o.stdout == bytes, "length {len}");
        }
    }

    #[test]
    fn a_missing_file_is_a_notice_and_no_text() {
        let mut files = files_with(&["data"], &[]);
        let o = read(&mut files, "data/none.csv", Faults::default());
        let notice = "data/e.csv not found; starting with an empty table\n";
        assert_eq!((o.status, o.stdout), (0, b"NULL".to_vec()));
        assert_eq!(String::from_utf8(o.stderr).unwrap(), notice);
    }

    /// The status and standard error of a read that ends the program.
    fn refused(mut files: Files, path: &str, faults: Faults) -> (i32, String) {
        let o = read(&mut files, path, faults);
        assert!(o.stdout.is_empty());
        (o.status, String::from_utf8(o.stderr).unwrap())
    }

    #[test]
    fn links_a_path_that_escapes_and_other_errors_are_failure_6() {
        let link = "line 7: failure 6: data/e.csv is reached through a symbolic link\n";
        let plain = "line 7: failure 6: data/e.csv cannot be read\n";
        let mut files = files_with(&[], &[("e.csv", b"a\n")]);
        files.links.insert("e.csv".into());
        assert_eq!(refused(files, "e.csv", Faults::default()), (2, link.into()));
        let files = files_with(&[], &[("e.csv", b"a\n")]);
        assert_eq!(
            refused(files, "../e.csv", Faults::default()),
            (2, link.into())
        );
        let files = files_with(&[], &[("e.csv", b"a\n")]);
        let faults = Faults {
            failing_open: Some("e.csv".into()),
            ..Faults::default()
        };
        assert_eq!(refused(files, "e.csv", faults), (2, plain.into()));
        // A folder opens, and then cannot be read.
        let files = files_with(&["data"], &[]);
        assert_eq!(refused(files, "data", Faults::default()), (2, plain.into()));
    }

    #[test]
    fn bytes_that_are_not_utf8_name_the_line_of_the_first_bad_one() {
        let bad = |bytes: &[u8]| {
            let files = files_with(&[], &[("e.csv", bytes)]);
            refused(files, "e.csv", Faults::default()).1
        };
        let line = |n: u32| format!("line 7: failure 6: data/e.csv is not UTF-8 text (line {n})\n");
        assert_eq!(bad(b"\xFF"), line(1));
        assert_eq!(bad(b"ab\ncd\n\xC3(\n\xFF"), line(3));
        assert_eq!(bad(b"a\r\nb\r\n\xE2\x82"), line(3));
        assert_eq!(bad(b"\n\n\n\n\xC0\xAF"), line(5));
    }
}
