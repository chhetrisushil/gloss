use super::lower;
use crate::check::check;
use crate::diag::Diagnostic;
use crate::ir::{Module, function_text};
use crate::source::Source;

fn lowered(text: &str) -> Result<Module, Diagnostic> {
    let src = Source::new(text);
    let program = crate::parse_source(&src).expect("parses");
    let types = check(&program, &src).types.expect("checks clean");
    lower(&program, &types, &src)
}

#[test]
fn show_of_text_lowers_to_out_text_calls() {
    let m = lowered("feature start\n  show \"hi\"\n").unwrap();
    let f = m.functions.iter().find(|f| f.name == "f_start").unwrap();
    assert_eq!(
        function_text(f),
        "function f_start, 0 params, 0 slots\n  enter 0\nline 2\n  a = address of text 0\n  \
         push a\n  call out_text\n  a = address of text 1\n  push a\n  call out_text\n  return\n"
    );
    assert_eq!(&m.texts[..2], [b"hi".to_vec(), b"\n".to_vec()]);
}

#[test]
fn a_program_with_a_table_is_refused() {
    let text = "record Row\n  name as text\n\ntable rows of Row in \"rows.csv\"\n\nfeature start\n  show \"x\"\n";
    let d = lowered(text).unwrap_err();
    assert_eq!(d.problem, "tables arrive in milestone 3");
    assert_eq!(d.line, None);
}

#[test]
fn the_lowered_program_runs_in_irsim() {
    let m = lowered("feature start\n  show \"hi\"\n").unwrap();
    let o = crate::irsim::run(&m, "_start", b"");
    assert_eq!((o.stdout, o.status), (b"hi\n".to_vec(), 0));
}
