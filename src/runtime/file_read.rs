//! Reading a whole file into a text, and checking that it is UTF-8.

use super::asm::Asm;
use crate::ir::Cond::Eq;
use crate::ir::Reg::{A, B};
use crate::ir::{ESCAPES, Function, Inst::*, LINK};

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

pub fn routines() -> Vec<Function> {
    vec![read_fail()]
}

#[cfg(test)]
mod tests {
    use super::super::tests::{program, run};
    use crate::ir::Inst::*;
    use crate::ir::{Data, ESCAPES, LINK, NOT_FOUND, Reg::A};

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
}
