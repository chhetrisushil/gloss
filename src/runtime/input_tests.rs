//! Reading standard input, through `irsim`.

use super::tests::program;
use crate::ir::Inst::*;
use crate::ir::Reg::A;
use crate::ir::{Data, Global, Inst, Prim};
use crate::irsim::{self, Faults, Files, Outcome};

fn run_with(body: Vec<Inst>, stdin: &[u8], faults: Faults) -> Outcome {
    let m = program(body, &[b"x"]);
    irsim::run_with(&m, "_start", stdin, &[], faults, &mut Files::default())
}

/// Runs `body` on `stdin`, which ends with the exit status in `a`.
fn status(mut body: Vec<Inst>, stdin: &[u8]) -> i32 {
    body.extend([Push(A), System(Prim::Exit)]);
    run_with(body, stdin, Faults::default()).status
}

fn fill() -> Inst {
    Call("in_fill".into())
}

/// `a` = the global, plus `n`.
fn global_plus(g: Global, n: i32) -> Vec<Inst> {
    let load = [
        AddressOf(A, Data::Global(g)),
        Load {
            to: A,
            base: A,
            offset: 0,
        },
    ];
    load.into_iter().chain([AddImm(A, n)]).collect()
}

/// Sets the global to `n`.
fn set(g: Global, n: i64) -> Vec<Inst> {
    let at = [
        Const(A, n),
        Push(A),
        AddressOf(A, Data::Global(g)),
        Pop(crate::ir::Reg::B),
    ];
    at.into_iter()
        .chain([Store {
            base: A,
            offset: 0,
            from: crate::ir::Reg::B,
        }])
        .collect()
}

#[test]
fn in_fill_reads_what_is_there_and_says_1() {
    assert_eq!(status(vec![fill()], b"abc"), 1);
    let mut body = vec![fill()];
    body.extend(global_plus(Global::InLen, 0));
    assert_eq!(status(body.clone(), b"abc"), 3);
    assert_eq!(status(body, b"abcdef"), 6);
}

#[test]
fn in_fill_reads_at_most_the_buffer_and_again_when_it_is_used_up() {
    let input = vec![b'z'; 5000];
    let mut body = vec![fill()];
    body.extend(global_plus(Global::InLen, -4000));
    assert_eq!(status(body, &input), 96);
    let mut body = vec![fill()];
    body.extend(set(Global::InPos, 4096));
    body.push(fill());
    body.extend(global_plus(Global::InLen, -800));
    assert_eq!(status(body, &input), 104);
}

#[test]
fn in_fill_does_not_read_while_a_byte_is_left() {
    let mut body = vec![fill()];
    body.extend(set(Global::InPos, 1));
    body.push(fill());
    body.extend(global_plus(Global::InPos, 0));
    assert_eq!(status(body, b"ab"), 1);
}

#[test]
fn in_fill_says_0_at_the_end_of_input_every_time() {
    assert_eq!(status(vec![fill()], b""), 0);
    let mut body = vec![fill()];
    body.extend(set(Global::InPos, 2));
    body.push(fill());
    assert_eq!(status(body.clone(), b"ab"), 0);
    body.push(fill());
    assert_eq!(status(body, b"ab"), 0);
}

#[test]
fn a_read_error_ends_the_program_with_status_2_and_no_message() {
    let faults = Faults {
        failing_read: true,
        ..Faults::default()
    };
    let say = [
        AddressOf(A, Data::Text(0)),
        Push(A),
        Call("out_text".into()),
    ];
    let o = run_with([say.to_vec(), vec![fill()]].concat(), b"a\n", faults);
    assert_eq!((o.status, o.stdout, o.stderr), (2, b"x".to_vec(), vec![]));
}

/// Calls `grow_text(text 0, 5, room, 3)`, then writes the first `shown` bytes of the result.
fn grown(room: i64, shown: i64) -> Outcome {
    let call = [AddressOf(A, Data::Text(0)), Push(A), Const(A, 5), Push(A)];
    let more = [
        Const(A, room),
        Push(A),
        Const(A, 3),
        Push(A),
        Call("grow_text".into()),
    ];
    let out = [
        AddImm(A, 8),
        Push(A),
        Const(A, shown),
        Push(A),
        Call("out_bytes".into()),
    ];
    let body = [&call[..], &more, &out, &[Call("flush".into())]].concat();
    irsim::run(&program(body, &[b"hello"]), "_start", b"")
}

#[test]
fn grow_text_keeps_the_bytes_it_was_given() {
    let o = grown(5, 5);
    assert_eq!((o.status, o.stdout), (0, b"hello".to_vec()));
    let o = grown(1 << 20, 5);
    assert_eq!((o.status, o.stdout), (0, b"hello".to_vec()));
}

#[test]
fn a_text_that_cannot_double_is_out_of_memory() {
    for room in [1 << 61, i64::MAX, i64::MAX - 3] {
        let o = grown(room, 0);
        assert_eq!(o.status, 2, "{room}");
        assert_eq!(o.stderr, b"line 3: failure 10: out of memory\n", "{room}");
    }
}

/// Source line the asks are made on.
const LINE: i64 = 7;

/// A program that asks `n` times with the question `?` and shows each answer then `|`.
fn run_asks(n: usize, stdin: &[u8]) -> Outcome {
    let mut body: Vec<Inst> = Vec::new();
    for _ in 0..n {
        body.extend([AddressOf(A, Data::Text(0)), Push(A)]);
        body.extend([Const(A, LINE), Push(A), Call("ask".into()), Push(A)]);
        body.push(Call("out_text".into()));
        body.extend([
            AddressOf(A, Data::Text(1)),
            Push(A),
            Call("out_text".into()),
        ]);
    }
    body.push(Call("flush".into()));
    let m = program(body, &[b"?", b"|"]);
    irsim::run(&m, "_start", stdin)
}

#[test]
fn two_asks_share_one_buffered_input_and_drop_line_endings() {
    for input in [&b"Asha\r\nRavi"[..], b"Asha\nRavi\n"] {
        let o = run_asks(2, input);
        assert_eq!((o.status, o.stdout), (0, b"?\nAsha|?\nRavi|".to_vec()));
        assert!(o.stderr.is_empty());
    }
}

#[test]
fn end_of_input_with_nothing_read_is_failure_9() {
    let o = run_asks(2, b"a\n");
    assert_eq!(o.stdout, b"?\na|?\n");
    let message = b"line 7: failure 9: input ended while ask was waiting\n";
    assert_eq!((o.status, o.stderr), (2, message.to_vec()));
}

/// What `run_asks` shows for these answers; the run must end well.
fn asked(n: usize, stdin: &[u8], answers: &[&[u8]]) {
    let o = run_asks(n, stdin);
    let mut shown = Vec::new();
    for a in answers {
        shown.extend(b"?\n");
        shown.extend(*a);
        shown.push(b'|');
    }
    assert_eq!((o.status, o.stdout == shown, o.stderr), (0, true, vec![]));
}

#[test]
fn an_empty_line_is_an_empty_text() {
    asked(1, b"\n", &[b""]);
    asked(2, b"\r\n\n", &[b"", b""]);
}

#[test]
fn only_a_line_ending_is_removed_so_a_lone_final_cr_stays() {
    asked(1, b"ab\r", &[b"ab\r"]);
    asked(1, b"ab\r\r\n", &[b"ab\r"]);
    asked(1, b"\rab", &[b"\rab"]);
}

#[test]
fn any_byte_value_is_kept() {
    let bytes: Vec<u8> = (0u8..=255).filter(|&b| b != b'\n').collect();
    asked(1, &[&bytes[..], b"\n"].concat(), &[&bytes]);
}

#[test]
fn a_line_longer_than_the_input_buffer_crosses_refills() {
    for len in [63, 64, 65, 4095, 4096, 4097, 8193, 10000] {
        let line: Vec<u8> = (0..len).map(|i| b'a' + (i % 26) as u8).collect();
        asked(2, &[&line[..], b"\nnext\n"].concat(), &[&line, b"next"]);
    }
}

#[test]
fn a_cr_at_the_end_of_one_refill_goes_with_the_lf_that_starts_the_next() {
    let x = [b'x'; 4095];
    asked(2, &[&x[..], b"\r\nnext\n"].concat(), &[&x, b"next"]);
}

#[test]
fn a_hundred_thousand_bytes_on_one_line_grow_the_text_by_doubling() {
    // The run cases have bigger lines; irsim keeps memory byte by byte, so it is slow here.
    let line = vec![b'q'; 100_000];
    asked(1, &[&line[..], b"\n"].concat(), &[&line]);
}

#[test]
fn a_line_one_byte_past_the_first_room_stays_inside_its_own_block() {
    // The first text has room for 64 bytes. Asking twice puts the second block right after
    // the first; an overrun by the 65th byte would show in the second length word or in the
    // first text read again at the end.
    let (a, b) = ([b'a'; 65], [b'b'; 65]);
    let ask = [Const(A, LINE), Push(A), Call("ask".into())];
    let question = [AddressOf(A, Data::Text(0)), Push(A)];
    let show = [Push(A), Call("out_text".into())];
    let body = [
        &question[..],
        &ask,
        &[Push(A)],
        &question,
        &ask,
        &show,
        &[Pop(A)],
        &show,
        &[Call("flush".into())],
    ]
    .concat();
    let input = [&a[..], b"\n", &b, b"\n"].concat();
    let o = irsim::run(&program(body, &[b"?"]), "_start", &input);
    assert_eq!((o.status, o.stderr), (0, vec![]));
    assert_eq!(o.stdout, [&b"?\n?\n"[..], &b, &a].concat());
}
