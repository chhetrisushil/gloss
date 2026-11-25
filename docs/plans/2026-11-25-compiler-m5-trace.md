# Gloss compiler M5: `gloss trace` - implementation plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task by task. Steps use checkbox (`- [ ]`) syntax for tracking. Ledger: `.superpowers/sdd/2026-11-25-compiler-m5-trace/progress.md`.

**Goal:** `gloss trace x.gloss` builds a traced executable that writes one JSON event per line (spec §10), runs it in the current folder with the user's standard input, writes the events to `x.trace` beside the source, and prints the program's own lines and a plain-English sentence for every step, indented by call depth.

**Architecture:** a third lowering mode, `Mode::Trace`, adds event code at each statement (D43-D46); a third runtime backend, `Backend::Trace`, replaces five routines by name so that everything the program prints becomes events on standard output (§4.9). The executable writes only events. A Rust narrator in `gloss` (D42) reads them line by line, writes the file and prints the sentences. `irsim` runs every trace case too and must produce the same bytes (D21).

**Tech stack:** Rust 2024, stable 1.87 or later, no crates. OrbStack Alpine machine `gloss-amd64` runs executables on the Mac (see "Running Linux-only tests on the Mac" in the design spec's M4 As built).

**Spec:** `docs/specs/2026-11-22-compiler-design.md` (binding; §4.9 and D41-D50 for this milestone). The language is defined by `spec/v0.md`: §10 (Trace, run-time failures), §11 (`gloss trace`, `--allow`, statuses), Appendix A (grammar). Conflicts about the language resolve against the language spec; about the implementation, against the design spec.

**How to read the tasks.**
- **Binding:** every exact string (event JSON, sentences, messages), the tests named in each task with their assertions, the types and signatures in each **Interfaces** block.
- **Guidance:** implementation steps give the algorithm, the hook point (file:line at `main` @ 0abe6df, from `.superpowers/sdd/2026-11-25-compiler-m5-trace/m5-facts.md`) and key code; the implementer writes the rest. Line numbers drift as tasks land: find the named function.
- A test may change only if it contradicts the language spec or the design spec. Record that as a ruling in the ledger.

## Global constraints

- [D1] Rust 2024 edition, `rust-version = "1.87"`. [D3] No dependencies of any kind (the narrator's JSON reader is hand-written). [D4] One crate.
- [D18] Generated code, the IR, `lower`, `runtime` and `irsim` speak only IR; only `src/target/` knows x86-64 and Linux numbers. M5 adds no primitive.
- [D19] Every runtime routine is written in IR.
- [D21] `irsim` is `cfg(test)` only; the real executable is the authority; both agree on every `tests/trace_cases` case, byte for byte on standard output, standard error and status.
- [D43] A traced executable writes nothing but events to standard output: every line it writes there is one complete JSON event (§4.9 table), written through the output buffer.
- [D44] A `set` event is written after the new value and any position checks, before the store; events from calls inside an expression come first. `append` carries only the new item.
- [§4.9] Real and test builds: no `trace_state` global and no trace routine; their code changes only by the `out_bytes` → `buf_bytes` and `ask` → `ask_read` splits (Tasks 2a, 7a).
- [D47] Narration shortens values; the `.trace` file never does.
- Problem strings never end with a period (as in M1 to M4); the diagnostic renderer adds it. Messages to the user are exact.

## Rules for every PR

- **One PR at a time** (owner directive). The next PR opens only after the previous one merges, branched from the new `main` (or, when the owner asks for a chain, from the previous task's branch; recorded on the `dispatched` line). Never two open at once for review.
- **Size.** At most 200 changed lines (tests and expected files included) and at most 10 files. Estimates run about 40% low, so each task states its estimate and the figure times 1.4; tasks at 140 budgeted lines or more are pre-split here. A task that still grows past the cap is split into sequential PRs (`m5/tNN-…`, `m5/tNNa-…`), recorded as a ruling. Never trim comments or tests to fit.
- **Never** `#[rustfmt::skip]` or golfed code to fit the cap; split instead.
- **Branches:** `m5/tNN-<slug>`.
- **Pull requests:** draft (`gh pr create --draft`); merged with `gh pr merge <n> --rebase` on the owner's word.
- **Commits:** one per branch, ending with
  ```
  Co-Authored-By: Claude Sonnet 5.5 <noreply@anthropic.com>
  Claude-Session: https://claude.ai/code/session_011uGukQR6EwQQdiSVNu4ERC
  ```
  PR bodies end with `🤖 Generated with [Claude Code](https://claude.com/claude-code)`, a blank line, and the session link.
- **Verify gate** at every commit: `CARGO_BUILD_JOBS=4 cargo fmt --check && cargo clippy --all-targets -- -D warnings && cargo test`. Tasks that touch the real executable also run the Linux-only tests (design spec M4 As built, "Running Linux-only tests on the Mac") and say so in the report.
- **Reviews:** implementers and reviewers use `sonnet`. Tasks marked **risky** get their **first** review on `opus`; every re-review uses `sonnet`.
- **Branch hygiene:** never delete a branch before `git log --oneline main..<branch>` prints nothing (or `git cherry` shows every commit as `-`).

## Resource limits

- At most 2 agents at once, reviews included; one `cargo` run at a time; `CARGO_BUILD_JOBS=4`. Agents never create extra cargo target directories, name any scratch script uniquely, and never kill a process they did not start.
- Tests that write files use `TempDir` (run harnesses) or the existing `temp_named` helpers (CLI tests), removed on drop.
- The Linux machine is shared: never start or stop it from a test.

## Findings that shape this plan

| # | Finding (`m5-facts.md`, verified 2026-11-25) | Consequence |
|---|---|---|
| F1 | `out_bytes` (`src/runtime/output.rs:99`) is the only place bytes enter the output buffer; `show` reaches it through `out_text`, `out_number`, `show_value` | Task 2 splits it so the trace backend can escape at one point |
| F2 | Failure text goes to fd 2 through `write_all(2, …)` from `fail_begin`, `err_*` and runtime failures | Task 6 captures it the way the test backend does (M4 Task 7b) |
| F3 | `ask` writes the question to fd 1 and flushes itself (`src/runtime/input.rs:102-110`) | Task 7a splits off `ask_read`; Trace replaces `ask` |
| F4 | Copy, show and equality are descriptor-driven runtime routines (`show_value`, `src/runtime/values.rs:296`); records hold only scalars, lists hold scalars or records | Task 3's `json_value` walks the same descriptors; no per-type lowering |
| F5 | Conditions are computed to 1/0 in `a` in `condition()` (`src/lower/cond.rs:176-181`), shared by `if` and `repeat while` | Task 10 hooks there |
| F6 | A callee never receives its call line; `f_start` is called by the runtime `_start` | Task 12 writes enter/leave at the call site; Task 1b gives Trace its own `_start` |
| F7 | Whether a `set` makes its variable cannot be decided statically inside a loop; frames start zeroed | Task 8 adds a hidden "made" flag slot per variable |
| F8 | A new global touches the enum, `name()`, two `GLOBALS` arrays and two `global_size`s; `TestState` is laid out only when used (`src/target/x86_64/program.rs:76-93`) | Task 1a adds `TraceState` after it with the same rule |
| F9 | `Mode::Test` is tested in about ten places (`m5-facts.md` §3) | Task 1b decides each for `Trace` |
| F10 | `testrun::spawn` forces null stdin and the private folder as cwd; `run_program_with` takes a `start` seam (`src/testrun.rs:133`) | Task 19 adds a trace runner through that seam |
| F11 | `gloss trace` is refused today (`src/cli.rs:59-74`), pinned by `src/cli.rs:375`, `tests/cli.rs:99-110` | Task 20 changes all three |
| F12 | Field and item `set` store with no read of the old value (`src/lower/stmt.rs:57-60`, `src/lower/list.rs:284`) | Task 9 adds the read after the position checks |
| F13 | Event helpers run below the current frame; `size_stack_checks` keeps 64 bytes for helpers (`src/lower/mod.rs:200`) | Task 6b measures the deepest event helper chain and pins failure 10 inside a traced recursion |

## File map

| File | Responsibility | Task |
|---|---|---|
| `src/ir/mod.rs`, `src/target/x86_64/program.rs`, `src/irsim/memory.rs` | `Global::TraceState` | 1a |
| `src/lower/mod.rs`, `src/lower/trace.rs` (new), `src/lib.rs` | `Mode::Trace`, `lower_trace`, trace `_start`, event helpers in lowering | 1b, 4, 8-13 |
| `src/runtime/mod.rs`, `src/runtime/trace_state.rs`, `src/runtime/trace_backend.rs` (new) | `Backend::Trace`, the swaps, `TraceState` words | 1b, 2a, 6a, 7a |
| `src/runtime/output.rs`, `src/runtime/input.rs` | `buf_bytes`, `ask_read` splits | 2a, 7a |
| `src/runtime/json.rs` (new) | `json_bytes`, `json_value`, `json_int` | 2b, 3 |
| `src/lower/stmt.rs`, `cond.rs`, `count.rs`, `list.rs`, `expr.rs`, `table.rs` | Event hooks | 4, 8-13 |
| `tests/trace_sweep.inc`, `tests/trace_cases.rs`, `src/irsim/trace_cases.rs`, `tests/trace_cases/` | Case harness and cases | 5, 6b-13 |
| `src/trace/{mod,json,event,value,narrate}.rs` (new) | The narrator | 14-18 |
| `src/ast/print.rs` | `expr_with` (printing with substitutions) | 17 |
| `tests/trace_narration.rs` | Narration golden tests | 18 |
| `src/testrun.rs`, `src/cli.rs`, `tests/cli.rs` | `run_traced`, the `gloss trace` command | 19, 20 |
| `spec/v0.md`, `README.md`, design spec §6 | Spec wording, As built | 21, 22 |

---

## Part M5a: the event stream

### Task 1: Trace mode scaffolding

**1a - the trace-state global (`m5/t01a-trace-state`).** Files: `src/ir/mod.rs`, `src/target/x86_64/program.rs`, `src/irsim/memory.rs`.

**Interfaces (produces):**
```rust
pub enum Global { …, TestState, TraceState }   // name "trace_state", after TestState
pub const TRACE_STATE_BYTES: usize = 32;
```
Both `GLOBALS` arrays gain it last; both `global_size` give it 32. `program.rs` lays it out only when some function takes `AddressOf(_, Data::Global(TraceState))`, by the same rule as `uses_test_state` (generalise the helper to take the global rather than copy it).

**Tests (binding):** the existing agreement test sees 13 globals; `trace_state_is_32_bytes_in_both_layouts`; `a_program_without_trace_state_has_the_same_layout` (a module that uses `TestState` but not `TraceState` lays out exactly as before: same addresses for every global, compare against the M4 layout numbers); an IR-text round trip of `addressof trace_state`.

**Fail-proof:** a layout that always reserves 32 bytes shifts nothing (it is last) but grows `p_memsz`; assert the total data size of a real build is unchanged.
**Review:** sonnet. **Estimate:** ~60 lines (×1.4 = 84), 3 files.

**1b - `Mode::Trace`, `Backend::Trace`, `lower_trace` (`m5/t01b-trace-mode`).** Files: `src/lower/mod.rs`, `src/lower/trace.rs` (new), `src/lower/table.rs`, `src/runtime/mod.rs`, `src/runtime/trace_backend.rs` (new), `src/lib.rs`, `src/lower/tests.rs`.

**Interfaces (produces):**
```rust
pub enum Mode { Real, Test, Trace }
pub enum Backend { Real, Test, Trace }
pub fn lower_trace(program: &Program, types: &TypeInfo, src: &Source) -> Result<Module, Diagnostic>;
// lib.rs
impl Checked { pub fn trace_module(&self) -> Result<Module, Diagnostic>; pub fn trace_executable(&self) -> Result<Vec<u8>, Diagnostic>; }
pub fn build_trace_executable(text: &str) -> Result<Vec<u8>, Vec<Diagnostic>>;
// runtime/trace_backend.rs
pub const REPLACED: [&str; 0] = [];          // grows in Tasks 2a, 6a, 7a
pub fn routines() -> Vec<Function>;          // empty for now
```
`routines_for` takes the replaced list and extra routines from `test_backend` or `trace_backend` by backend (today it is hard-wired to `test_backend`, `src/runtime/mod.rs:199`). The swap-guard asserts in `routines_swapping` apply to both.

Decide each `Mode::Test` site (F9) for `Trace`: `store_line`/`restore_line` do nothing (Real behaviour); `entry_bytes` is 16 (Real); `tables_init` sets loaded 0 (Real); `read_table`/`save_table` take the Real branch; `Expect`, `Input`, `Shown` stay test-only; `permissions_text` is not emitted. Trace `_start` (in `lower/trace.rs`, modelled on `lower/test.rs`'s driver start): `heap_init; stack_init; tables_init; call f_start; flush; exit 0` (enter/leave are added in Task 12). Use exhaustive `match mode` everywhere, never `mode != Mode::Test`.

**Tests (binding):** `a_trace_build_runs_start` (`irsim`: `show "hi"` prints `hi\n` with status 0 - no events yet); `a_trace_module_has_no_permissions_text_or_args_check`; `real_and_test_builds_have_no_trace_code` (for the §13 program, the modules from `lower` and `lower_tests` reference no `TraceState` and contain no routine from `trace_backend::routines()`); the controller also checks, per task, that a real build's executable bytes are unchanged against the base commit (as in M4); `the_trace_backend_starts_empty`.

**Fail-proof:** a Trace that silently fell back to `Mode::Test` would emit `store_line` writes to `test_state`; `a_trace_module_has_no_test_state` asserts no function references `TestState`.
**Review:** sonnet. **Estimate:** ~130 lines (×1.4 = 182: if it passes 140 in draft, move `lib.rs` and its test to 1c).

### Task 2: One escaping point

**2a - `buf_bytes` and the trace `out_bytes` (`m5/t02a-buf-bytes`).** Files: `src/runtime/output.rs`, `src/runtime/trace_state.rs` (new), `src/runtime/trace_backend.rs`, `src/runtime/mod.rs`, `src/runtime/trace_backend_tests.rs` (new).

`trace_state.rs` fixes the word layout (bytes): `ESCAPING` 0, `FAILING` 8, `OPEN` 16, with `get`/`set` helpers like `test_state.rs`. `out_bytes`'s body moves unchanged into `buf_bytes(addr, len)`; the real `out_bytes(addr, len)` calls `buf_bytes`. The trace backend replaces `out_bytes`: `ESCAPING` 0 → `buf_bytes`; 1 → `json_bytes(addr, len)` (Task 2b; in this task a stub routine `json_bytes` that calls `buf_bytes`, replaced in 2b). `REPLACED = ["out_bytes"]`.

**Tests (binding, `irsim`):** every existing output test passes unchanged; `buf_bytes_flushes_when_full` (5000 bytes through `out_bytes` in a real build arrive whole and in order); `the_trace_out_bytes_passes_through_when_not_escaping`; `the_trace_out_bytes_routes_to_json_bytes_when_escaping` (with a counting stub).
**Fail-proof:** a trace `out_bytes` that ignored the flag passes the first trace test and fails the second.
**Review:** risky (it replaces the routine every output goes through) - first review `opus`. **Estimate:** ~100 lines (×1.4 = 140: pre-split from 2b).

**2b - `json_bytes` (`m5/t02b-json-bytes`).** Files: `src/runtime/json.rs` (new), `src/runtime/mod.rs`, `src/runtime/json_tests.rs` (new).

```text
json_bytes(addr, len)   writes the bytes JSON-escaped through buf_bytes (§4.9 rule); never allocates
```
Use the existing UTF-8 decoder (`src/runtime/utf8.rs`, or `decode` in `shown.rs:105`) to decide valid sequences; reuse, do not copy.

**Tests (binding, exact bytes):** `a"b\c` → `a\"b\\c`; `\n\r\t` → `\n\r\t` (as backslash forms); byte 0x01 → `\u0001`; 0x1f → `\u001f`; 0x7f → `\u007f`; `é` (C3 A9) unchanged; `€` (E2 82 AC) unchanged; a lone 0x80 → `�`; C3 followed by `A` → `�A`; an overlong C0 80 → `��`; 5000 escaped quotes arrive whole (buffer refills).
**Fail-proof:** an escaper that passes every byte ≥ 0x80 through passes the `é` test and fails the lone-0x80 one.
**Review:** sonnet. **Estimate:** ~120 lines (×1.4 = 168: if past 140, move the buffer-refill test and the overlong case to Task 3).

### Task 3: `json_value` (`m5/t03-json-value`)

**Files:** `src/runtime/json.rs`, `src/runtime/mod.rs` (texts), `src/runtime/json_tests.rs`.

```text
json_int(n)              writes the integer n in decimal (no fixed point): line, item, turn, rows, failure
json_text(text)          writes `"`, json_bytes(text bytes), `"`
json_value(value, desc)  writes the typed value of §4.9 by walking desc like show_value
```
Numbers use `number_format` (the show form, `src/runtime/numbers.rs`) inside `{"number":"…"}`; truths `{"truth":"yes"}`/`{"truth":"no"}`; records `{"record":"<name>","fields":{"<field>":v,…}}` in declaration order, names from the descriptor; lists `{"list":[v,v]}`, `{"list":[]}` when empty. Fixed JSON pieces are runtime texts appended to `TEXTS`.

**Tests (binding, `irsim`, exact bytes):** number 120.5 → `{"number":"120.5"}`; -3 → `{"number":"-3"}`; 0 → `{"number":"0"}`; text `say "hi"` → `{"text":"say \"hi\""}`; truth → `{"truth":"yes"}`; an `Expense` record (name text, amount number, paid truth) → `{"record":"Expense","fields":{"name":{"text":"tea"},"amount":{"number":"2.5"},"paid":{"truth":"no"}}}`; a list of numbers `[1, 2]` → `{"list":[{"number":"1"},{"number":"2"}]}`; an empty list → `{"list":[]}`; a list of two records; `json_int` of 0, 7, 1234567, -10.
**Fail-proof:** a writer that used `out_quoted` (doubling quotes) for text fails the quote test; one that wrote fields in name order fails the record test (fields are not alphabetical).
**Review:** sonnet. **Estimate:** ~135 lines (×1.4 = 189: split at the record/list half into 3a scalars and 3b compound if it passes 140).

### Task 4: Event framing and `show` events (`m5/t04-show-event`)

**Files:** `src/lower/trace.rs`, `src/lower/stmt.rs`, `src/runtime/trace_backend.rs` (helpers), `src/lower/tests.rs`.

**Interfaces (produces, used by every later event task):**
```rust
// lower/trace.rs, on Lowerer, all emit nothing outside Mode::Trace
fn ev_open(&mut self, line: u32, kind: &str);     // writes {"line":L,"event":"<kind>"
fn ev_key(&mut self, key: &str);                   // writes ,"<key>":
fn ev_fixed_text(&mut self, s: &str);              // writes a compile-time JSON string, escaped by the compiler
fn ev_value_from_slot(&mut self, slot: u32, ty: &Ty); // json_value(LoadSlot(slot), type_desc(ty))
fn ev_int_from_slot(&mut self, slot: u32);         // json_int
fn ev_close(&mut self);                            // writes }\n
```
Fixed pieces are made at compile time (the compiler escapes names and conditions with a Rust twin of the §4.9 rule, `json_escape_host`, unit tested against `json_bytes` on the same inputs) and written with `out_text`, so the escaper never sees them. Helpers clobber `a` and `b`: callers keep values in hidden slots (`new_temp`).

`show` in Trace mode: `ev_open(line, "show")`, `,"text":"`, set `OPEN` and `ESCAPING`; the items as today; clear `ESCAPING` and `OPEN`; `"}` and newline. The trailing `"\n"` that a real `show` writes is not written.

**Tests (binding, `irsim`, stdout exact):** `show "hi"` → `{"line":2,"event":"show","text":"hi"}\n`; `show "a", 1.5, yes` → `…"text":"a1.5yes"}`; `show` of a text holding `"` → escaped; `show` of a record (source form with quotes, escaped); two shows give two lines; `json_escape_host_agrees_with_json_bytes` on the Task 2b inputs.
**Fail-proof:** forgetting to clear `ESCAPING` makes the closing `"}` come out as `\"}`: the first test catches it.
**Review:** sonnet. **Estimate:** ~120 lines (×1.4 = 168: if past 140, move `json_escape_host` and its agreement test to 4a first).

### Task 5: The trace case harness

**5a - sweep and real harness (`m5/t05a-trace-harness`).** Files: `tests/trace_sweep.inc` (new), `tests/trace_cases.rs` (new), `src/lib.rs` (if a library entry is needed), `tests/trace_cases/show_line.{gloss,trace,out}`.

A case is `tests/trace_cases/<name>.gloss` with `<name>.trace` (required: the exact stdout of the traced executable), `<name>.out` (required: the narration, checked from Task 18 on; until then the sweep only checks it exists), optional `<name>.in` (stdin), `<name>.status` (default 0), `<name>.err` (default empty: the traced executable's stderr), and an optional `<name>.files/` folder copied into the working folder first. The sweep (`trace_sweep()`) rejects any other suffix and asserts at least `MIN_TRACE_CASES` cases (starts at 1; each task that adds cases raises it). `tests/trace_cases.rs` builds with `build_trace_executable`, runs through `run_support::run_case` with the case's stdin and folder, and compares stdout to `.trace`, stderr to `.err`, status to `.status`.

**5b - the `irsim` twin (`m5/t05b-trace-irsim`).** Files: `src/irsim/trace_cases.rs` (new), `src/irsim/mod.rs`, `tests/trace_cases/show_values.{gloss,trace,out}`.

Same sweep through `include!`, seeding `Files` from `.files`, `run_with(&module, "_start", input, &[], Faults::default(), &mut files)`; same three comparisons.

**Tests (binding):** `every_trace_case_matches_on_linux`, `every_trace_case_matches_in_irsim`, `the_trace_sweep_rejects_unknown_suffixes`. Cases: `show_line` (`show "hello"`), `show_values` (a number, a text with a quote, a truth, a record, a list in one program).
**Fail-proof:** the sweep floor makes an empty or skipped folder fail; an `.out` that is never read is caught in Task 18 (the sweep there asserts each `.out` was compared).
**Review:** sonnet. **Estimate:** 5a ~120 lines (×1.4 = 168, 6 files); 5b ~90 lines (×1.4 = 126).

### Task 6: `fail` events

**6a - the swaps (`m5/t06a-fail-event`).** Files: `src/runtime/trace_backend.rs`, `src/runtime/trace_backend_tests.rs`, `src/runtime/mod.rs` (texts).

`REPLACED` gains `write_all`, `fail_begin`, `fail_end`, with the behaviour in §4.9's table. `write_all` in Trace: fd 2 with `FAILING` → `out_bytes` (escaped, since `ESCAPING` is set); fd 2 otherwise → `flush`, then the real write loop; fd 1 → the real write loop. Reuse the real loop (move it into a helper both call), do not copy it. `fail_begin(line, code)`: if `OPEN`: clear `ESCAPING`, write `"}` and newline, clear `OPEN`; then write `{"line":<json_int line>,"event":"fail","failure":<json_int code>,"message":"`, set `FAILING` and `ESCAPING`. `fail_end()`: clear `ESCAPING`, write `"}` and newline, flush, exit 2.

**Tests (binding, `irsim`):** `a_failure_is_one_fail_event` (`fail_begin(5, 1)`, `err_text("dividing by zero (n is 0)")`, `fail_end()`: stdout exactly `{"line":5,"event":"fail","failure":1,"message":"dividing by zero (n is 0)"}\n`, stderr empty, status 2); `err_int_and_err_number_go_into_the_message`; `a_failure_inside_a_show_closes_it_first` (an open show with `a` written, then a failure: two lines, the first `…"text":"a"}`); `a_quote_in_the_message_is_escaped`; `a_notice_on_fd_2_flushes_events_first` (a pending event then `write_all(2, …)` outside a failure: the event reaches fd 1 before the notice reaches fd 2, by the order of `irsim`'s write log).
**Fail-proof:** a `fail_begin` that forgot the `OPEN` check leaves a show line unterminated; the third test catches it.
**Review:** risky - first review `opus`. **Estimate:** ~130 lines (×1.4 = 182: if past 140, move the notice rule and its test to 6b).

**6b - failure cases and the stack margin (`m5/t06b-fail-cases`).** Files: `tests/trace_cases/` (cases), `tests/trace_sweep.inc` (raise the floor), `src/lower/mod.rs` (only if the margin must change).

Cases: `fail_divide` (failure 1 after a show), `fail_inside_show` (`show "a", 1 / n` with n 0), `fail_item` (failure 2), `fail_too_deep` (a recursion that ends in failure 10; its `.trace` holds only the show events before it and the fail event). Measure the deepest stack an event helper chain uses below a frame (`json_value` of a list of records → `json_bytes` → `buf_bytes` → `flush` → `write_all`), and if it exceeds the 64 bytes `size_stack_checks` keeps (F13), raise the margin in Trace mode only and record the number in a comment. `fail_too_deep` must pass on Linux and in `irsim`.
**Review:** sonnet. **Estimate:** ~100 lines (×1.4 = 140), 10 files (3 per case: watch the file cap; split the fourth case into its own PR if needed).

### Task 7: `ask` events

**7a - `ask_read` and the trace `ask` (`m5/t07a-ask-read`).** Files: `src/runtime/input.rs`, `src/runtime/trace_backend.rs`, `src/runtime/input_tests.rs`, `src/runtime/trace_backend_tests.rs`.

The real `ask(question, line)`: `out_text(question)`, `out_text("\n")`, `flush`, `ask_read(line)` (the old read loop, moved unchanged). Trace `ask` (`REPLACED` gains it): writes `{"line":L,"event":"show","text":"` + escaped question + `"}` and newline, `flush`, `ask_read(line)`. The answer is returned in `a` as before.

**Tests (binding):** all existing input tests pass; `the_trace_ask_shows_the_question_as_an_event_then_reads` (`irsim`, stdin `tea\n`: stdout `{"line":3,"event":"show","text":"What?"}\n`, answer `tea`); `the_question_is_flushed_before_the_read` (`irsim`'s write log: the write precedes the first read); `input_ended_is_a_fail_event` (empty stdin: the show event, then `{"line":3,"event":"fail","failure":9,"message":"input ended while ask was waiting"}`).
**Review:** risky - first review `opus`. **Estimate:** ~110 lines (×1.4 = 154).

**7b - the `ask` event (`m5/t07b-ask-event`).** Files: `src/lower/stmt.rs`, `src/lower/trace.rs`, cases `ask_answer` and `ask_twice` (with `.in`), `tests/trace_sweep.inc`.

Trace lowering of `ask q into x`: evaluate `q` into a hidden slot; call `ask`; store the answer in `x`'s slot and set its "made" flag (Task 8 adds the flags; until then this task writes no flag); write `{"line":L,"event":"ask","name":"x","question":<json_text q>,"answer":<json_text answer>}`.
**Review:** sonnet. **Estimate:** ~110 lines (×1.4 = 154).

### Task 8: `set` events on variables (`m5/t08-set-var`)

**Files:** `src/lower/mod.rs` (made-flag slots), `src/lower/stmt.rs`, `src/lower/trace.rs`, `src/lower/tests.rs`, cases `set_number`, `set_in_loop` (the second turn has `from`), `add_subtract`, `multiply_divide`, `tests/trace_sweep.inc`.

In Trace mode, `assign_slots` gives each made variable (`FeatureInfo.vars`) a second hidden slot, its made flag (frames start zeroed). `set x to e`: compute `e` (with copy) into a hidden slot; `ev_open(line,"set")`; `,"name":"x"`; if the flag is 1: `,"from":` + `json_value(old)`; `,"to":` + `json_value(new)`; `}`; store; set the flag. `add a to x` and the other verbs: evaluate the operand; save the old value in a hidden slot; do the arithmetic (failure 4 jumps away before any event); write the event with `from` (always: an arithmetic verb needs `x` to exist) and `to`; store. `ask … into x` (Task 7b) sets the flag.

**Tests (binding):** the four cases (`.trace`, and `.out` holding the expected sentences from §4.9, compared from Task 18 on); `a_set_inside_a_call_comes_after_the_calls_events` (`set x to double with 2` - with Task 12 not yet landed, assert only that the set event is last); `failure_4_writes_no_set_event`.
**Fail-proof:** writing the event before the store but after computing `e` is the binding order; a version that reads `from` after the store shows from = to, caught by `set_number` (`total becomes 150` from 0).
**Review:** risky (D44 ordering) - first review `opus`. **Estimate:** ~140 lines (×1.4 = 196, 10 files: split the cases for multiply/divide into 8b).

### Task 9: `set` events on fields and items, and `append`

**9a - fields and items (`m5/t09a-set-place`).** Files: `src/lower/stmt.rs`, `src/lower/list.rs`, `src/lower/record.rs`, `src/lower/trace.rs`, cases `set_field`, `set_item`, `set_field_of_item`, `item_failure_no_event`.

The value first, then the position checks (`item_address`: failures 2 and 3), then read the old value at the address, write the event (`"name"`, then `"item":N` with the checked whole position as a JSON integer, then `"field"`), then store. For the arithmetic verbs on a field or item, the same order with the old value saved before the arithmetic. The item number written is the position as the program computed it (a whole number), via `json_int` of position / SCALE.

**9b - `append` (`m5/t09b-append`).** Files: `src/lower/list.rs`, `src/lower/trace.rs`, cases `append_numbers`, `append_to_table`.

After `list_push`, read the new count (`[list + 0]`) and write `{"line":L,"event":"set","name":"prices","item":N,"to":V}` with the pushed item from its hidden slot.
**Tests (binding):** the cases; `append_writes_only_the_new_item` (a list of 3 records: the event holds one record).
**Review:** sonnet (both). **Estimate:** 9a ~140 (×1.4 = 196: the two verbs-on-places cases may move to 9c); 9b ~90 (×1.4 = 126).

### Task 10: `check` events (`m5/t10-check`)

**Files:** `src/lower/cond.rs`, `src/lower/trace.rs`, `src/lower/tests.rs`, cases `check_if_yes_no`, `check_otherwise_if`, `check_repeat_while`, `check_values_fields`, `tests/trace_sweep.inc`.

In `condition()`, after `expr(cond)` leaves 1/0 in `a` and only in Trace mode: save `a` in a hidden slot; `ev_open(cond.line,"check")`; `,"condition":` + the compile-time `ast::print::expr(cond)` escaped; `,"values":{` + for each value name (collect with `ast::walk::for_each_expr`: `ExprKind::Name` whose `types.names` kind is `Variable`, `LoopVariable` or `Input`, and `FieldOf` chains whose root is such a name; distinct by printed form, in order of first appearance) `"<printed>":` + its value read again now (`LoadSlot`/`LoadArg` and field loads only: never re-evaluate a call or an item) + `}`; `,"result":"yes"|"no"` from the saved slot; `}`; restore `a`. `Expect` (test-only) is untouched.

**Tests (binding):** the cases; `a_condition_with_a_call_reads_no_call_twice` (a feature called in the condition runs once: count its show events); `item_reads_are_not_in_values`; `and_or_conditions_list_each_name_once`.
**Fail-proof:** a values list built by re-lowering the subexpressions would call features twice; the second test catches it.
**Review:** sonnet. **Estimate:** ~130 lines (×1.4 = 182: cases may spill into 10b).

### Task 11: `loop` events (`m5/t11-loop`)

**Files:** `src/lower/count.rs`, `src/lower/list.rs`, `src/lower/trace.rs`, cases `loop_range`, `loop_list`, `loop_skip_stop`, `tests/trace_sweep.inc`.

After the loop variable is stored and before `loop_body`: `{"line":L,"event":"loop","name":"<var>","turn":N,"value":V}`. Counting loops keep a hidden turn slot (1 at the first turn, +1 per turn; Trace only); list loops write `at + 1`. `L` is the `for each` line.
**Tests (binding):** the cases (`skip` still writes the next turn's event; `stop` writes none after); a zero-turn loop writes nothing.
**Review:** sonnet. **Estimate:** ~100 lines (×1.4 = 140).

### Task 12: `enter` and `leave` (`m5/t12-calls`)

**Files:** `src/lower/expr.rs` (`feature_call`), `src/lower/trace.rs` (`_start` gains start's pair), `src/lower/tests.rs`, cases `call_gives`, `call_nothing`, `call_recursion`, `call_in_condition`, `tests/trace_sweep.inc`.

In `feature_call`, Trace only: evaluate each input into a hidden slot (instead of pushing as it goes), then the `StackCheck` (failure 10 before any event), then `{"line":L,"event":"enter","feature":"f","inputs":{"a":V,…}}` with input names from `FeatureInfo.inputs`, push the slots, call, save `a`, `{"line":L,"event":"leave","feature":"f","result":V}` (no `result` when `FeatureInfo.result` is `Nothing`), restore `a`. `L` is `self.line`. The trace `_start` writes `enter` and `leave` for `start` with the `feature start` line.
**Tests (binding):** the cases; `call_recursion` (gcd from the README) pins nesting order; `call_in_condition` pins that the callee's events come before the `check` event.
**Fail-proof:** an `enter` written before the `StackCheck` makes `fail_too_deep`'s `.trace` (Task 6b) gain an `enter`: that case is the check.
**Review:** risky - first review `opus`. **Estimate:** ~130 lines (×1.4 = 182: split `_start`'s pair into 12b if past 140).

### Task 13: `file` events (`m5/t13-file`)

**Files:** `src/lower/table.rs`, `src/lower/trace.rs`, cases `file_load_save` (with `.files/`), `file_clear`, `file_missing` (loads an absent file: 0 rows; the not-found notice is on stderr, in `.err`), `tests/trace_sweep.inc`.

After `load`, `save` or `clear` succeeds: `{"line":L,"event":"file","table":"<name>","file":"<display_name(path)>","action":"load"|"save"|"clear","rows":N}` with `N` read from the table's list count.
**Review:** sonnet. **Estimate:** ~110 lines (×1.4 = 154).

---

## Part M5b: the narrator and `gloss trace`

### Task 14: The JSON reader (`m5/t14-json`)

**Files:** `src/trace/mod.rs` (new), `src/trace/json.rs` (new), `src/lib.rs` (`pub mod trace;`).

```rust
pub enum Json { Null, Bool(bool), Int(i64), Str(String), Array(Vec<Json>), Object(Vec<(String, Json)>) }
pub fn parse(line: &str) -> Result<Json, String>;   // one value, nothing after it
```
Objects keep key order. Strings: every escape of RFC 8259, `\uXXXX` with surrogate pairs; a lone surrogate is an error. Numbers: integers only (events never write fractions or exponents); anything else is an error.
**Tests (binding):** each spec §10 example line parses; escapes (`\"`, `\\`, `\n`, `é`, `😀`, `�`); errors for trailing text, a lone surrogate, `1.5`, an unclosed string.
**Review:** sonnet. **Estimate:** ~150 lines (×1.4 = 210: split the error tests into 14b if past 140).

### Task 15: Typed events (`m5/t15-events`)

**Files:** `src/trace/event.rs` (new), `src/trace/mod.rs`.

```rust
pub enum Value { Number(String), Text(String), Truth(bool), List(Vec<Value>), Record { name: String, fields: Vec<(String, Value)> } }
pub enum Event {
    Set { line: u32, name: String, item: Option<i64>, field: Option<String>, from: Option<Value>, to: Value },
    Check { line: u32, condition: String, values: Vec<(String, Value)>, result: bool },
    Loop { line: u32, name: String, turn: i64, value: Value },
    Enter { line: u32, feature: String, inputs: Vec<(String, Value)> },
    Leave { line: u32, feature: String, result: Option<Value> },
    Show { line: u32, text: String },
    Ask { line: u32, name: String, question: String, answer: String },
    File { line: u32, table: String, file: String, action: FileAction, rows: i64 },
    Fail { line: u32, failure: i64, message: String },
}
pub fn decode(line: &str) -> Result<Event, String>;
```
**Tests (binding):** one decode per event kind from §4.9's shapes; an unknown event, a missing key and a wrong type are errors naming the key.
**Review:** sonnet. **Estimate:** ~140 lines (×1.4 = 196: split values (15a) from events (15b)).

### Task 16: Values in sentences (`m5/t16-value-text`)

**Files:** `src/trace/value.rs` (new), `src/irsim/…` test only.

`pub fn say(v: &Value) -> String`: as `show` writes the value (numbers as given, `yes`/`no`, text in quotes with `"` doubled, records and lists exactly as `show_value` writes them), then D47's shortening: a list with more than 3 items shows its first 3 and `… (N items)`; text longer than 60 characters keeps 60 and ends with `…`.
**Tests (binding):** `say_agrees_with_show` (for a number, a text with a quote, a truth, a record, a list of 2 records: build the same value in a Gloss program, run its `show` in `irsim`, compare to `say` of the decoded `json_value` event of the same value); `a_long_list_is_shortened` (`[1, 2, 3, … (5 items)]`, exact form following `show`'s list syntax); `a_long_text_is_cut` (61 characters, multi-byte characters counted as one).
**Review:** sonnet. **Estimate:** ~120 lines (×1.4 = 168).

### Task 17: Conditions with values (`m5/t17-expr-with`)

**Files:** `src/ast/print.rs`, `src/trace/narrate.rs` (new, the check sentence only).

```rust
pub fn expr_with(e: &Expr, sub: &dyn Fn(&Expr) -> Option<String>) -> String;  // expr() = expr_with(e, &|_| None)
```
The narrator finds the condition for an event's line (the `if`/`otherwise if`/`repeat while` whose condition starts on that line; `Narrator::new(&Program)` builds the map) and replaces each name or field read whose printed form is in `values` by `say` of its value.
**Tests (binding):** `price is greater than 100` with price 150 → `150 is greater than 100`; `name of e is "tea" and paid` → `"tea" is "tea" and yes`; a single-name condition `paid` → `paid is yes` / `paid is no`; a call in the condition stays as written; `ast::print::expr` output is unchanged for every spec example (existing tests).
**Review:** sonnet. **Estimate:** ~110 lines (×1.4 = 154).

### Task 18: The narrator (`m5/t18-narrate`)

**18a - sentences (`m5/t18a-sentences`).** Files: `src/trace/narrate.rs`.
```rust
pub struct Narrator<'p> { … }
impl<'p> Narrator<'p> {
    pub fn new(program: &'p Program) -> Self;
    pub fn line(&mut self, event_line: &str, out: &mut dyn Write, err: &mut dyn Write) -> io::Result<Result<(), String>>;
}
```
Every sentence of §4.9's table, exactly, with the indentation rule (2 spaces per open call below `start`; `start`'s enter and leave print nothing). Unit tests: one per row of the table, plus nesting depth 2.

**18b - golden narration (`m5/t18b-narration-cases`).** Files: `tests/trace_narration.rs` (new), `tests/trace_sweep.inc` (assert every `.out` is compared), and any `.out` corrections.

For each trace case: parse and check the `.gloss`, feed each line of `.trace` to a `Narrator`, compare stdout to `.out` and stderr to the fail line (if any). Runs on every host.
**Review:** sonnet. **Estimate:** 18a ~150 (×1.4 = 210: split check/set/append (18a) from the rest (18c)); 18b ~60 (×1.4 = 84).

### Task 19: Running the traced executable (`m5/t19-run-traced`)

**Files:** `src/testrun.rs` (or `src/testrun/trace.rs`), its tests.

```rust
pub fn run_traced(executable: &[u8], on_line: &mut dyn FnMut(&str) -> io::Result<()>, err: &mut dyn Write) -> io::Result<TraceEnd>;
pub enum TraceEnd { Exited(i32), Unfinished(i32) }
```
Through `run_program_with`'s `start` seam: the executable in a `PrivateDir`, the current folder as working directory, standard input inherited, standard output read and split at `\n` (a chunk may end mid-line; keep the rest), standard error passed through. A last fragment with no newline, or a status other than 0 and 2, is `Unfinished(status)` (signal: `256 + signal`, as `status_for`).
**Tests (binding, Linux):** a program that writes two events and exits 0; a line split across two writes arrives whole; an executable killed by a signal is `Unfinished(265)`; stdin reaches the program (`ask`).
**Review:** sonnet. **Estimate:** ~130 lines (×1.4 = 182: split the tests into 19b if past 140).

### Task 20: The `gloss trace` command

**20a - shared preparation (`m5/t20a-prepare`).** Files: `src/cli.rs`.
Move `build`'s diagnostics, `.gloss` rule, `check_flags`, permission list and `approve` steps into `fn prepare(path, options, prompt, out, err) -> Result<Checked, i32>`; `build` calls it. Every existing CLI test passes unchanged.

**20b - `trace` (`m5/t20b-trace-command`).** Files: `src/cli.rs`, `tests/cli.rs`.
`"trace" => 0` in the milestone table; `trace()`: `prepare`; host check (`gloss trace runs programs on Linux only`, status 3); `trace_executable`; create `<source>.trace` (`path.with_extension("trace")`; failure `<name>.trace cannot be written`, status 3); `run_traced` with a `Narrator` writing each line to the file and narrating; status 0 → 0, `Fail` seen → 2, `Unfinished(n)` → `the traced run ended without finishing (status n)` and 2.
**Tests (binding):** update `trace_accepts_allow` and `usage_and_unbuilt_commands_exit_3` (trace is no longer unbuilt); `trace_refuses_on_a_host_that_cannot_run_it` (the deciding function, unit tested); on Linux: `trace_narrates_and_writes_the_trace_file` (a show + a set + a check), `trace_of_a_failure_exits_2`, `trace_with_a_table_asks_for_approval` (non-terminal: status 4), `trace_with_allow_runs`.
**Review:** sonnet. **Estimate:** 20a ~80 (×1.4 = 112); 20b ~140 (×1.4 = 196: split the Linux tests into 20c).

### Task 21: The language spec (`m5/t21-spec`)

**Files:** `spec/v0.md`, `tests/spec_examples.rs`, `tests/trace_cases/spec_trace.{gloss,trace,out}`.
§10 Trace gains: the sentence table of §4.9 (as the reader sees it), the event shapes and key order, `append` as a `set` with `item`, `ask`'s question as a `show` event, `values` rules, D47's shortening, the indentation rule. §11 gains the `gloss trace` messages and statuses (`gloss trace runs programs on Linux only`, `the traced run ended without finishing (status N)`, `x.trace cannot be written`). The §10 three-line example becomes a trace case whose `.out` contains those three lines exactly.
**Review:** sonnet. **Estimate:** ~120 lines (×1.4 = 168).

### Task 22: As built (`m5/t22-as-built`)

**Files:** `README.md` (milestone 5 Done, a Trace section with a short example), design spec §6 ("As built - Milestone 5": rulings, deviations, known limits including the stderr-notice ordering and the absence of an event cap), `docs/plans/…` (none).
**Review:** sonnet. **Estimate:** ~100 lines.

---

## Rulings made while planning

- P1: `json_value` walks the existing type descriptors at run time (F4) instead of a serializer generated per type: the descriptors already carry record and field names, and `show_value` is the model. Cost if wrong: a later per-type writer replaces one routine.
- P2: one escaping point, the trace `out_bytes` (F1), switched by `ESCAPING`; fixed JSON pieces are escaped by the compiler. Cost if wrong: some show path that bypasses `out_bytes` would write raw bytes into an event; Task 4's record and list tests would show it.
- P3: invalid UTF-8 bytes become `�` (§4.9). Cost if wrong: the `.trace` file loses the exact bytes of a bad answer; they were never valid text.
- P4: only a `show` event is open while user code runs; a failure closes it (§4.9). Cost if wrong: none found; every other event is written from saved values.
- P5: enter/leave at the call site (F6); `start`'s pair from the trace `_start`, not narrated.
- P6: a hidden made flag per variable (F7) rather than a static rule.
- P7: notices on fd 2 (the not-found notice) stay on standard error; the trace `write_all` flushes events first. Their order against the narration is only as good as the two pipes allow; recorded as a known limit.
- P8: a check event's values are read after the condition is evaluated, and never by re-evaluating calls or items.
