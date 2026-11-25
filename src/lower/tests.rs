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
    assert_eq!(refusal("load rows"), format!("line 10: load rows. {stmt}"));
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
        ("clear rows\n  save rows", 8, "save rows"),
        ("if 1 is 1\n    load rows", 8, "load rows"),
        ("for each i from 1 to 2\n    load rows", 8, "load rows"),
    ];
    for (body, line, shown) in in_features {
        let text = format!("{head}feature start\n  {body}\n\ntest \"t\"\n  show 1\n");
        let err = lowered_tests(&text).expect_err("refused").render();
        assert_eq!(err, format!("line {line}: {shown}. {stmt}"));
    }
    // And nested in a test body (its statement is line 11).
    for (body, shown) in [
        ("if 1 is 1\n    load rows", "load rows"),
        ("for each i from 1 to 2\n    load rows", "load rows"),
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
fn input_lowers_each_answer_to_a_test_input_call_with_its_line() {
    let text = format!("{ROWS}test \"t\"\n  input \"a\", \"b\"\n");
    let m = lowered_tests(&text).unwrap();
    let body = function_text(function(&m, "t_0"));
    let call = "push a\n  a = 10\n  push a\n  call test_input\n";
    assert_eq!(body.matches(call).count(), 2, "{body}");
    assert!(m.functions.iter().any(|f| f.name == "test_input"));
    assert!(m.functions.iter().any(|f| f.name == "list_push"));
    assert!(!has_file_primitive(&m));
    // Nested under an `if`, the statement lowers too.
    let nested = format!("{ROWS}test \"t\"\n  if 1 is 1\n    input \"x\"\n");
    assert!(lowered_tests(&nested).is_ok());
}

/// Runs the test program `body` (statements under `test "t"`) and gives stdout and status.
fn run_test_body(body: &str, stdin: &[u8]) -> (String, i32) {
    let m = lowered_tests(&format!("{ROWS}test \"t\"\n{body}")).unwrap();
    let o = crate::irsim::run(&m, "_start", stdin);
    assert_eq!(o.stderr, b"");
    (String::from_utf8(o.stdout).unwrap(), o.status)
}

#[test]
fn ask_takes_the_input_answers_in_order_and_shows_nothing() {
    let body = "  input \"a\", \"b\"\n  ask \"first?\" into x\n  ask \"second?\" into y\n  \
                expect x is \"a\"\n  expect y is \"b\"\n";
    let (out, status) = run_test_body(body, b"");
    assert_eq!(
        (out.as_str(), status),
        ("pass  t\n\n1 passed, 0 failed\n", 0)
    );
}

#[test]
fn a_second_input_statement_extends_what_ask_takes() {
    let body = "  input \"a\"\n  ask \"q\" into x\n  input \"b\"\n  ask \"q\" into y\n  \
                expect y is \"b\"\n";
    assert_eq!(run_test_body(body, b"").1, 0);
}

#[test]
fn ask_with_no_answer_fails_the_test_with_the_spec_message() {
    let body = "  input \"a\"\n  ask \"q\" into x\n  ask \"q\" into y\n";
    let (out, status) = run_test_body(body, b"x\n");
    let want = "fail  t\n      the test gave 1 answer but line 12 asked a second question\n\n\
                0 passed, 1 failed\n";
    assert_eq!((out.as_str(), status), (want, 1));
    // Without any `input`, it is the first question, with 0 answers.
    let (out, _) = run_test_body("  ask \"q\" into x\n", b"x\n");
    assert!(out.contains("the test gave 0 answers but line 10 asked a first question"));
}

#[test]
fn real_ask_is_unchanged() {
    let real = lowered("feature start\n  ask \"Q\" into s\n  show s\n").unwrap();
    let real_ask = function(&real, "ask");
    // Reads standard input (through in_fill) and shows the question.
    let text = function_text(real_ask);
    assert!(
        text.contains("call in_fill") && text.contains("call out_text"),
        "{text}"
    );
    assert_eq!(real_ask.params, 2);
    // A test build swaps it for the queue's version, which calls neither.
    let test = lowered_tests(&format!("{ROWS}test \"t\"\n  ask \"q\" into x\n")).unwrap();
    let text = function_text(function(&test, "ask"));
    assert!(text.contains("call test_ask_fail"), "{text}");
    assert!(
        !text.contains("in_fill") && !text.contains("out_text"),
        "{text}"
    );
    assert!(!has_file_primitive(&test));
}

#[test]
fn real_mode_still_lowers_load_and_save() {
    let text = "record Row\n  n as number\n\ntable rows of Row in \"rows.csv\"\n\n\
                feature start\n  load rows\n  save rows\n\ntest \"t\"\n  show 1\n";
    let m = lowered(text).unwrap();
    assert!(m.functions.iter().any(|f| f.name == "table_read"));
    assert!(m.functions.iter().any(|f| f.name == "table_save"));
}

/// Whether a primitive touches files or reads input. The match has no wildcard: a new
/// primitive must be put on one side here.
fn reaches_files(p: crate::ir::Prim) -> bool {
    use crate::ir::Prim::*;
    match p {
        Open(_) | Rename | Sync | Mode | SetMode | Read => true,
        Write | Map | Exit | StackSize | Close | Fork | Wait | DefaultChildSignal => false,
    }
}

/// True when an instruction of the module opens, renames, syncs or changes the mode of a
/// file, or reads a descriptor.
pub(crate) fn has_file_primitive(m: &Module) -> bool {
    use crate::ir::Inst;
    let mut prims = m.functions.iter().flat_map(|f| &f.body);
    prims.any(|i| matches!(i, Inst::System(p) if reaches_files(*p)))
}

#[test]
fn has_file_primitive_finds_each_file_primitive_and_only_those() {
    use crate::ir::{Function, Inst, OpenMode, Prim};
    let with = |p: Prim| {
        let mut m = Module::default();
        m.functions.push(Function {
            name: "f".into(),
            params: 0,
            slots: 0,
            body: vec![Inst::Enter(0), Inst::System(p), Inst::Return],
        });
        m
    };
    for p in [
        Prim::Open(OpenMode::Folder),
        Prim::Open(OpenMode::Create),
        Prim::Rename,
        Prim::Sync,
        Prim::Mode,
        Prim::SetMode,
        Prim::Read,
    ] {
        assert!(has_file_primitive(&with(p)), "{p:?}");
    }
    for p in [Prim::Write, Prim::Exit, Prim::Close, Prim::Fork, Prim::Wait] {
        assert!(!has_file_primitive(&with(p)), "{p:?}");
    }
}

#[test]
fn no_real_file_primitive_in_a_test_module() {
    let m = lowered_tests(&format!(
        "{ROWS}test \"a\"\n  append Row with n 1 to rows\n  show count of rows\n  \
         set y to item 7 of rows\n"
    ))
    .unwrap();
    assert!(!has_file_primitive(&m));
    // The same program built for real, with `load` and `save`, does reach them.
    let real = lowered(
        "record Row\n  n as number\n\ntable rows of Row in \"rows.csv\"\n\n\
         feature start\n  load rows\n  save rows\n",
    )
    .unwrap();
    assert!(has_file_primitive(&real));
}

#[test]
fn test_mode_stores_each_statements_line_in_the_line_word_and_real_mode_does_not() {
    let text = format!("{ROWS}test \"t\"\n  show 1\n\n  show 2\n");
    let word = crate::runtime::CURRENT_LINE * 8;
    let store = |line: u32| {
        format!("line {line}\n  a = {line}\n  b = address of test_state\n  [b + {word}] = a\n")
    };
    let t = function_text(function(&lowered_tests(&text).unwrap(), "t_0"));
    assert!(t.contains(&store(10)) && t.contains(&store(12)), "{t}");
    assert_eq!(t.matches("test_state").count(), 2, "{t}");
    // A real build of the same statements stores nothing.
    let real = "feature start\n  show 1\n\n  show 2\n";
    let f = function_text(function(&lowered(real).unwrap(), "f_start"));
    assert!(!f.contains("test_state"), "{f}");
}

#[test]
fn the_line_is_stored_before_every_kind_of_statement() {
    // A loop's condition runs each turn, so its line is stored at the top of the loop.
    let text = format!(
        "{ROWS}test \"t\"\n  set y to 1\n  for each i from 1 to 2\n    show i\n  \
         repeat while y is 1\n    set y to 2\n  if y is 2\n    show y\n"
    );
    let t = function_text(function(&lowered_tests(&text).unwrap(), "t_0"));
    for line in [10, 11, 12, 13, 14, 15, 16] {
        assert!(
            t.contains(&format!("line {line}\n  a = {line}\n  b = address of")),
            "{line}\n{t}"
        );
    }
}

/// A program whose test has one statement per kind from line 11 on (the feature `f` gives 1).
const KINDS: [&str; 14] = [
    "append Row with n 1 to rows",
    "set n of item 1 of rows to 5",
    "set item 1 of rows to Row with n 2",
    "add 1 to n of item 1 of rows",
    "subtract 1 from n of item 1 of rows",
    "set y to 1",
    "add 1 to y",
    "multiply y by 2",
    "divide y by 2",
    "run g",
    "clear rows",
    "ask \"q\" into w",
    "expect w is \"a\"",
    "show f",
];

#[test]
fn the_line_is_stored_before_each_statement_kind() {
    let head = format!("{ROWS}feature f\n  give 1\n\nfeature g\n  show 2\n\ntest \"t\"\n");
    let text = format!("{head}  {}\n", KINDS.join("\n  "));
    let t = function_text(function(&lowered_tests(&text).unwrap(), "t_0"));
    let first = head.lines().count() as u32 + 1;
    for (i, kind) in KINDS.iter().enumerate() {
        let line = first + i as u32;
        let stored = format!("line {line}\n  a = {line}\n  b = address of test_state\n");
        assert!(t.contains(&stored), "{kind} on line {line}\n{t}");
    }
}

#[test]
fn the_line_is_stored_again_when_a_called_feature_returns() {
    // The feature's own statements move the line word; the caller's statement takes it back,
    // keeping the result in `a`, for an expression call and for `run`.
    let head = format!("{ROWS}feature f\n  set y to 1\n  give y\n\nfeature g\n  show 2\n\n");
    let text = format!("{head}test \"t\"\n  show f\n  run g\n");
    let t = function_text(function(&lowered_tests(&text).unwrap(), "t_0"));
    let again = |name: &str, line: u32| {
        format!(
            "call {name}\n  push a\n  a = {line}\n  b = address of test_state\n  [b + 64] = a\n  pop a\n"
        )
    };
    assert!(t.contains(&again("f_f", 17)), "{t}");
    assert!(t.contains(&again("f_g", 18)), "{t}");
    // A real build does not.
    let real = function_text(function(&lowered(&text).unwrap(), "f_start"));
    assert!(!real.contains("test_state"));
}

#[test]
fn a_show_after_a_feature_call_reports_its_own_line_when_shown_grows() {
    // `f` runs statements on other lines; the flush that grows shown happens in a later
    // item of the show, so the line must be the show's (10 here), not `f`'s (6).
    let text = "feature start\n  show 1\n\nfeature f\n  set y to 1\n  give y\n\n\
                test \"t\"\n  for each i from 1 to 2000\n    show f, i\n";
    let mut m = lowered_tests(text).unwrap();
    stub_grow_text(&mut m);
    assert_eq!(crate::irsim::run(&m, "t_0", b"").status, 10);
}

/// Replaces `grow_text` with a stub that exits with the line it is given.
fn stub_grow_text(m: &mut Module) {
    use crate::ir::{Inst, Prim, Reg};
    let at = m
        .functions
        .iter()
        .position(|f| f.name == "grow_text")
        .unwrap();
    m.functions[at].body = vec![
        Inst::Enter(0),
        Inst::LoadArg(Reg::A, 4),
        Inst::Push(Reg::A),
        Inst::System(Prim::Exit),
    ];
    m.functions[at].slots = 0;
}

#[test]
fn out_of_memory_while_shown_grows_reports_the_line_of_the_show() {
    // The shows fill the output buffer, and the one that overflows it flushes into shown,
    // whose first growth the stub for `grow_text` ends with the line it is given (its fourth
    // argument). Pending bytes flushed later, at the end of a test, name the last statement run.
    let text = format!("{ROWS}test \"t\"\n  for each i from 1 to 2000\n    show i\n");
    let mut m = lowered_tests(&text).unwrap();
    let at = m
        .functions
        .iter()
        .position(|f| f.name == "grow_text")
        .unwrap();
    m.functions[at].body = vec![
        crate::ir::Inst::Enter(0),
        crate::ir::Inst::LoadArg(crate::ir::Reg::A, 4),
        crate::ir::Inst::Push(crate::ir::Reg::A),
        crate::ir::Inst::System(crate::ir::Prim::Exit),
    ];
    m.functions[at].slots = 0;
    assert_eq!(crate::irsim::run(&m, "t_0", b"").status, 11);
}

fn run_tests(text: &str) -> (String, i32) {
    let o = crate::irsim::run(&lowered_tests(text).unwrap(), "_start", b"");
    (String::from_utf8(o.stdout).unwrap(), o.status)
}

const TWO_TESTS: &str = "feature start\n  show 1\n\ntest \"one\"\n  set y to 1\n\n\
                         test \"two\"\n  set y to 2\n";

#[test]
fn the_driver_forks_once_per_test() {
    let m = lowered_tests(TWO_TESTS).unwrap();
    let main = function_text(function(&m, "test_main"));
    assert_eq!(function(&m, "test_main").params, 0);
    assert_eq!(function(&m, "test_main").slots, 3);
    assert_eq!(main.matches("  system fork\n").count(), 2, "{main}");
    assert_eq!(main.matches("  system wait\n").count(), 2, "{main}");
    let (a, b) = (
        main.find("call t_0\n").unwrap(),
        main.find("call t_1\n").unwrap(),
    );
    assert!(a < b, "{main}");
    assert_eq!(main.matches("call t_").count(), 2, "{main}");
    // `_start` replaces the runtime's: no `f_start`, the driver instead.
    let start = function_text(function(&m, "_start"));
    assert!(
        start.contains("call test_main") && !start.contains("f_start"),
        "{start}"
    );
    assert_eq!(m.functions.iter().filter(|f| f.name == "_start").count(), 1);
}

#[test]
fn the_title_is_escaped() {
    let text = "feature start\n  show 1\n\ntest \"a\u{202E}b\"\n  set y to 1\n";
    let m = lowered_tests(text).unwrap();
    // The driver names the title twice per test; `Module::text` keeps one copy.
    assert_eq!(m.texts.iter().filter(|t| *t == b"a\\u{202E}b").count(), 1);
    assert!(!m.texts.iter().any(|t| t == "a\u{202E}b".as_bytes()));
}

#[test]
fn a_test_module_has_no_permissions_text() {
    // Only the runtime's `args_check` calls it, and a test executable has no flags.
    let m = lowered_tests(TWO_TESTS).unwrap();
    assert!(!m.functions.iter().any(|f| f.name == "permissions_text"));
    assert!(!m.functions.iter().any(|f| f.name == "args_check"));
    let real = lowered(TWO_TESTS).unwrap();
    assert!(real.functions.iter().any(|f| f.name == "permissions_text"));
}

#[test]
fn an_empty_driver_summarises_zero() {
    let (out, status) = run_tests("feature start\n  show 1\n");
    assert_eq!((out.as_str(), status), ("\n0 passed, 0 failed\n", 0));
}

#[test]
fn the_driver_counts_passes_and_failures_and_exits_1_when_any_failed() {
    let (out, status) = run_tests(TWO_TESTS);
    assert_eq!(
        (out.as_str(), status),
        ("pass  one\npass  two\n\n2 passed, 0 failed\n", 0)
    );
    let text =
        format!("{ROWS}test \"ok\"\n  set y to 1\n\ntest \"bad\"\n  set y to item 7 of rows\n");
    let (out, status) = run_tests(&text);
    let want = "pass  ok\nfail  bad\n      line 13: failure 2: there is no item 7 in rows; it has 0 items\n\n\
                1 passed, 1 failed\n";
    assert_eq!((out.as_str(), status), (want, 1));
}

#[test]
fn a_child_that_dies_is_reported_lost() {
    let text = "feature start\n  show 1\n\ntest \"t\"\n  set y to 1\n";
    let mut m = lowered_tests(text).unwrap();
    let at = m.functions.iter().position(|f| f.name == "t_0").unwrap();
    m.functions[at].body = vec![
        crate::ir::Inst::Enter(0),
        crate::ir::Inst::Const(crate::ir::Reg::A, 7),
        crate::ir::Inst::Push(crate::ir::Reg::A),
        crate::ir::Inst::System(crate::ir::Prim::Exit),
    ];
    m.functions[at].slots = 0;
    let o = crate::irsim::run(&m, "_start", b"");
    let want = "fail  t\n      ended without a result (status 7)\n\n0 passed, 1 failed\n";
    assert_eq!(
        (String::from_utf8(o.stdout).unwrap().as_str(), o.status),
        (want, 1)
    );
}

#[test]
fn a_failed_fork_is_a_lost_test_and_the_parent_never_waits() {
    // irsim refuses a `Wait` for a pid of 0 or less, so a Wait after the failed fork would
    // panic rather than report.
    let text = "feature start\n  show 1\n\ntest \"t\"\n  set y to 1\n\ntest \"u\"\n  set y to 1\n";
    let faults = crate::irsim::Faults {
        failing_fork: true,
        ..Default::default()
    };
    let mut files = crate::irsim::Files::default();
    let m = lowered_tests(text).unwrap();
    let o = crate::irsim::run_with(&m, "_start", b"", &[], faults, &mut files);
    let want = "fail  t\n      ended without a result (status -11)\n\
                fail  u\n      ended without a result (status -11)\n\n0 passed, 2 failed\n";
    assert_eq!(
        (String::from_utf8(o.stdout).unwrap().as_str(), o.status),
        (want, 1)
    );
}

#[test]
fn a_wait_that_gives_no_such_child_is_a_lost_test_not_a_pass() {
    // As when SIGCHLD is ignored: the child ran and passed, but the parent has no status.
    let text = "feature start\n  show 1\n\ntest \"t\"\n  set y to 1\n";
    let faults = crate::irsim::Faults {
        failing_wait: true,
        ..Default::default()
    };
    let mut files = crate::irsim::Files::default();
    let m = lowered_tests(text).unwrap();
    let o = crate::irsim::run_with(&m, "_start", b"", &[], faults, &mut files);
    let want =
        "pass  t\nfail  t\n      ended without a result (status -10)\n\n0 passed, 1 failed\n";
    assert_eq!(
        (String::from_utf8(o.stdout).unwrap().as_str(), o.status),
        (want, 1)
    );
}

#[test]
fn the_test_start_resets_sigchld_before_anything_else() {
    // An inherited ignored SIGCHLD would make every Wait fail (no such child).
    let m = lowered_tests(TWO_TESTS).unwrap();
    let start = function_text(function(&m, "_start"));
    let want = "enter 1\n  system default_child_signal\n  call heap_init\n  call stack_init\n  \
                call tables_init\n  call test_main\n";
    assert!(start.contains(want), "{start}");
}

const EXPECT_N: &str = "feature start\n  show 1\n\ntest \"t\"\n  set n to 3\n  expect n is 3\n";

#[test]
fn expect_lowers_a_condition_and_a_failure_block() {
    let m = lowered_tests(EXPECT_N).unwrap();
    let t = function_text(function(&m, "t_0"));
    // The condition's `jump` goes to the label that the failure block marks.
    let jump = t
        .split("jump if eq L")
        .nth(1)
        .unwrap_or_else(|| panic!("{t}"));
    let label: String = jump.chars().take_while(|c| c.is_ascii_digit()).collect();
    let block = t
        .split(&format!("L{label}:\n"))
        .nth(1)
        .unwrap_or_else(|| panic!("{t}"));
    let k = m.texts.iter().position(|x| x == b"n is 3").unwrap();
    let want =
        format!("  a = 6\n  push a\n  a = address of text {k}\n  push a\n  call test_expect\n");
    assert_eq!(block, want, "{t}");
    assert!(m.texts.iter().any(|x| x == b"n is 3"));
    assert!(m.functions.iter().any(|f| f.name == "test_expect"));
}

#[test]
fn expect_with_a_formatted_source() {
    let text =
        "feature start\n  show 1\n\ntest \"t\"\n  set a to 2\n  set b to 3\n  expect a*b is 6\n";
    let m = lowered_tests(text).unwrap();
    assert!(m.texts.iter().any(|x| x == b"a * b is 6"));
    assert!(!m.texts.iter().any(|x| x == b"a*b is 6"));
    // Invisible characters are spelled out.
    let text =
        "feature start\n  show 1\n\ntest \"t\"\n  set n to 3\n  expect \"a\u{202E}b\" is \"x\"\n";
    let m = lowered_tests(text).unwrap();
    assert!(m.texts.iter().any(|x| x == b"\"a\\u{202E}b\" is \"x\""));
}

#[test]
fn a_passing_and_a_failing_expect_report_through_the_driver() {
    let text = "feature start\n  show 1\n\ntest \"good\"\n  set n to 3\n  expect n is 3\n\n\
                test \"bad\"\n  set n to 4\n  expect n is 3\n";
    let (out, status) = run_tests(text);
    let want = "pass  good\nfail  bad\n      line 10: expect n is 3\n\n1 passed, 1 failed\n";
    assert_eq!((out.as_str(), status), (want, 1));
}

const START: &str = "feature start\n  show 1\n\n";

#[test]
fn expect_keeps_the_parentheses_of_its_source() {
    let text = format!("{START}test \"t\"\n  set a to 2\n  set b to 4\n  expect (a + b) is 6\n");
    let m = lowered_tests(&text).unwrap();
    assert!(m.texts.iter().any(|x| x == b"(a + b) is 6"));
    let bad = text.replace("is 6", "is 7");
    let (out, status) = run_tests(&bad);
    assert_eq!(
        (out.as_str(), status),
        (
            "fail  t\n      line 7: expect (a + b) is 7\n\n0 passed, 1 failed\n",
            1
        )
    );
}

#[test]
fn expect_short_circuits_and_and_or() {
    // `1 / n` would fail with failure 1 if it ran: it must not when the left side decides.
    let body = |cond: &str| format!("{START}test \"t\"\n  set n to 0\n  expect {cond}\n");
    let (out, status) = run_tests(&body("n is 1 or n is 0 or 1 / n is 1"));
    assert_eq!(
        (out.as_str(), status),
        ("pass  t\n\n1 passed, 0 failed\n", 0)
    );
    let (out, status) = run_tests(&body("n is 1 and 1 / n is 1"));
    let want = "fail  t\n      line 6: expect n is 1 and 1 / n is 1\n\n0 passed, 1 failed\n";
    assert_eq!((out.as_str(), status), (want, 1));
}

#[test]
fn a_failure_inside_an_expect_condition_is_that_failure_not_an_expect_failure() {
    let text = format!("{START}test \"t\"\n  set n to 0\n  expect 1 / n is 1\n");
    let (out, status) = run_tests(&text);
    let want =
        "fail  t\n      line 6: failure 1: dividing by zero (n is 0)\n\n0 passed, 1 failed\n";
    assert_eq!((out.as_str(), status), (want, 1));
}

#[test]
fn an_expect_condition_is_evaluated_once() {
    let text = "feature noisy\n  show \"hit\"\n  give yes\n\nfeature start\n  show 1\n\n\
                test \"t\"\n  expect not (noisy)\n";
    let (out, status) = run_tests(text);
    assert_eq!(out.matches("hit").count(), 1, "{out}");
    assert_eq!(status, 1);
    // A passing condition is also evaluated once (its output is not shown, so count calls).
    let passing = text.replace("not (noisy)", "(noisy)");
    let t = function_text(function(&lowered_tests(&passing).unwrap(), "t_0"));
    assert_eq!(t.matches("call f_noisy").count(), 1, "{t}");
}

const TWO_TABLES: &str = "record Row\n  name as text\n\ntable a of Row in \"a.csv\"\n\
                          table b of Row in \"b.csv\"\n\nfeature start\n  show \"ran\"\n\
                          test \"t\"\n  show 1\n";

fn tables_init_text(m: &Module) -> String {
    function_text(function(m, "tables_init"))
}

#[test]
fn a_test_module_starts_every_table_cleared() {
    // Each table's entry is 16 bytes: the list, then the loaded flag at +8.
    let flags = ["a = 1\n  [b + 8] = a\n", "a = 1\n  [b + 24] = a\n"];
    let test = tables_init_text(&lowered_tests(TWO_TABLES).unwrap());
    assert!(flags.iter().all(|f| test.contains(f)), "{test}");
    let real = tables_init_text(&lowered(TWO_TABLES).unwrap());
    assert!(
        !real.contains("[b + 8]") && !real.contains("[b + 24]"),
        "{real}"
    );
}

/// A module whose `probe` runs the start-up routines (not `stack_init`, which reads the
/// process's entry stack), loads the table block, follows `path`
/// (offsets read in turn from `a`) and exits with the word it reaches.
fn probed(m: &mut Module, path: &[i32]) {
    use crate::ir::{Data, Function, Global, Inst, Prim, Reg};
    let mut body = vec![Inst::Enter(0)];
    for routine in ["heap_init", "tables_init"] {
        body.push(Inst::Call(routine.to_string()));
    }
    body.push(Inst::AddressOf(Reg::B, Data::Global(Global::Tables)));
    body.push(Inst::Load {
        to: Reg::A,
        base: Reg::B,
        offset: 0,
    });
    for &offset in path {
        body.push(Inst::Load {
            to: Reg::A,
            base: Reg::A,
            offset,
        });
    }
    body.push(Inst::Push(Reg::A));
    body.push(Inst::System(Prim::Exit));
    m.functions.push(Function {
        name: "probe".into(),
        params: 0,
        slots: 0,
        body,
    });
}

#[test]
fn after_tables_init_in_a_test_module_the_flags_are_1_and_the_lists_empty() {
    let status = |path: &[i32]| {
        let mut m = lowered_tests(TWO_TABLES).unwrap();
        probed(&mut m, path);
        crate::irsim::run(&m, "probe", b"").status
    };
    // The second table's flag, then the first table's list count.
    assert_eq!(status(&[24]), 1);
    assert_eq!(status(&[0, 0]), 0);
    let mut real = lowered(TWO_TABLES).unwrap();
    probed(&mut real, &[24]);
    assert_eq!(crate::irsim::run(&real, "probe", b"").status, 0);
}

#[test]
fn shown_lowers_to_a_shown_list_call_with_its_line() {
    let m = lowered_tests(&format!("{ROWS}test \"t\"\n  expect count of shown is 0\n")).unwrap();
    let body = function_text(function(&m, "t_0"));
    assert!(
        body.contains("a = 10\n  push a\n  call shown_list\n"),
        "{body}"
    );
}

#[test]
fn shown_sees_the_line_a_show_left_in_the_output_buffer() {
    let body = "  show \"Total\"\n  expect last of shown is \"Total\"\n  show \"Largest\"\n  \
                expect count of shown is 2\n  expect first of shown is \"Total\"\n";
    let (out, status) = run_test_body(body, b"");
    assert_eq!(
        (out.as_str(), status),
        ("pass  t\n\n1 passed, 0 failed\n", 0)
    );
}

#[test]
fn shown_is_a_fresh_copy_each_time_it_is_read() {
    let body = "  show \"a\"\n  set before to shown\n  show \"b\"\n  \
                expect count of before is 1\n  expect count of shown is 2\n  \
                set before to shown\n  append \"x\" to before\n  expect count of shown is 2\n";
    let (out, status) = run_test_body(body, b"");
    assert_eq!(
        (out.as_str(), status),
        ("pass  t\n\n1 passed, 0 failed\n", 0)
    );
}
