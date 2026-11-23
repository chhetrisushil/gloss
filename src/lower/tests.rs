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

fn start_text(m: &Module) -> String {
    let f = m.functions.iter().find(|f| f.name == "f_start").unwrap();
    function_text(f)
}

#[test]
fn line_markers_follow_the_source_lines_past_blanks_and_notes() {
    let m = lowered("feature start\n  show \"a\"\n\n  note why\n  show \"b\"\n").unwrap();
    assert_eq!(
        start_text(&m),
        "function f_start, 0 params, 0 slots\n  enter 0\nline 2\n  a = address of text 0\n  \
         push a\n  call out_text\n  a = address of text 1\n  push a\n  call out_text\n\
         line 5\n  a = address of text 2\n  push a\n  call out_text\n  a = address of text 1\n  \
         push a\n  call out_text\n  return\n"
    );
}

#[test]
fn equal_texts_are_interned_once() {
    let m = lowered("feature start\n  show \"a\"\n  show \"a\"\n").unwrap();
    assert_eq!(&m.texts[..2], [b"a".to_vec(), b"\n".to_vec()]);
    assert_eq!(m.texts.iter().filter(|t| *t == b"a").count(), 1);
}

#[test]
fn every_feature_is_lowered_with_its_input_count() {
    let text = "feature start\n  show \"x\"\nfeature other\n  needs n as number\n  show \"y\"\n";
    let m = lowered(text).unwrap();
    let params: Vec<(&str, u32)> = m.functions.iter().map(|f| (&*f.name, f.params)).collect();
    assert_eq!(&params[..2], [("f_start", 0), ("f_other", 1)]);
}

#[test]
fn slots_are_unique_names_without_inputs() {
    let text = "feature start\n  set x to 1\n  for each i from 1 to 2\n    show i\n  for each i from 1 to 2\n    show i\n  show x\n";
    let src = Source::new(text);
    let program = crate::parse_source(&src).unwrap();
    let types = check(&program, &src).types.expect("checks clean");
    let slots = super::assign_slots(&types, "start");
    assert_eq!(slots.len(), 2);
    assert_eq!((slots["x"], slots["i"]), (1, 2));
}

#[test]
fn show_of_a_number_lowers_to_out_number_with_the_line() {
    let m = lowered("feature start\n  show -7.5\n").unwrap();
    let f = m.functions.iter().find(|f| f.name == "f_start").unwrap();
    assert!(function_text(f).contains("line 2\n  a = -75000\n  push a\n  call out_number\n"));
    assert!(m.functions.iter().any(|f| f.name == "number_format"));
}

#[test]
fn a_mixed_show_lowers_each_item_in_order() {
    let m = lowered("feature start\n  show \"a\", 1, \"b\", -2.5\n").unwrap();
    let f = m.functions.iter().find(|f| f.name == "f_start").unwrap();
    assert_eq!(
        function_text(f),
        "function f_start, 0 params, 0 slots\n  enter 0\nline 2\n  a = address of text 0\n  \
         push a\n  call out_text\n  a = 10000\n  push a\n  call out_number\n  \
         a = address of text 1\n  push a\n  call out_text\n  a = -25000\n  push a\n  \
         call out_number\n  a = address of text 2\n  push a\n  call out_text\n  return\n"
    );
}

#[test]
fn a_sum_lowers_to_num_add_with_a_failure_block() {
    let m = lowered("feature start\n  set t to 1 + 2\n  show t\n").unwrap();
    assert_eq!(
        start_text(&m),
        "function f_start, 0 params, 1 slots\n  enter 1\nline 2\n  a = 10000\n  push a\n  \
         a = 20000\n  b = a\n  pop a\n  a = num_add a, b; out of range L0\n  slot 1 = a\n\
         line 3\n  a = slot 1\n  push a\n  call out_number\n  a = address of text 0\n  push a\n  \
         call out_text\n  return\nL0:\n  a = 2\n  push a\n  a = 4\n  push a\n  call fail_begin\n  \
         a = address of text 1\n  push a\n  call err_text\n  a = address of text 2\n  push a\n  \
         call err_text\n  a = address of text 3\n  push a\n  call err_text\n  call fail_end\n"
    );
    assert_eq!(m.texts[1], b"a number went out of range (");
    assert_eq!(m.texts[2], b"1 + 2");
}
