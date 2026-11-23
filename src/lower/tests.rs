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

#[test]
fn a_division_reports_a_zero_divisor_as_failure_1() {
    let m = lowered("feature start\n  set a to 1\n  set b to 0\n  show a / b\n").unwrap();
    let t = start_text(&m);
    assert!(
        t.contains("a = num_div a, b; zero L0; out of range L1\n"),
        "{t}"
    );
    assert!(m.texts.contains(&b"dividing by zero (".to_vec()));
    assert!(m.texts.contains(&b" is 0)".to_vec()));
}

#[test]
fn a_negative_position_is_failure_2_and_the_last_item_is_reachable() {
    let text = "feature start\n  set xs to list of 7, 8\n  show last of xs, item 2 of xs\n  show item (0 - 1) of xs\n";
    let o = crate::irsim::run(&lowered(text).unwrap(), "_start", b"");
    let err = "line 4: failure 2: there is no item -1 in xs; it has 2 items\n";
    assert_eq!((o.stdout, o.stderr), (b"88\n".to_vec(), err.into()));
}

#[test]
fn a_record_copied_into_an_item_is_not_built_yet() {
    let rec = "record B\n  n as number\n\nfeature start\n  set xs to list of (B with n 1)\n  set item 1 of xs to item 1 of xs\n";
    let d = lowered(rec).unwrap_err();
    assert_eq!(d.problem, "this compiler cannot build this statement yet");
}

#[test]
fn and_jumps_past_its_right_side_when_the_left_is_no() {
    let m = lowered("feature start\n  show (1 is 2 and 3 is 3)\n").unwrap();
    let t = start_text(&m);
    assert!(
        t.contains(
            "  a = 10000\n  push a\n  a = 20000\n  b = a\n  pop a\n  compare a, b\n  a = 1 if eq\n  \
             b = 0\n  compare a, b\n  jump if eq L0\n  a = 30000\n  push a\n  a = 30000\n  b = a\n  \
             pop a\n  compare a, b\n  a = 1 if eq\nL0:\n"
        ),
        "{t}"
    );
}

#[test]
fn or_jumps_past_its_right_side_when_the_left_is_yes() {
    let m = lowered("feature start\n  show (yes or no)\n").unwrap();
    let t = start_text(&m);
    assert!(
        t.contains("  a = 1\n  b = 1\n  compare a, b\n  jump if eq L0\n  a = 0\nL0:\n"),
        "{t}"
    );
}

#[test]
fn a_truth_shows_as_yes_or_no_texts() {
    let m = lowered("feature start\n  show yes\n").unwrap();
    assert!(m.texts.contains(&b"yes".to_vec()) && m.texts.contains(&b"no".to_vec()));
}

#[test]
fn comparing_texts_calls_text_equal_and_is_not_flips_it() {
    let m = lowered("feature start\n  show (\"a\" is not \"b\")\n").unwrap();
    let t = start_text(&m);
    assert!(
        t.contains("  call text_equal\n  b = a\n  a = 1\n  a = a - b\n"),
        "{t}"
    );
    assert!(m.functions.iter().any(|f| f.name == "text_equal"));
}

#[test]
fn text_order_is_not_built_since_the_checker_refuses_it() {
    let src = Source::new("feature start\n  show (\"a\" is greater than \"b\")\n");
    let program = crate::parse_source(&src).expect("parses");
    assert!(check(&program, &src).types.is_none());
}

#[test]
fn an_if_with_otherwise_lowers_to_jumps_over_each_branch() {
    let m = lowered("feature start\n  if 1 is 1\n    show \"a\"\n  otherwise\n    show \"b\"\n")
        .unwrap();
    let t = start_text(&m);
    assert!(
        t.contains("  jump if eq L1\n") && t.contains("  jump L0\nL1:\n") && t.contains("L0:\n"),
        "{t}"
    );
}

#[test]
fn a_while_loop_tests_its_condition_before_each_turn() {
    let m =
        lowered("feature start\n  set n to 0\n  repeat while n is less than 1\n    add 1 to n\n")
            .unwrap();
    let t = start_text(&m);
    let top = t.find("L0:\n").expect("a loop top");
    assert!(
        top < t.find("jump if eq L1").unwrap() && t.contains("  jump L0\nL1:\n"),
        "{t}"
    );
}

#[test]
fn nested_ifs_and_loops_use_distinct_labels() {
    let text = "feature start\n  set n to 0\n  repeat while n is less than 2\n    if n is 0\n      show \"a\"\n    otherwise\n      show \"b\"\n    add 1 to n\n";
    let t = start_text(&lowered(text).unwrap());
    for l in ["L0:\n", "L1:\n", "L2:\n", "L3:\n"] {
        assert_eq!(t.matches(l).count(), 1, "{l} in {t}");
    }
}

#[test]
fn a_feature_that_falls_off_its_end_stops_with_an_internal_error() {
    let text = "feature f\n  needs n as number\n  if n is 0\n    give 1\n  give 2\n\nfeature start\n  show (f with 1)\n";
    let src = Source::new(text);
    let mut program = crate::parse_source(&src).expect("parses");
    let types = check(&program, &src).types.expect("checks clean");
    // Bypass the checker's guarantee: drop the final give, so `f with 1` falls off the end.
    for item in &mut program.items {
        match item {
            crate::ast::program::Item::Feature(f) if f.name == "f" => {
                f.body.pop();
            }
            _ => {}
        }
    }
    let m = lower(&program, &types, &src).expect("lowers");
    let o = crate::irsim::run(&m, "_start", b"");
    assert_eq!(o.stdout, b"");
    assert_eq!(
        String::from_utf8(o.stderr).unwrap(),
        "internal error: feature f ended without giving a value\n"
    );
    assert_eq!(o.status, 2);
}

#[test]
fn a_call_checks_the_stack_for_the_callee_frame() {
    let text =
        "feature deep\n  needs n as number\n  give n\n\nfeature start\n  show (deep with 1)\n";
    let t = start_text(&lowered(text).unwrap());
    // 0 slots: 8 * (0 + 3) + 64.
    assert!(t.contains("stack check 88"), "{t}");
}

#[test]
fn the_check_counts_the_callees_named_and_hidden_slots() {
    // `total` plus the count loop's hidden slots: read the callee's real slot count.
    let text = "feature sum\n  set total to 0\n  for each i from 1 to 3\n    add i to total\n  give total\n\nfeature start\n  show sum\n";
    let m = lowered(text).unwrap();
    let slots = m
        .functions
        .iter()
        .find(|f| f.name == "f_sum")
        .unwrap()
        .slots;
    assert!(
        slots > 2,
        "a named slot, a loop variable and temps: {slots}"
    );
    let want = format!("stack check {}", 8 * (slots + 3) + 64);
    let t = start_text(&m);
    assert!(t.contains(&want), "{want} in {t}");
}

#[test]
fn the_source_in_a_failure_message_spells_invisible_characters() {
    let m = lowered("feature start\n  show \"a\u{200F}b\u{202E}x\" as number\n").unwrap();
    let spelled = b"\"a\\u{200F}b\\u{202E}x\" as number".to_vec();
    assert!(m.texts.contains(&spelled), "{:?}", m.texts);
    let raw = "as number".as_bytes();
    let unspelled = |t: &Vec<u8>| t.windows(raw.len()).any(|w| w == raw) && t != &spelled;
    assert!(!m.texts.iter().any(unspelled));
}

#[test]
fn ask_lowers_to_a_call_with_the_question_and_the_line() {
    let m = lowered("feature start\n  ask \"Q\" into s\n  show s\n").unwrap();
    let f = m.functions.iter().find(|f| f.name == "f_start").unwrap();
    assert!(function_text(f).contains(
        "line 2\n  a = address of text 0\n  push a\n  a = 2\n  push a\n  call ask\n  slot 1 = a\n"
    ));
    assert!(m.functions.iter().any(|f| f.name == "ask"));
}
