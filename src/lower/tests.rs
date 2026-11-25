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
fn tables_init_gives_each_table_a_list_and_a_flag_of_zero() {
    let text = "record Row\n  name as text\n\ntable a of Row in \"a.csv\"\ntable b of Row in \"b.csv\"\n\n\
                feature start\n  show \"ran\"\n";
    let src = Source::new(text);
    let program = crate::parse_source(&src).expect("parses");
    let types = check(&program, &src).types.expect("checks clean");
    let m = super::lower(&program, &types, &src).expect("lowers");
    let init = m
        .functions
        .iter()
        .find(|f| f.name == "tables_init")
        .unwrap();
    let text = function_text(init);
    assert_eq!(text.matches("call list_new").count(), 2, "{text}");
    assert!(text.contains("[b + 16] = a"), "{text}");
    let o = crate::irsim::run(&m, "_start", b"");
    assert_eq!((o.stdout, o.status), (b"ran\n".to_vec(), 0));
}

#[test]
fn a_table_works_like_a_list() {
    let text = "record Row\n  n as number\n\ntable rows of Row in \"rows.csv\"\n\nfeature start\n  \
                append Row with n 1 to rows\n  set n of item 1 of rows to 5\n  \
                show count of rows, \" \", n of first of rows\n";
    let m = lowered(text).unwrap();
    let o = crate::irsim::run(&m, "_start", b"");
    assert_eq!((o.stdout, o.status), (b"1 5\n".to_vec(), 0));
}

#[test]
fn a_table_is_compared_by_its_rows_like_a_list() {
    let text = "record Row\n  n as number\n\ntable rows of Row in \"a.csv\"\n\
                table other of Row in \"b.csv\"\n\nfeature start\n  \
                append Row with n 1 to rows\n  set y to rows\n  \
                show (rows is y), (rows is not y), (y is rows), (y is not rows)\n  \
                show (rows is other), (rows is not other)\n  \
                append Row with n 1 to other\n  \
                show (rows is other), (other is not rows)\n  \
                show (rows is list of (Row with n 1)), (rows is not list of (Row with n 1))\n  \
                show ((list of (Row with n 1)) is rows), (rows is empty list of Row)\n";
    let o = crate::irsim::run(&lowered(text).unwrap(), "_start", b"");
    let want = "yesnoyesno\nnoyes\nyesno\nyesno\nyesno\n";
    assert_eq!((o.stdout, o.status), (want.as_bytes().to_vec(), 0));
}

#[test]
fn a_program_without_tables_allocates_nothing_at_start() {
    let m = lowered("feature start\n  show \"x\"\n").unwrap();
    let init = m
        .functions
        .iter()
        .find(|f| f.name == "tables_init")
        .unwrap();
    assert!(!function_text(init).contains("call"));
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
    let slots = super::assign_slots(
        &types.features["start"].vars,
        &types.features["start"].loops,
    );
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
fn a_stored_record_is_copied_unless_it_was_just_built() {
    let rec = "record B\n  n as number\n\nfeature start\n  set xs to list of (B with n 1)\n  set item 1 of xs to item 1 of xs\n";
    let t = start_text(&lowered(rec).unwrap());
    assert_eq!(t.matches("call copy_value").count(), 1, "{t}");
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

const TABLE_PROGRAM: &str = "record Row\n  name as text\n\ntable rows of Row in \"rows.csv\"\n\n\
                             feature start\n  show \"ran\"\n";

fn run_with_args(text: &str, args: &[&str]) -> crate::irsim::Outcome {
    let src = Source::new(text);
    let program = crate::parse_source(&src).expect("parses");
    let types = check(&program, &src).types.expect("checks clean");
    let m = super::lower(&program, &types, &src).expect("lowers");
    let (faults, mut files) = (
        crate::irsim::Faults::default(),
        crate::irsim::Files::default(),
    );
    crate::irsim::run_with(&m, "_start", b"", args, faults, &mut files)
}

fn list_of(text: &str) -> String {
    let program = crate::parse_source(&Source::new(text)).expect("parses");
    crate::permissions::render(&crate::permissions::list(&program))
}

#[test]
fn permissions_prints_the_list_and_exits_0() {
    let o = run_with_args(TABLE_PROGRAM, &["--permissions"]);
    let list = list_of(TABLE_PROGRAM);
    assert!(list.contains("rows.csv"), "{list}");
    assert_eq!(
        (o.stdout, o.stderr, o.status),
        (list.into_bytes(), vec![], 0)
    );
}

#[test]
fn other_arguments_are_ignored() {
    let cases: [&[&str]; 6] = [
        &["--permissions", "x"],
        &["-p"],
        &["--permissionsX"],
        &["--permission"],
        &[""],
        &[],
    ];
    for args in cases {
        let o = run_with_args(TABLE_PROGRAM, args);
        assert_eq!(
            (o.stdout.as_slice(), o.status),
            (&b"ran\n"[..], 0),
            "{args:?}"
        );
    }
}

#[test]
fn a_program_without_tables_prints_nothing_else() {
    let text = "feature start\n  show \"ran\"\n";
    let o = run_with_args(text, &["--permissions"]);
    assert_eq!(o.stdout, crate::permissions::render(&[]).into_bytes());
    assert_eq!(o.status, 0);
}

#[test]
fn a_saved_table_path_is_a_folder_and_two_plain_names() {
    use super::table::{SaveNames, save_names};
    let names = |folder: &str, name: &str| SaveNames {
        folder: folder.into(),
        name: name.into(),
        temp: format!("{name}.gloss-new"),
    };
    assert_eq!(save_names("e.csv"), Ok(names(".", "e.csv")));
    assert_eq!(save_names("data/e.csv"), Ok(names("data", "e.csv")));
    assert_eq!(save_names("a/b/e.csv"), Ok(names("a/b", "e.csv")));
    for bad in [
        "/e.csv",
        "/a/e.csv",
        "../e.csv",
        "a/../e.csv",
        "a//e.csv",
        "a/",
        "",
        "a\0b",
        ".",
        "..",
        "a/..",
        "./e.csv",
        "a/./e.csv",
        "a/.",
        "./",
    ] {
        assert!(save_names(bad).is_err(), "{bad:?}");
    }
}

#[test]
fn save_lowers_to_the_flag_check_and_a_table_save_call_with_its_texts() {
    let text = "record Row\n  n as number\n\ntable rows of Row in \"data/rows.csv\"\n\nfeature start\n  \
                clear rows\n  save rows\n";
    let m = lowered(text).unwrap();
    let f = m.functions.iter().find(|f| f.name == "f_start").unwrap();
    let text = function_text(f);
    assert_eq!(text.matches("call table_save").count(), 1, "{text}");
    for want in [
        &b"data\0"[..],
        b"rows.csv\0",
        b"rows.csv.gloss-new\0",
        b"data/rows.csv",
    ] {
        assert!(m.texts.iter().any(|t| t == want), "{want:?}");
    }
    assert!(m.functions.iter().any(|f| f.name == "table_save"));
}

#[test]
fn load_lowers_to_a_table_read_call_and_stores_the_list_only_after_it() {
    let text = "record Row\n  n as number\n\ntable rows of Row in \"data/rows.csv\"\n\nfeature start\n  \
                load rows\n";
    let m = lowered(text).unwrap();
    let f = m.functions.iter().find(|f| f.name == "f_start").unwrap();
    let text = function_text(f);
    assert_eq!(text.matches("call table_read").count(), 1, "{text}");
    let read = text.find("call table_read").expect("the call");
    assert!(text[read..].contains("[a] = b"), "{text}");
    for want in [&b"data/rows.csv\0"[..], b"data/rows.csv"] {
        assert!(m.texts.iter().any(|t| t == want), "{want:?}");
    }
    assert!(m.functions.iter().any(|f| f.name == "table_read"));
}

fn lowered_tests(text: &str) -> Result<Module, Diagnostic> {
    let src = Source::new(text);
    let program = crate::parse_source(&src).expect("parses");
    let checked = check(&program, &src);
    let types = checked.types.unwrap_or_else(|| {
        let shown: Vec<String> = checked.diagnostics.iter().map(|d| d.render()).collect();
        panic!("does not check clean: {shown:?}")
    });
    super::lower_tests(&program, &types, &src)
}

fn function<'a>(m: &'a Module, name: &str) -> &'a crate::ir::Function {
    m.functions
        .iter()
        .find(|f| f.name == name)
        .unwrap_or_else(|| panic!("no function {name}"))
}

const ROWS: &str = "record Row\n  n as number\n\ntable rows of Row in \"rows.csv\"\n\n\
                    feature start\n  show \"x\"\n\n";

#[test]
fn a_test_body_lowers_to_t_0() {
    let m = lowered_tests(&format!(
        "{ROWS}test \"adds\"\n  append Row with n 1 to rows\n"
    ))
    .unwrap();
    let t = function(&m, "t_0");
    assert_eq!(t.params, 0);
    assert!(function_text(t).contains("call list_push"), "{t:?}");
    assert!(function_text(t).ends_with("  return\n"));
    assert!(m.functions.iter().any(|f| f.name == "f_start"));
}

#[test]
fn tests_are_numbered_in_source_order() {
    let m = lowered_tests(&format!(
        "{ROWS}test \"a\"\n  append Row with n 1 to rows\n\ntest \"b\"\n  show \"b\"\n\n\
         test \"c\"\n  show \"c\"\n  show \"c\"\n"
    ))
    .unwrap();
    let names: Vec<&str> = m.functions.iter().map(|f| &*f.name).collect();
    let at = |n: &str| names.iter().position(|x| *x == n).expect(n);
    assert!(at("t_0") < at("t_1") && at("t_1") < at("t_2"));
    let (a, b, c) = (
        function(&m, "t_0"),
        function(&m, "t_1"),
        function(&m, "t_2"),
    );
    assert!(function_text(a).contains("call list_push"));
    assert!(function_text(b).contains("text 1"), "{}", function_text(b));
    assert_eq!(function_text(c).matches("call out_text").count(), 4);
    assert!(function_text(b).matches("call out_text").count() == 2);
    assert!(!m.functions.iter().any(|f| f.name == "t_3"));
}

#[test]
fn a_test_body_that_can_fail_gets_its_failure_block_frame_and_routines() {
    let text = format!("{ROWS}test \"t\"\n  set y to item 7 of rows\n");
    let m = lowered_tests(&text).unwrap();
    let t = function(&m, "t_0");
    let body = function_text(t);
    assert!(body.contains("call fail_begin"), "{body}");
    assert!(body.contains("call fail_end"), "{body}");
    assert_eq!(t.body[0], crate::ir::Inst::Enter(t.slots));
    assert!(t.slots >= 1, "the named slot y");
    // The routines the failure block calls come from the test backend.
    assert!(function_text(function(&m, "fail_end")).contains("call test_fail"));
    // A real build of the same text has no `t_0`, and its fail_end exits at once.
    let real = lowered(&text).unwrap();
    assert!(!real.functions.iter().any(|f| f.name == "t_0"));
    assert!(!function_text(function(&real, "fail_end")).contains("test_fail"));
}

fn refusal(body: &str) -> String {
    let text = format!("{ROWS}test \"t\"\n  {body}\n");
    lowered_tests(&text).expect_err("refused").render()
}

#[test]
fn test_mode_refuses_what_is_not_built_yet() {
    let stmt = "this compiler cannot build this statement yet.";
    // The statement under `test` is line 10.
    assert_eq!(
        refusal("ask \"q\" into a"),
        format!("line 10: ask \"q\" into a. {stmt}")
    );
    assert_eq!(refusal("load rows"), format!("line 10: load rows. {stmt}"));
    assert_eq!(
        refusal("input \"x\""),
        format!("line 10: input \"x\". {stmt}")
    );
    assert_eq!(
        refusal("expect 1 is 1"),
        format!("line 10: expect 1 is 1. {stmt}")
    );
    assert_eq!(
        refusal("show shown"),
        "line 10: show shown. this compiler cannot build this expression yet."
    );
    // `save` needs a feature that loads or clears the table.
    let text = "record Row\n  n as number\n\ntable rows of Row in \"rows.csv\"\n\n\
                feature start\n  clear rows\n\ntest \"t\"\n  save rows\n";
    assert_eq!(
        lowered_tests(text).expect_err("refused").render(),
        format!("line 10: save rows. {stmt}")
    );
    // The same refusals in a feature no test calls, plain and nested under if and for each.
    let head = "record Row\n  n as number\n\ntable rows of Row in \"rows.csv\"\n\n";
    let in_features = [
        ("ask \"q\" into a\n  show a", 7, "ask \"q\" into a"),
        ("clear rows\n  save rows", 8, "save rows"),
        ("if 1 is 1\n    load rows", 8, "load rows"),
        (
            "for each i from 1 to 2\n    ask \"q\" into a\n    show a",
            8,
            "ask \"q\" into a",
        ),
    ];
    for (body, line, shown) in in_features {
        let text = format!("{head}feature start\n  {body}\n\ntest \"t\"\n  show 1\n");
        let err = lowered_tests(&text).expect_err("refused").render();
        assert_eq!(err, format!("line {line}: {shown}. {stmt}"));
    }
    // And nested in a test body (its statement is line 11).
    for (body, shown) in [
        ("if 1 is 1\n    load rows", "load rows"),
        (
            "for each i from 1 to 2\n    ask \"q\" into a\n    show a",
            "ask \"q\" into a",
        ),
        ("if 1 is 1\n    input \"x\"", "input \"x\""),
        ("if 1 is 1\n    expect 1 is 1", "expect 1 is 1"),
    ] {
        assert_eq!(refusal(body), format!("line 11: {shown}. {stmt}"), "{body}");
    }
    // Features are lowered whole: a `load` no test calls is refused too.
    let text = "record Row\n  n as number\n\ntable rows of Row in \"rows.csv\"\n\n\
                feature start\n  load rows\n\ntest \"t\"\n  show 1\n";
    assert_eq!(
        lowered_tests(text).expect_err("refused").render(),
        format!("line 7: load rows. {stmt}")
    );
}

#[test]
fn real_mode_still_lowers_load_and_save() {
    let text = "record Row\n  n as number\n\ntable rows of Row in \"rows.csv\"\n\n\
                feature start\n  load rows\n  save rows\n\ntest \"t\"\n  show 1\n";
    let m = lowered(text).unwrap();
    assert!(m.functions.iter().any(|f| f.name == "table_read"));
    assert!(m.functions.iter().any(|f| f.name == "table_save"));
}
