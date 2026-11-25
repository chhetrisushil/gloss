//! Tests of the test backend, run on hand-made modules under `irsim`.

use super::test_state::{ANSWERS, CURRENT_LINE};
use super::tests::{program_for, run};
use super::{Backend, routines_for, test_backend};
use crate::ir::Reg::{A, B};
use crate::ir::{Cond, Data, Global, Inst, Inst::*, Prim};

/// Pushes the runtime text `i`'s address.
fn text(i: u32) -> Vec<Inst> {
    vec![AddressOf(A, Data::Text(i)), Push(A)]
}

fn number(n: i64) -> Vec<Inst> {
    vec![Const(A, n), Push(A)]
}

/// Calls `routine` with the arguments.
fn call(routine: &str, args: &[Vec<Inst>]) -> Vec<Inst> {
    let mut body = args.concat();
    body.push(Call(routine.into()));
    body
}

/// Runs `body` after `test_begin("t")`, under the Test backend; texts 1 and up are `texts`.
fn run_test(body: &[Vec<Inst>], texts: &[&[u8]]) -> (String, String, i32) {
    let all: Vec<&[u8]> = [&[&b"t"[..]][..], texts].concat();
    let body = [call("test_begin", &[text(0)]), body.concat()].concat();
    let o = run(&program_for(Backend::Test, body, &all));
    let (out, err) = (o.stdout, o.stderr);
    (
        String::from_utf8(out).unwrap(),
        String::from_utf8(err).unwrap(),
        o.status,
    )
}

fn failure(line: i64, code: i64, message: u32) -> Vec<Inst> {
    let begin = call("fail_begin", &[number(line), number(code)]);
    [
        begin,
        call("err_text", &[text(message)]),
        call("fail_end", &[]),
    ]
    .concat()
}

#[test]
fn a_failure_goes_to_the_failure_text() {
    let body = failure(5, 1, 1);
    let (out, err, status) = run_test(&[body], &[b"dividing by zero (n is 0)"]);
    assert_eq!(
        out,
        "fail  t\n      line 5: failure 1: dividing by zero (n is 0)\n"
    );
    assert_eq!((err.as_str(), status), ("", 1));
}

#[test]
fn shown_output_comes_after_the_message() {
    let show = call("out_text", &[text(1)]);
    let (out, err, status) = run_test(&[show, failure(5, 1, 2)], &[b"hi\n", b"oops"]);
    assert_eq!(
        out,
        "fail  t\n      line 5: failure 1: oops\n      shown:\n        hi\n"
    );
    assert_eq!((err.as_str(), status), ("", 1));
}

#[test]
fn a_failure_inside_a_show_flushes_into_shown() {
    // `a` has no newline and is still in the output buffer when the failure begins.
    let show = call("out_text", &[text(1)]);
    let (out, _, _) = run_test(&[show, failure(2, 3, 2)], &[b"a", b"x"]);
    assert!(out.ends_with("      shown:\n        a\n"), "{out}");
}

#[test]
fn write_all_sends_descriptor_1_to_shown_and_2_to_the_failure_text() {
    let write = |fd, i| {
        let bytes = [AddressOf(A, Data::Text(i)), AddImm(A, 8), Push(A)];
        call("write_all", &[number(fd), bytes.to_vec(), number(3)])
    };
    let body = [
        write(1, 1),
        write(2, 2),
        write(1, 1),
        call("test_fail", &[]),
    ];
    let (out, err, status) = run_test(&body, &[b"hi\n", b"oops"]);
    let want = "fail  t\n      oop\n      shown:\n        hi\n        hi\n";
    assert_eq!((out.as_str(), err.as_str(), status), (want, "", 1));
}

#[test]
fn test_fail_flushes_pending_output_into_shown() {
    let show = call("out_text", &[text(1)]);
    let (out, _, status) = run_test(&[show, call("test_fail", &[])], &[&[b'x'; 300]]);
    assert_eq!(
        (out, status),
        (
            format!("fail  t\n      shown:\n        {}\n", "x".repeat(300)),
            1
        )
    );
}

#[test]
fn no_shown_part_when_nothing_was_shown() {
    let (out, _, status) = run_test(&[call("test_fail", &[])], &[]);
    assert_eq!((out.as_str(), status), ("fail  t\n", 1));
}

#[test]
fn write_all_to_other_descriptors_is_ignored() {
    let at = [AddressOf(A, Data::Text(1)), AddImm(A, 8), Push(A)];
    let mut writes = Vec::new();
    for fd in [0, 3, 9, -1] {
        writes.extend(call("write_all", &[number(fd), at.to_vec(), number(2)]));
    }
    let (out, err, status) = run_test(&[writes, call("test_fail", &[])], &[b"ab"]);
    assert_eq!((out.as_str(), err.as_str(), status), ("fail  t\n", "", 1));
}

#[test]
fn test_expect_formats_the_line() {
    let source: &[u8] = b"name of largest_expense is \"Dinner\"";
    let expect = call("test_expect", &[number(23), text(1)]);
    let (out, err, status) = run_test(&[expect], &[source]);
    let want = "fail  t\n      line 23: expect name of largest_expense is \"Dinner\"\n";
    assert_eq!((out.as_str(), err.as_str(), status), (want, "", 1));
}

#[test]
fn test_expect_keeps_pending_output() {
    let show = call("out_text", &[text(1)]);
    let expect = call("test_expect", &[number(4), text(2)]);
    let (out, _, status) = run_test(&[show, expect], &[&[b'x'; 3000], b"a is 1"]);
    let want = format!(
        "      line 4: expect a is 1\n      shown:\n        {}\n",
        "x".repeat(3000)
    );
    assert_eq!((out, status), (format!("fail  t\n{want}"), 1));
}

#[test]
fn replaced_routines_keep_their_parameters() {
    let real = routines_for(Backend::Real);
    let test = routines_for(Backend::Test);
    for name in test_backend::REPLACED {
        let params =
            |all: &[crate::ir::Function]| all.iter().find(|f| f.name == name).map(|f| f.params);
        assert!(params(&real).is_some(), "{name}");
        assert_eq!(params(&real), params(&test), "{name}");
    }
}

#[test]
fn a_failure_flushes_3000_pending_bytes_into_shown() {
    let lines = "ab\n".repeat(1000);
    let show = call("out_text", &[text(1)]);
    let (out, err, status) = run_test(&[show, failure(2, 3, 2)], &[lines.as_bytes(), b"oops"]);
    let shown = "        ab\n".repeat(1000);
    let want = format!("fail  t\n      line 2: failure 3: oops\n      shown:\n{shown}");
    assert_eq!((out, err.as_str(), status), (want, "", 1));
}

/// The status of a test whose `body` runs after 3000 bytes are shown and the heap is noted,
/// with `test_fail` replaced by a stub that exits 1 if the heap has moved since, else 0.
fn heap_moved_during(body: Vec<Inst>) -> i32 {
    let state = AddressOf(B, Data::Global(Global::TestState));
    let heap = AddressOf(A, Data::Global(Global::HeapNext));
    let load = |to, base| Load {
        to,
        base,
        offset: 40 * (to == B) as i32,
    };
    let show = call("out_text", &[text(1)]);
    let note = [
        heap.clone(),
        load(A, A),
        state.clone(),
        Store {
            base: B,
            offset: 40,
            from: A,
        },
    ];
    let begin = call("test_begin", &[text(0)]);
    let all = [begin, show, note.to_vec(), body].concat();
    let mut m = program_for(
        Backend::Test,
        all,
        &[b"t", "ab\n".repeat(1000).as_bytes(), b"x"],
    );
    let stub = [Enter(0), heap, load(A, A), state, load(B, B)];
    let stub = [
        &stub[..],
        &[Compare, SetIf(Cond::Ne), Push(A), System(Prim::Exit)],
    ]
    .concat();
    let at = m
        .functions
        .iter()
        .position(|f| f.name == "test_fail")
        .unwrap();
    m.functions[at].body = stub;
    m.functions[at].slots = 0;
    run(&m).status
}

#[test]
fn a_failure_with_pending_output_does_not_allocate() {
    assert_eq!(heap_moved_during(failure(2, 3, 2)), 0);
    // The check can tell: an allocation before the failure moves the heap.
    let alloc = call("alloc", &[number(8), number(0)]);
    assert_eq!(heap_moved_during([alloc, failure(2, 3, 2)].concat()), 1);
}

#[test]
fn running_out_of_memory_inside_a_test_is_a_failure_of_the_test() {
    let show = call("out_text", &[text(1)]);
    let alloc = call("alloc", &[number(-1), number(7)]);
    let (out, err, status) = run_test(&[show, alloc], &[b"hi\n"]);
    let want = "fail  t\n      line 7: failure 10: out of memory\n      shown:\n        hi\n";
    assert_eq!((out.as_str(), err.as_str(), status), (want, "", 1));
}

#[test]
fn a_show_too_big_to_grow_shown_is_out_of_memory_at_its_line_and_its_bytes_are_lost() {
    let at = |text: u32, len: i64| {
        let bytes = vec![AddressOf(A, Data::Text(text)), AddImm(A, 8), Push(A)];
        call("out_bytes", &[bytes, number(len)])
    };
    let line = [
        AddressOf(A, Data::Global(Global::TestState)),
        Const(B, 9),
        Store {
            base: A,
            offset: CURRENT_LINE as i32 * 8,
            from: B,
        },
    ];
    // "abc" waits in the output buffer; the next show is far larger than the buffer, so it is
    // flushed first (reaching shown) and then the big one is written straight to `write_all`.
    // Growing the shown bytes for it fails before any of its bytes are copied: they are lost,
    // and what was shown before them is kept.
    let body = [line.to_vec(), at(1, 3), at(1, 1 << 41)];
    let (out, err, status) = run_test(&body, &[b"abcdef"]);
    let want = "fail  t\n      line 9: failure 10: out of memory\n      shown:\n        abc\n";
    assert_eq!((out.as_str(), err.as_str(), status), (want, "", 1));
}

/// Shows answer `i` of the queue, read straight from the state.
fn show_answer(i: i32) -> Vec<Inst> {
    let items = [
        AddressOf(A, Data::Global(Global::TestState)),
        Load {
            to: A,
            base: A,
            offset: ANSWERS as i32 * 8,
        },
        Load {
            to: A,
            base: A,
            offset: 16,
        },
        Load {
            to: A,
            base: A,
            offset: i * 8,
        },
        Push(A),
    ];
    call("out_text", &[items.to_vec()])
}

fn input(answer: u32, line: i64) -> Vec<Inst> {
    call("test_input", &[text(answer), number(line)])
}

#[test]
fn input_queues_answers_in_order() {
    let mut body = vec![input(1, 3), input(2, 3)];
    body.extend([show_answer(0), show_answer(1), call("test_fail", &[])]);
    let (out, err, status) = run_test(&body, &[b"a\n", b"b\n"]);
    let want = "fail  t\n      shown:\n        a\n        b\n";
    assert_eq!((out.as_str(), err.as_str(), status), (want, "", 1));
}

#[test]
fn a_second_input_statement_adds_to_the_queue() {
    // Six answers from two statements: the list grows past `list_new`'s room of 4.
    let mut body: Vec<Vec<Inst>> = (0..6).map(|i| input(1 + i % 2, 3 + i64::from(i))).collect();
    body.extend((0..6).map(show_answer));
    body.push(call("test_fail", &[]));
    let (out, _, _) = run_test(&body, &[b"a\n", b"b\n"]);
    let want =
        "fail  t\n      shown:\n        a\n        b\n        a\n        b\n        a\n        b\n";
    assert_eq!(out, want);
}

#[test]
fn input_shows_nothing_and_reads_nothing() {
    let body = [input(1, 3), call("test_fail", &[])];
    let (out, _, _) = run_test(&body, &[b"a\n"]);
    assert_eq!(out, "fail  t\n");
}

fn ask_fail(line: i64, given: i64, asked: i64) -> String {
    let body = call(
        "test_ask_fail",
        &[number(line), number(given), number(asked)],
    );
    let (out, err, status) = run_test(&[body], &[]);
    assert_eq!((err.as_str(), status), ("", 1));
    out
}

#[test]
fn the_question_message_names_the_answers_given_the_line_and_the_question() {
    let pre = "fail  t\n      the test gave ";
    assert_eq!(
        ask_fail(7, 0, 1),
        format!("{pre}0 answers but line 7 asked a first question\n")
    );
    assert_eq!(
        ask_fail(7, 2, 3),
        format!("{pre}2 answers but line 7 asked a third question\n")
    );
    // The spec's own example (section 9).
    assert_eq!(
        ask_fail(31, 2, 3),
        format!("{pre}2 answers but line 31 asked a third question\n")
    );
}

#[test]
fn one_answer_is_singular() {
    let want = "fail  t\n      the test gave 1 answer but line 7 asked a second question\n";
    assert_eq!(ask_fail(7, 1, 2), want);
}

#[test]
fn past_the_tenth_question_the_number_stands_in_for_the_ordinal() {
    let want = "fail  t\n      the test gave 10 answers but line 4 asked question number 11\n";
    assert_eq!(ask_fail(4, 10, 11), want);
}

#[test]
fn each_of_the_ten_ordinals_is_spelled() {
    let words = [
        "first", "second", "third", "fourth", "fifth", "sixth", "seventh", "eighth", "ninth",
        "tenth",
    ];
    for (k, word) in words.iter().enumerate() {
        let want =
            format!("fail  t\n      the test gave 3 answers but line 5 asked a {word} question\n");
        assert_eq!(ask_fail(5, 3, k as i64 + 1), want, "{word}");
    }
}

#[test]
fn the_question_message_comes_before_what_was_shown() {
    let show = call("out_text", &[text(1)]);
    let fail = call("test_ask_fail", &[number(2), number(0), number(1)]);
    let (out, _, _) = run_test(&[show, fail], &[b"hi\n"]);
    let want = "fail  t\n      the test gave 0 answers but line 2 asked a first question\n      shown:\n        hi\n";
    assert_eq!(out, want);
}

fn ask(question: u32, line: i64) -> Vec<Inst> {
    call("ask", &[text(question), number(line)])
}

/// Shows the text `ask` left in `A`.
fn show_result() -> Vec<Inst> {
    call("out_text", &[vec![Push(A)]])
}

#[test]
fn ask_takes_the_answers_in_order_and_shows_only_them() {
    let body = [
        input(1, 3),
        input(2, 3),
        ask(3, 5),
        show_result(),
        ask(3, 6),
        show_result(),
        call("test_fail", &[]),
    ];
    let (out, err, status) = run_test(&body, &[b"a\n", b"b\n", b"Q?\n"]);
    let want = "fail  t\n      shown:\n        a\n        b\n";
    assert_eq!((out.as_str(), err.as_str(), status), (want, "", 1));
}

#[test]
fn ask_with_none_left_fails_with_the_count_and_the_line() {
    let body = [input(1, 3), ask(2, 5), ask(2, 8)];
    let (out, err, status) = run_test(&body, &[b"a\n", b"Q?\n"]);
    let want = "fail  t\n      the test gave 1 answer but line 8 asked a second question\n";
    assert_eq!((out.as_str(), err.as_str(), status), (want, "", 1));
    let (out, _, _) = run_test(&[ask(1, 4)], &[b"Q?\n"]);
    let want = "fail  t\n      the test gave 0 answers but line 4 asked a first question\n";
    assert_eq!(out, want);
}

#[test]
fn ask_never_reads_standard_input() {
    use crate::irsim::{self, Faults, Files};
    let body = [input(1, 3), ask(2, 5), show_result(), ask(2, 6)];
    let all: Vec<&[u8]> = vec![b"t", b"a\n", b"Q?\n"];
    let body = [call("test_begin", &[text(0)]), body.concat()].concat();
    let m = program_for(Backend::Test, body, &all);
    // A read of descriptor 0 would fail here (and a real `ask` would end the program with
    // failure 9): the test still gets its queued answer and then the message.
    let faults = Faults {
        failing_read: true,
        ..Faults::default()
    };
    let o = irsim::run_with(&m, "_start", b"x\n", &[], faults, &mut Files::default());
    let want = "fail  t\n      the test gave 1 answer but line 6 asked a second question\n      shown:\n        a\n";
    assert_eq!(String::from_utf8(o.stdout).unwrap(), want);
    assert_eq!((o.stderr.as_slice(), o.status), (&b""[..], 1));
    // And with input available it is not consumed either.
    let o = irsim::run(&m, "_start", b"x\n");
    assert!(
        String::from_utf8(o.stdout)
            .unwrap()
            .contains("asked a second question")
    );
}

#[test]
fn the_ask_prompt_is_not_part_of_shown() {
    let body = [input(1, 3), ask(2, 5), call("test_fail", &[])];
    let (out, _, _) = run_test(&body, &[b"a\n", b"Prompt?\n"]);
    assert_eq!(out, "fail  t\n");
}

fn load(offset: i32) -> Inst {
    Load {
        to: A,
        base: A,
        offset,
    }
}

/// Loads state word `word` (a list) into `a`, then item `i` of it or, for `None`, its count.
fn list_part(word: u32, item: Option<i32>) -> Vec<Inst> {
    let state = AddressOf(A, Data::Global(Global::TestState));
    let at = [state, load(word as i32 * 8)];
    let part = match item {
        Some(i) => vec![load(16), load(i * 8)],
        None => vec![load(0)],
    };
    [&at[..], &part, &[Push(A)]].concat()
}

/// `shown_list(5)`, kept in state word `word`.
fn take_shown(word: u32) -> Vec<Inst> {
    let state = [Pop(B), AddressOf(A, Data::Global(Global::TestState))];
    let store = Store {
        base: A,
        offset: word as i32 * 8,
        from: B,
    };
    let take = call("shown_list", &[number(5)]);
    [take, vec![Push(A)], state.to_vec(), vec![store]].concat()
}

/// Puts the count and items (`|` after each, text 1) of the list in `word` into the failure
/// text, then fails the test: `fail  t`, `<count><item>|...`, and the shown bytes.
fn dump(word: u32, items: i32) -> Vec<Inst> {
    let mut body = call("err_int", &[list_part(word, None)]);
    for i in 0..items {
        body.extend(call("err_text", &[list_part(word, Some(i))]));
        body.extend(call("err_text", &[text(1)]));
    }
    body.extend(call("test_fail", &[]));
    body
}

/// What a test that shows each of `lines` (texts 2 up), then takes `shown`, finds: the
/// failure text with the count and the first `items` items, then the shown bytes.
fn shown_items(lines: &[&[u8]], items: i32) -> String {
    let mut texts: Vec<&[u8]> = vec![b"|"];
    texts.extend(lines);
    let mut body: Vec<Vec<Inst>> = (0..lines.len() as u32)
        .map(|i| call("out_text", &[text(2 + i)]))
        .collect();
    body.extend([take_shown(ANSWERS), dump(ANSWERS, items)]);
    let out = run_test(&body, &texts).0;
    out.strip_prefix("fail  t\n      ").expect(&out).to_string()
}

#[test]
fn shown_splits_at_newlines() {
    let out = shown_items(&[b"a\nbb\n"], 2);
    assert_eq!(out, "2a|bb|\n      shown:\n        a\n        bb\n");
}

#[test]
fn shown_reads_what_is_still_in_the_output_buffer() {
    // Three separate shows, all pending in the buffer when `shown_list` runs.
    let out = shown_items(&[b"a\n", b"bb\n", b"c\n"], 3);
    assert!(out.starts_with("3a|bb|c|\n"), "{out}");
}

#[test]
fn a_fragment_is_not_a_line() {
    assert!(shown_items(&[b"a\nb"], 1).starts_with("1a|\n"));
    assert!(shown_items(&[b"abc"], 0).starts_with("0\n"));
    // An empty line is a line.
    assert!(shown_items(&[b"\n\nx\n"], 3).starts_with("3||x|\n"));
}

#[test]
fn shown_is_a_copy() {
    let take_a = [
        call("out_text", &[text(2)]),
        take_shown(ANSWERS),
        call("out_text", &[text(3)]),
        dump(ANSWERS, 1),
    ];
    // The list taken first still has one item after another line is shown.
    let out = run_test(&take_a, &[b"|", b"a\n", b"b\n"]).0;
    assert!(out.starts_with("fail  t\n      1a|\n"), "{out}");
}

#[test]
fn a_carriage_return_stays_in_the_line() {
    // A line ends at `\n` only: `a\r\n` is the item `a\r`.
    assert!(shown_items(&[b"a\r\n"], 1).starts_with("1a\r|\n"));
}
