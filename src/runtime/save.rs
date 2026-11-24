//! Saving a table: write a temporary file beside it, then rename it over the table.

use super::asm::Asm;
use crate::ir::Cond::{Eq, Ne};
use crate::ir::Reg::{A, B};
use crate::ir::{ESCAPES, Function, Inst::*, LINK, NOT_FOUND};

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

pub fn routines() -> Vec<Function> {
    vec![save_fail()]
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
