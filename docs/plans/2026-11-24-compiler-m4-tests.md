# Gloss compiler M4: `gloss test` - implementation plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task by task. Steps use checkbox (`- [ ]`) syntax for tracking. Ledger: `.superpowers/sdd/2026-11-24-compiler-m4-tests/progress.md`.

**Goal:** `gloss test x.gloss` builds a test executable and runs every `test` block, each in its own forked process, printing `pass` or `fail` with details (a failed `expect`, a run-time failure, what the test printed), with `input` answers for `ask`, `shown` for what was printed, and pretend in-memory files for tables, and exits 0, 1 or 3 as the language spec says.

**Architecture:** `lower` gets a test mode whose entry is a driver (D31) that forks once per test; the child runs the test and prints its own block, the parent counts. Two new primitives, `Fork` and `Wait` (D32), are the only target-specific code. Everything else is runtime written in IR (D19): the report routines, and a second backend (D30, D36) that replaces a handful of runtime routines by name in test builds (`write_all`, `fail_begin`, `fail_end`, `ask`) and lowers `load` and `save` to pretend-file routines that share the real CSV writer and parser. `irsim` models fork by cloning the machine (D21), so every case also runs without Linux.

**Tech stack:** Rust 2024, stable 1.87 or later, no crates. OrbStack Alpine machine `gloss-amd64` (QEMU, D17 as amended in M3) runs executables on the Mac; CI runs them natively.

**Spec:** `docs/specs/2026-11-22-compiler-design.md` (binding; §4.8 and D30–D40 for this milestone). The language is defined by `spec/v0.md`: §9 (tests), §10 (run-time failures, output), §11 (`gloss test`, statuses), §13 (the complete example), Appendix A (grammar). Conflicts about the language resolve against the language spec; about the implementation, against the design spec.

**How to read the tasks.**
- **Binding:** every exact string (messages, report lines), the tests named in each task with their assertions, the types and signatures in each **Interfaces** block, and the primitive conventions in Tasks 1 to 3.
- **Guidance:** implementation steps give the algorithm and key code; the implementer writes the rest.
- A test may change only if it contradicts the language spec or the design spec. Record that as a ruling in the ledger.

## Global constraints

- [D1] Rust 2024 edition, `rust-version = "1.87"`. [D3] No dependencies of any kind. [D4] One crate.
- [D18] Nothing outside `src/target/` names an x86-64 register, instruction, system-call number or Linux flag value, or decodes a wait status. `lower`, `runtime` and `irsim` speak only IR.
- [D19] Every runtime routine is written in IR. The only new target-specific code is `Fork` and `Wait` (Task 3).
- [D21] `irsim` is `cfg(test)` only; the real executable is the authority; both agree on every `tests/test_cases` case, byte for byte on standard output, standard error and status.
- [D31] The test executable's entry is the driver; `start` runs only when a test says `run start`. Each child starts from the parent's untouched state.
- [D33] The report format is exactly the owner's preview; the `shown:` part appears only if the test printed something.
- [D34] A failed `expect`, a run-time failure, a missing answer: each fails only its own test, with its message under the title. Nothing from a test goes to standard error.
- [D36] A test build contains no real file primitive: its IR has no `Open`, `Rename`, `Sync`, `Mode` or `SetMode`, and standard input is never read.
- [D37] `gloss test` never writes beside the source and never reads or writes `.allow`.
- Problem strings never end with a period (as in M1 to M3); the diagnostic renderer adds it.

## Rules for every PR

- **One PR at a time** (owner directive). The next PR opens only after the previous one merges; branched from the new `main`. Never stacked.
- **Size.** At most 200 changed lines (tests and expected files included) and at most 10 files. Estimates run about 40% low, so each task below states its estimate and the same figure times 1.4; a task whose budgeted figure is within 30% of the cap (140 lines or more) is pre-split in this plan. A task that still grows past the cap is split into sequential PRs (`m4/tNN-…`, `m4/tNNa-…`), recorded as a ruling. A run case is two or three files (`.gloss`, `.out`, `.status`); budget files, not just lines.
- **Never** `#[rustfmt::skip]` or golfed code to fit the cap; split instead.
- **Branches:** `m4/tNN-<slug>` from `origin/main`.
- **Pull requests:** draft (`gh pr create --draft`); merged with `gh pr merge <n> --rebase` on the owner's word.
- **Commits** end with:
  ```
  Co-Authored-By: Claude Sonnet 5.5 <noreply@anthropic.com>
  Claude-Session: https://claude.ai/code/session_011uGukQR6EwQQdiSVNu4ERC
  ```
  PR bodies end with `🤖 Generated with [Claude Code](https://claude.com/claude-code)`, a blank line, and the session link.
- **Verify gate** before a task is done, at every commit: `CARGO_BUILD_JOBS=4 cargo fmt --check && cargo clippy --all-targets -- -D warnings && cargo test`. The report says whether the run tests ran or were skipped.
- **Reviews:** implementers and reviewers use `sonnet`. Tasks marked **risky** get their **first** review on `opus` (fork and wait, the test backend's swaps, the pretend-file backend); every re-review uses `sonnet`.
- **Branch hygiene:** never delete an implementer's branch before `git log --oneline main..<branch>` prints nothing.

## Resource limits

- At most 2 agents at once, reviews included; one `cargo` run at a time; `CARGO_BUILD_JOBS=4`.
- Tests that write files use `TempDir` under `CARGO_TARGET_TMPDIR` (run tests) or the existing `temp_named` helpers (CLI tests), removed on drop.
- The Linux machine is shared: never start or stop it from a test. A test that forks must finish; there is no time limit in the product (D40), so every harness runs with the existing `DEADLINE`.

## Findings that shape this plan

| # | Finding (verified 2026-11-24, `.superpowers/sdd/m4-facts.md`) | Consequence |
|---|---|---|
| F1 | `lower_program` iterates `program.features()` only (`src/lower/mod.rs:210`); test bodies are never lowered, and `Expect`, `Input` and `Shown` have no lowering (`src/lower/stmt.rs:160`) | Task 8 lowers test bodies as `t_<i>`; Tasks 9, 15, 17 add the statements |
| F2 | Slots and variable types come from `TypeInfo.features` by name (`src/lower/mod.rs:159,172`); tests have `TestInfo { vars, loops }` | Task 8a generalises both helpers |
| F3 | Failure text reaches fd 2 by four routes, and `err_int` skips `write_all` through its own `Write` (`src/runtime/output.rs:173`) | Task 5b routes it through `write_all` so one swap catches every byte |
| F4 | `fail_begin` flushes first (`src/runtime/output.rs:251`) | With `write_all` swapped, pending `show` output reaches `shown` before a failure's text (Task 7b) |
| F5 | `add_reachable` picks routines by name, with no mode (`src/runtime/mod.rs:164`) | Task 5a adds `Backend`; the test backend replaces routines by name |
| F6 | The global layout is a fixed array duplicated in `src/target/x86_64/program.rs:14` and `src/irsim/memory.rs:17` | Task 4 adds one global, `TestState`, with the existing agreement test extended |
| F7 | `tables_init` sets the loaded flag to 0 and has no pretend-file word (`src/lower/table.rs:205`) | Task 10 sets flag 1 in test mode; Task 19b widens an entry to three words |
| F8 | `table_read` reads bytes with `file_bytes`, then parses (`src/runtime/table_read.rs:111-130`) | Task 18 splits it at `csv_cursor`; the pretend `load` feeds the parser from memory |
| F9 | `Machine` is not `Clone`; `Files` is (`src/irsim/files.rs:17`) | Task 2 derives `Clone` and runs the clone inside the `Fork` call |
| F10 | Integration tests cannot see `irsim`; `run_sweep.inc` rejects unknown suffixes | Task 12 adds a library entry (`build_test_executable`), a sweep for `tests/test_cases`, and an `irsim` twin |
| F11 | `gloss test` is refused today with status 3 (`src/cli.rs:57-77`), and `tests/cli.rs:98` asserts it | Task 11b changes both |
| F12 | A real `ask` in a test executable would read standard input, and a real `load` would open files | Test-mode lowering refuses `ask`, `load` and `save` (and `input`, `shown`) in Task 8a, and Tasks 15 and 19b lift each one |

## File map

| File | Responsibility | Task |
|---|---|---|
| `src/ir/mod.rs`, `src/ir/text.rs` | `Prim::Fork`, `Prim::Wait`; `Global::TestState` | 1, 4 |
| `src/irsim/machine.rs`, `memory.rs`, `system.rs`, `files.rs` | Fork by cloning | 2 |
| `src/target/x86_64/linux.rs`, `difftest`, `lower_tests.rs`, `program.rs` | `fork`, `wait4`, the wait-status decoding; the new global | 1, 3, 4 |
| `src/runtime/mod.rs` | `Backend`, `routines_for`, texts | 5a, 6 |
| `src/runtime/output.rs` | `err_int` through `write_all` | 5b |
| `src/runtime/report.rs` | Report routines (direct writes to standard output) | 6a, 6b |
| `src/runtime/test_state.rs` | `shown_add`, `fail_add` and the state layout | 7a |
| `src/runtime/test_backend.rs` | The replaced routines, `test_fail`, `test_expect`, `test_ask`, `test_input`, `shown_list`, the pretend-file routines | 7b, 15, 17, 19a |
| `src/lower/mod.rs`, `src/lower/test.rs` | `Mode`, `lower_tests`, `t_<i>`, the driver | 8 |
| `src/lower/stmt.rs`, `expr.rs`, `table.rs` | `expect`, `input`, `shown`, cleared tables, pretend load and save | 9, 10, 15, 17, 19b |
| `src/lib.rs`, `src/testrun.rs`, `src/cli.rs`, `tests/cli.rs` | `Checked::test_executable`, `build_test_executable`, the `gloss test` command | 8b, 11 |
| `tests/test_sweep.inc`, `tests/test_cases.rs`, `src/irsim/test_cases.rs`, `tests/test_cases/` | Case harness and cases | 12, 13, 16, 17, 20, 21 |
| `src/check/stmt.rs` | `input` answers must be text | 14 |
| `spec/v0.md`, `tests/spec_examples.rs`, `examples/`, `tests/examples.rs` | Spec wording, §13 as a case, examples with tests | 21, 22 |
| `README.md`, design spec §6 | As built | 23 |

---

## Part M4a: plain tests

### Task 1: `Fork` and `Wait` in the IR

**Files:** modify `src/ir/mod.rs`, `src/ir/text.rs`, `src/target/x86_64/linux.rs` (a stub), `src/target/x86_64/lower_tests.rs`.

**Interfaces (produces):**
```rust
pub enum Prim { …, Fork, Wait }
```
- `Fork()` (no arguments): in the parent the child's process id (positive); in the child 0; negative on error. The child is a copy of the process: registers, stack, memory and globals as they were; it shares the open files and the output descriptors.
- `Wait(pid)` (one argument): waits for that child. The result is its exit status (0 to 255) when it exited, `256 + signal number` when a signal ended it, or a negative error (`-10`, no such child). This is the IR's vocabulary; each target decodes its own wait status to it.
- IR text: `system fork`, `system wait`.
- The Linux backend panics with `Fork and Wait arrive in task 3` until Task 3 (as M3-R6 did for the file primitives); no program can reach them yet.

**Tests (binding):**
- `ir/text.rs`: the text round trip for `system fork` and `system wait`.
- `lower_tests.rs`: `fork_and_wait_are_not_lowered_yet` (`#[should_panic(expected = "arrive in task 3")]`, replaced by Task 3's real tests).

**Steps:**
- [ ] Write the tests; run `cargo test fork` and `cargo test wait` - they fail (variants missing).
- [ ] Add the variants with their doc comments, the text forms and the parser entries; add the stub arms in `linux.rs::system`.
- [ ] Gate; commit "Add the Fork and Wait primitives to the IR".

**Fail-proof:** a text printer that prints `fork` as `wait` still round-trips if both directions share one table, so the test also asserts the printed bytes of each (`"  system fork\n"` and `"  system wait\n"` in a one-instruction function).
**Review:** risky (new primitives at the process boundary) - first review `opus`.
**Risks:** the result vocabulary (`256 + signal`) is fixed here and used by the driver's `ended without a result (status N)`; changing it later changes a message.
**Estimate:** ~50 lines (×1.4 = 70), 4 files.

### Task 2: Fork and Wait in `irsim`

**Files:** modify `src/irsim/machine.rs`, `src/irsim/memory.rs`, `src/irsim/system.rs`, `src/irsim/tests.rs`.

**Interfaces:**
- `#[derive(Clone)]` on `Machine`, `Memory`, `Frame`, and every type a `Machine` holds that lacks it (the files, the descriptor table's `OpenFile` and `Target`).
- `Machine` gains `children: BTreeMap<i64, i32>` (pid to exit status) and `next_pid: i64` (starts at 100).
- `Prim::Fork`: clone the machine, set the clone's `a` to 0, run the clone with `step()` until it ends (`Ended::Exited(s)`: record `children.insert(pid, s)`; `Ended::Returned`: panic `irsim: a forked child returned from the program's entry`), then copy the clone's `stdout`, `stderr`, `writes`, `files` and `stdin_pos` back into the parent (they are what the real children share with it) and leave the new pid in the parent's `a`. Memory, registers, frames and open descriptors stay the parent's own.
- `Prim::Wait`: pop the pid; `a` = the recorded status (and forget it), or `-10` when there is no such child.

**Tests (binding), `src/irsim/tests.rs`:**
- `a_child_gets_zero_and_the_parent_the_pid`: a module whose `_start` forks; the child writes `"c"` and exits 7; the parent writes `"p"`, waits, exits with the status: stdout `cp`, status 7.
- `a_child_cannot_change_the_parents_memory`: a global written by the child before it exits is unchanged for the parent afterwards.
- `a_childs_files_and_output_reach_the_parent`: the child creates a file and writes standard error; both are in the `Outcome`.
- `waiting_for_an_unknown_child_is_an_error`: `Wait(5)` gives `-10`.
- `two_children_wait_in_any_order`: fork twice, wait for the second pid first; statuses match their children.

**Fail-proof:** a Fork that runs the child *on the parent's machine* (no clone) passes the output test and fails `a_child_cannot_change_the_parents_memory`. A Fork that forgets to copy output back passes the memory test and fails `a_childs_files_and_output_reach_the_parent`.
**Review:** risky (the model the driver's isolation rests on) - first review `opus`.
**Risks:** cloning `Memory` per fork costs time proportional to the touched memory (small in cases; the whole-heap copy is the price of real fork semantics). If `tests` get slow, `labels` can move behind an `Rc`; do not share `memory`.
**Estimate:** ~110 lines (×1.4 = 154), 4 files. It is under the pre-split line only because the cloning derives are one-line edits; if the derive spread exceeds 3 files, split the derives into 2a.

### Task 3: Fork and Wait on Linux

**Files:** modify `src/target/x86_64/linux.rs`, `src/target/x86_64/lower_tests.rs`, `src/target/x86_64/difftest/sys.rs` (cases).

`Fork`: `fork` is syscall 57 (`const FORK: i64 = 57`), no arguments, through the existing `call()` helper (which keeps `rcx`, the IR's `b`). `Wait`: pop the pid into `rdi`; reserve 8 bytes of stack for the status; `wait4(pid, &status, 0, 0)` is syscall 61 (`const WAIT4: i64 = 61`; `rsi` = the stack address, `rdx` = `r10` = 0); a negative result stays as it is; otherwise decode the status word `w`: if `w & 0x7f == 0` the result is `(w >> 8) & 0xff`, else `256 + (w & 0x7f)`; release the 8 bytes. `rcx` is kept as for every primitive. If the encoder lacks a form the decode needs, add it with its golden bytes in the same task (and record a ruling).

**Tests (binding):**
- `lower_tests.rs`: instruction text of `Fork` and of `Wait` (exact `X` sequences, written in the test).
- `difftest` (run on the Linux machine, skipped with the usual message elsewhere): `fork_gives_zero_in_the_child` (child exits 3, parent waits, result 3); `wait_decodes_a_signal` (child stores through a null pointer, parent waits: `256 + 11`); `wait_of_a_stranger_is_negative`; `b_survives_fork_and_wait`.

**Fail-proof:** a decode that returns the raw status passes only when the child exits 0; `fork_gives_zero_in_the_child` exits 3 (raw status 768). A decode that ignores the signal bits passes the exit tests and fails `wait_decodes_a_signal`.
**Review:** risky (system-call numbers, status decoding, stack discipline in an encoder-level helper) - first review `opus`.
**Risks:** a fork in a child that has a large stack is fine; a forgotten stack release in `Wait` shifts the caller's frame (caught by `b_survives_fork_and_wait`, which also checks `rsp`-relative slots).
**Estimate:** ~100 lines (×1.4 = 140), 3 files. Pre-split: 3a the primitive and the `lower_tests`, 3b the difftests, if the first draft passes 120.

### Task 4: The test-state global

**Files:** modify `src/ir/mod.rs` (`Global::TestState`, `TEST_STATE_BYTES`, its name), `src/ir/text.rs` (if names are listed there), `src/target/x86_64/program.rs`, `src/irsim/memory.rs`.

**Interfaces (produces):**
```rust
pub enum Global { …, TestState }     // name "test_state"
pub const TEST_STATE_BYTES: usize = 2048;
```
Both `GLOBALS` arrays gain `Global::TestState` after `NumBuf`, and both `global_size` matches give it `TEST_STATE_BYTES`. The layout of the 2048 bytes (word offsets and the failure-text area) lives in `src/runtime/test_state.rs` (Task 7a), not here.

**Tests (binding):** the existing test that asserts the two arrays list the same globals in the same order (M3 Task 7a) now sees 12; add `test_state_is_2048_bytes_in_both_layouts` (the byte size of the global from each side); an IR-text round trip of `addressof test_state`.

**Fail-proof:** a global added to one array only is caught by the agreement test; a size mismatch is caught by the new test.
**Review:** sonnet.
**Estimate:** ~40 lines (×1.4 = 56), 4 files.

### Task 5: The test backend switch, and `err_int` through `write_all`

**5a - `Backend` (`m4/t05a-backend`).** Files: `src/runtime/mod.rs`, `src/lower/mod.rs` (callers), `src/runtime/tests.rs`.

```rust
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Backend { Real, Test }
pub fn routines_for(backend: Backend) -> Vec<Function>;
pub fn add_reachable(m: &mut Module, called: &[String], backend: Backend);
```
`routines_for(Real)` is today's `routines()`. `routines_for(Test)` is that list with every routine whose name is in `test_backend::REPLACED` removed and `test_backend::routines()` appended; both start empty in this task (`REPLACED: [&str; 0]`, no routines). Every existing caller passes `Backend::Real`.

Tests (binding): `the_real_backend_is_unchanged` (`routines_for(Real)` names equal the old `routines()` names); `a_replaced_routine_is_swapped_by_name` (a test-only override of `REPLACED` through a function parameter `routines_swapping(backend, replaced, extra)` that `routines_for` calls: swapping `flush` for a one-instruction stub gives exactly one `flush` whose body is the stub, and `Real` keeps the original).

**5b - `err_int` through `write_all` (`m4/t05b-err-int`).** Files: `src/runtime/output.rs`, `src/runtime/tests.rs`.

`emit_byte` stops using the `Write` primitive and calls `write_all(2, address of the byte on the stack, 1)`; `err_int`'s digits are unchanged. Real behaviour is identical.

Tests (binding): the existing `err_int` tests still pass byte for byte; new `err_int_writes_through_write_all`: an IR program that replaces `write_all` by a routine counting its calls (a test-only module hack, as `run` helpers in `src/runtime/tests.rs` allow) and writes `-12`: the counting routine is called 3 times, once per byte, with fd 2 each time.

**Fail-proof:** an `err_int` that still issues `Write` itself passes every old test and fails `err_int_writes_through_write_all`.
**Review:** sonnet (5a), sonnet (5b).
**Risks:** 5b changes real failure output code; the run cases for failures 1 to 10 and 6 to 8 must stay green, and are the check.
**Estimate:** 5a ~70 lines (×1.4 = 98); 5b ~45 lines (×1.4 = 63).

### Task 6: Report routines

New runtime texts (appended to `TEXTS` in `src/runtime/mod.rs`, indexes after `MISPLACED`): `PASS` = `pass  `, `FAIL` = `fail  `, `INDENT` = eight spaces, `SHOWN_HEAD` = `      shown:\n`, `LOST` = `ended without a result (status `, `PASSED` = ` passed, `, `FAILED` = ` failed\n`, `EXPECT` = `expect `. Report routines write straight to descriptor 1 with `Prim::Write`, through their own loop, because `write_all`, `flush` and the output buffer belong to the program under test (D31). A failed report write ends the process with status 2.

**6a - lines and the title (`m4/t06a-report-lines`).** Files: `src/runtime/report.rs` (new), `src/runtime/mod.rs`, `src/runtime/tests.rs`.

Interfaces:
```text
report_bytes(addr, len)            writes every byte to fd 1; short writes loop; failure exits 2
report_text(text)                  report_bytes(text + 8, [text])
report_lines(addr, len, indent)    for each line of the bytes (a line ends at \n; a last fragment without \n counts if it is not empty): `indent` spaces (0 to 8, taken from INDENT), the line, \n
test_begin(title)                  stores the title text's address in the test state (TITLE word)
```
The test state's word layout is defined by `src/runtime/test_state.rs` from Task 7a; Task 6a defines only `TITLE = 0` there (the module is created here with that one constant, and Task 7a fills it in).

Tests (binding, `irsim`, exact bytes): `report_lines` of `"a\nb\n"` with indent 6 gives `"      a\n      b\n"`; of `"a\nb"` gives `"      a\n      b\n"`; of the empty text gives nothing; of `"\n"` gives `"      \n"` (an empty line still gets its indent); indent 8 gives eight spaces; `report_bytes` of 5000 bytes with `short_write: Some(7)` writes all of them.

**6b - the three blocks (`m4/t06b-report-blocks`).** Files: `src/runtime/report.rs`, `src/runtime/tests.rs`.

```text
test_pass()                  writes `pass  <title>\n`, exits 0
test_lost(title, status)     writes `fail  <title>\n      ended without a result (status N)\n`; N via number_text(status * 10000, 0)
test_summary(passed, failed) writes `\n`, N, ` passed, `, M, ` failed\n`   (N, M via number_text(n * 10000, 0))
```
Tests (binding): `test_pass_prints_the_title_and_exits_0`; `test_lost_names_the_status` (status 7 gives `      ended without a result (status 7)`; status 267 gives `(status 267)`; status -10 gives `(status -10)`); `test_summary_counts` (`"\n2 passed, 1 failed\n"`, and `"\n0 passed, 0 failed\n"`).

**Fail-proof:** a `report_lines` that adds the indent only once per call passes the single-line tests and fails the two-line one; a `test_summary` that prints the counts swapped fails the 2-and-1 test.
**Review:** sonnet.
**Estimate:** 6a ~110 lines (×1.4 = 154: pre-split already); 6b ~90 lines (×1.4 = 126).

### Task 7: The test backend: state, failure text and the swaps

**7a - state, `fail_add`, `shown_add` (`m4/t07a-test-state`).** Files: `src/runtime/test_state.rs`, `src/runtime/mod.rs`, `src/runtime/tests.rs`.

`test_state.rs` fixes the layout of `Global::TestState` (words of 8 bytes, offsets in bytes): `TITLE` 0, `SHOWN` 8 (the address of a text-layout block whose bytes start at +8), `SHOWN_LEN` 16, `SHOWN_ROOM` 24, `ANSWERS` 32 (a list of text, or 0), `NEXT_ANSWER` 40 (the index of the next answer), `FAILING` 48, `FAIL_LEN` 56, `FAIL_BYTES` 64 (an area of `FAIL_CAP = 1792` bytes). Interfaces:
```text
fail_add(addr, len)     appends to the failure text; bytes beyond FAIL_CAP are dropped; never allocates
shown_add(addr, len)    appends to the shown bytes; the first call allocates 8 + 256 bytes (room 256); a full buffer grows with grow_text (room doubles);
                        when FAILING is 1 and there is no room, the bytes are dropped instead of growing
```
Tests (`irsim`): `fail_add_keeps_the_first_1792_bytes`; `fail_add_never_allocates` (`HeapNext` unchanged); `shown_add_collects_and_grows` (300 bytes, then 5000, read back exactly); `shown_add_drops_when_failing_and_full` (FAILING set, full buffer: length unchanged and `HeapNext` unchanged).

**7b - the swaps and `test_fail` (`m4/t07b-test-backend`).** Files: `src/runtime/test_backend.rs`, `src/runtime/mod.rs` (`REPLACED`), `src/runtime/tests.rs`.

`REPLACED = ["write_all", "fail_begin", "fail_end"]`. Routines:
```text
write_all(fd, addr, len)    fd 1: shown_add; fd 2: fail_add; anything else: ignored; never fails, never exits
fail_begin(line, code)      FAILING = 1; flush; then writes `line <line>: failure <code>: ` through err_text and err_int (so through write_all)
fail_end()                  test_fail()      (no newline is added)
test_fail()                 FAILING = 1; flush; writes `fail  <title>\n`; report_lines(failure text, 6);
                            if the shown length is above 0: SHOWN_HEAD, report_lines(shown bytes, 8); exits 1
test_expect(line, source)   fail_add of `line <line>: expect <source>` (err_int for the line, texts LINE, COLON, EXPECT, err_text for the source text); then test_fail()
```
`test_expect` is a routine of the test backend, not a replacement: it is listed in `test_backend::routines()` only.

Tests (binding, `irsim`, with the Test backend and a hand-made module): `a_failure_goes_to_the_failure_text` (a program that calls `fail_begin(5, 1)`, `err_text("dividing by zero (n is 0)")`, `fail_end()` under `TestState`'s title `t`: stdout is exactly `"fail  t\n      line 5: failure 1: dividing by zero (n is 0)\n"`, stderr empty, status 1); `shown_output_comes_after_the_message` (a show of `"hi\n"` first: the block ends `"      shown:\n        hi\n"`); `no_shown_part_when_nothing_was_shown`; `test_expect_formats_the_line` (`"      line 23: expect name of largest_expense is \"Dinner\"\n"` for the source text given); `a_failure_inside_a_show_flushes_into_shown` (partial output `a` without a newline appears as one line `        a`); `write_all_to_other_descriptors_is_ignored`.

**Fail-proof:** a `fail_end` that kept the real newline-and-exit-2 passes the title test only if status and stderr are not asserted; the tests assert status 1 and empty stderr. A `fail_begin` that does not flush loses the pending `show` output (`a_failure_inside_a_show_flushes_into_shown`).
**Review:** 7a sonnet; 7b risky (it replaces runtime routines the whole program calls) - first review `opus`.
**Risks:** a test routine that allocates while reporting out of memory would recurse (F4): `fail_add` and the `FAILING` rule exist to prevent it, and the second 7b test pins it.
**Estimate:** 7a ~110 lines (×1.4 = 154: pre-split already); 7b ~130 lines (×1.4 = 182, at the edge: if it passes 130 in draft, move `test_expect` and its test to Task 9).

### Task 8: Lowering tests

**8a - test bodies as functions (`m4/t08a-lower-tests`).** Files: `src/lower/mod.rs`, `src/lower/test.rs` (new), `src/lower/stmt.rs`, `src/lower/tests.rs`.

```rust
pub enum Mode { Real, Test }
pub fn lower(program: &Program, types: &TypeInfo, src: &Source) -> Result<Module, Diagnostic>;      // Mode::Real, as today
pub fn lower_tests(program: &Program, types: &TypeInfo, src: &Source) -> Result<Module, Diagnostic>; // Mode::Test
```
`Lowerer` gains `pub mode: Mode`. `assign_slots` and `variable_types` take `(vars: &[(String, Ty, u32)], loops: &[LoopVar])` instead of a feature name (a feature passes its `FeatureInfo`'s; a test passes its `TestInfo`'s). `lower_tests` lowers every feature (as `lower`) and one function `t_<i>` per test (`i` from 0, source order, params 0, no `give`), whose body is `l.stmts(&test.body)` and then `Return`; it passes `Backend::Test` to `add_reachable`. In `Mode::Test`, statements and expressions this milestone has not built are refused with the existing `this compiler cannot build this statement yet` (or `…expression…`): `ask`, `load`, `save`, `input`, `shown`, and `expect` (until Task 9). This is deliberate (F12): no test build may reach a real file primitive or read standard input.

Tests (binding), `src/lower/tests.rs`:
- `a_test_body_lowers_to_t_0` (a program with one test that appends to a table: the module has `t_0`, params 0, and calls `list_push`).
- `tests_are_numbered_in_source_order` (three tests: `t_0`, `t_1`, `t_2` bodies differ as the sources do).
- `test_mode_refuses_what_is_not_built_yet` (one case per refused construct: `ask`, `load`, `save`, `input`, `shown` inside a test each give `this compiler cannot build this statement yet` or `…expression yet` with the right line; a feature with `load` that a test never calls is refused too, since features are lowered whole).
- `real_mode_still_lowers_load_and_save` (the same `load` and `save` lower under `lower`).
- `no_real_file_primitive_in_a_test_module` (a program with tables and tests, no `load`, `save`, `ask`: no instruction of the module is `Inst::System(Prim::Open(_) | Rename | Sync | Mode | SetMode)` and none is `Read`; the helper `has_file_primitive(&Module) -> bool` is written here and reused by Task 19b).

**8b - the driver and the library entry (`m4/t08b-driver`).** Files: `src/lower/test.rs`, `src/runtime/core.rs` (nothing; the test `_start` is lowered), `src/lib.rs`, `src/lower/tests.rs`.

`lower_tests` also emits `test_main` (params 0, slots 3: 1 passed, 2 failed, 3 the status) and a `_start` that replaces the runtime's: `heap_init; stack_init; tables_init; test_main; flush; push a; Exit`. (`_start` is added to the module by lowering, before `add_reachable`, so the runtime's `_start` is never copied; lowering adds `heap_init`, `stack_init`, `tables_init`, `flush`, `test_begin`, `test_pass`, `test_lost` and `test_summary` to the routines `add_reachable` must bring in.) Per test `i`, with the title as a compile-time text (each invisible character written `\u{XXXX}`, as `fail.rs`'s `spell_invisible` does; make it `pub(super)`):
```text
call flush
Fork
if a != 0 goto parent_i
  push title; call test_begin; call t_<i>; call test_pass            (the child; test_pass exits)
parent_i:
  push a; Wait; store slot 3
  if slot 3 == 0: passed += 1
  elif slot 3 == 1: failed += 1
  else: push title; push slot 3; call test_lost; failed += 1
```
After the last test: `push passed; push failed; call test_summary`; the result in `a` is 1 when `failed > 0`, else 0. A module with no tests prints `\n0 passed, 0 failed\n` and exits 0 (the CLI never builds one, Task 11).

`Checked` gains `pub fn test_executable(&self) -> Result<Vec<u8>, Diagnostic>` (like `executable`, through `lower::lower_tests`) and `pub fn test_count(&self) -> usize`; `lib.rs` gains `pub fn build_test_executable(text: &str) -> Result<Vec<u8>, Vec<Diagnostic>>`, the twin of `build_executable`.

Tests (binding): `the_driver_forks_once_per_test` (IR text of `test_main` for two tests: two `system fork` and two `system wait`, calls `t_0` and `t_1` in that order); `the_title_is_escaped` (a title with U+202E prints as `\u{202E}` in the module's text constant); `an_empty_driver_summarises_zero` (run under `irsim`: stdout `"\n0 passed, 0 failed\n"`, status 0); `build_test_executable_gives_bytes_for_a_program_with_tests` (an ELF header, and a `Vec<Diagnostic>` with the usual error for a program with a check error). An `irsim` test, `a_child_that_dies_is_reported_lost`, replaces `t_0`'s body in a lowered module with `push 7; Exit` and expects stdout `"fail  t\n      ended without a result (status 7)\n\n0 passed, 1 failed\n"` and status 1.

**Fail-proof:** a driver that runs `t_i` in the parent (no fork) passes a pass-only program; `the_driver_forks_once_per_test` and `a_child_that_dies_is_reported_lost` (the parent would exit 7) fail it. A driver that omits the escape passes ASCII titles; `the_title_is_escaped` catches it.
**Review:** sonnet (8a); 8b risky (the driver is where isolation and counting live) - first review `opus`.
**Risks:** `tables_init` runs once, before the loop, so every child inherits its fresh tables; if a later change moved it into the child, isolation would silently weaken (Task 13's `tables_do_not_leak_between_tests` pins it).
**Estimate:** 8a ~120 lines (×1.4 = 168, pre-split); 8b ~120 lines (×1.4 = 168, pre-split).

### Task 9: `expect`

**Files:** modify `src/lower/stmt.rs`, `src/lower/tests.rs`.

`StmtKind::Expect(cond)` (only in a test): `self.line = s.line; emit Line; let ok = new_label(); self.condition(cond, fail)`; the failure label is emitted after the test body's `Return` with the other fail blocks: push `line`, push the condition's source as a text (`ast::print::expr(cond)`, invisible characters spelled as in `spell_invisible`), `call test_expect`. Lowering refuses `expect` in `Mode::Real` (it cannot occur, the checker already refuses it outside a test).

Tests (binding): `expect_lowers_a_condition_and_a_failure_block` (IR text: a `jump` to a label that pushes line 4 and the source text `n is 3` and calls `test_expect`); `expect_with_a_formatted_source` (`expect a*b is 6` reports `a * b is 6`, as `gloss format` writes it).

**Fail-proof:** a lowering that evaluates the condition but never jumps passes the IR-shape test only if the shape test asserts the jump; it does. The end-to-end proof is Task 12's `expect_fails`.
**Review:** sonnet.
**Estimate:** ~60 lines (×1.4 = 84), 2 files.

### Task 10: Tables start empty and cleared in tests

**Files:** modify `src/lower/table.rs`, `src/lower/mod.rs`, `src/lower/tests.rs`.

`tables_init(program, called, mode)`: in `Mode::Test` the loaded flag of each table is 1 (and the list is the usual empty one). In `Mode::Real` nothing changes.

Tests (binding): `a_test_module_starts_every_table_cleared` (`tables_init` IR in test mode stores 1 at each flag offset, 0 in real mode: two assertions on the IR text); an `irsim` check that after `tables_init` in a test module the flag word of the second table is 1 and the first table's list count is 0.

**Fail-proof:** flag 0 in test mode would make the first `save` (Task 19) fail 8; the IR-text assertions fail first.
**Review:** sonnet.
**Estimate:** ~45 lines (×1.4 = 63), 3 files.

### Task 11: The `gloss test` command

**11a - running a test executable (`m4/t11a-testrun`).** Files: `src/testrun.rs` (new), `src/lib.rs` (module), `src/testrun/tests.rs` (or an inline `mod tests`).

```rust
/// Whether this host can run the executables the compiler builds.
pub fn host_runs_tests() -> bool { cfg!(all(target_os = "linux", target_arch = "x86_64")) }
/// A fresh private folder (mode 0700) under the system temporary folder, removed on drop.
pub struct PrivateDir { path: PathBuf }
impl PrivateDir { pub fn new() -> std::io::Result<PrivateDir>; pub fn path(&self) -> &Path }
/// Writes `executable` into a private folder (mode 0700), runs it with standard input closed,
/// streams its output as it arrives, and gives the exit status for `gloss test`.
pub fn run_test_executable(executable: &[u8], out: &mut dyn Write, err: &mut dyn Write) -> i32;
/// 0 and 1 are the test run's own; any other end is 1 with a message.
pub fn status_for(ended: std::process::ExitStatus) -> (i32, Option<String>);
```
The message is `the test run ended without finishing (status N)`, where `N` is the exit code, or `256 + signal` for a signal (the same vocabulary as `Wait`). Output is streamed: the child is spawned with piped standard output and error, and each is copied to `out` and `err` as bytes arrive (standard error on its own thread; `out` is flushed after every chunk), so a hung test still shows every finished test's result; the folder is removed on every path by `PrivateDir`'s `Drop`, including a panic.

Tests (binding): `the_private_folder_is_0700_and_removed` (mode bits `0o700` while alive; gone after drop; two folders have different names); `a_panic_still_removes_the_folder` (`catch_unwind` around a scope holding one); `status_for_maps_exit_codes` (`ExitStatus::from_raw(0)` gives `(0, None)`, `from_raw(1 << 8)` gives `(1, None)`, `from_raw(7 << 8)` gives `(1, Some("the test run ended without finishing (status 7)"))`, `from_raw(11)` gives `(1, Some("… (status 267)"))`); on Linux x86-64 only: `a_shell_script_stands_in_for_the_executable` is not possible (the executable is an ELF), so `run_test_executable` is unit tested through a seam `run_program(path, out, err) -> i32` that takes a path, with a `#!/bin/sh` script that prints `x` and exits 1.

**11b - the command (`m4/t11b-test-command`).** Files: `src/cli.rs`, `tests/cli.rs`, `src/lib.rs`.

In `cli::run`, `test` is no longer in the milestone table. `gloss test <file.gloss>` (no `--allow`: a usage error, as for `format`): read the file and apply the `.gloss` suffix rule; check (the same diagnostics as `build`, sorted, printed to standard error); an error gives 3; no test prints `No tests in <name>` to standard output and returns 0; `!testrun::host_runs_tests()` prints `gloss test runs tests on Linux only` to standard error and returns 3; otherwise `checked.test_executable()` (an error prints its diagnostic, 3), then `testrun::run_test_executable`. `.allow` is never read or written, and nothing is written beside the source.

Tests (binding, `tests/cli.rs`): replace the `test` expectation in `usage_and_unbuilt_commands_exit_3` (the other unbuilt commands stay); `test_reports_errors_with_status_3`; `test_with_no_tests_says_so` (`No tests in x.gloss`, status 0, nothing beside the source); `test_refuses_allow` (usage, 3); `test_never_touches_allow_or_beside_the_source` (a program with a table, a `.allow` that would change, no tests: status 0, directory listing and `.allow` bytes unchanged); on a non-Linux-x86-64 host only (`#[cfg(not(all(target_os = "linux", target_arch = "x86_64")))]`) `test_refuses_off_linux` (stderr exactly `gloss test runs tests on Linux only\n`, status 3); on Linux x86-64 only: `a_passing_program_exits_0` (stdout is `pass  t\n\n1 passed, 0 failed\n`) and `a_failing_program_exits_1`.

**Fail-proof:** a command that builds beside the source passes `a_passing_program_exits_0` but fails `test_never_touches_allow_or_beside_the_source`'s directory listing; a command that checks the platform before the diagnostics passes the Linux-only test but fails `test_reports_errors_with_status_3` on the Mac.
**Review:** sonnet for both (the temporary folder is the only new boundary and it is built and tested in 11a).
**Risks:** a hung test hangs the run (accepted with D40); streaming (ruling P8) means the finished tests' results are already on screen. Test: `output_is_streamed_before_the_run_ends` (the `run_program` seam with a script that prints `x`, then waits on a FIFO the test opens only after it has read `x` from `out`).
**Estimate:** 11a ~110 lines (×1.4 = 154: pre-split already); 11b ~120 lines (×1.4 = 168: pre-split already).

### Task 12: The case harness

**12a - real executables (`m4/t12a-case-harness`).** Files: `tests/test_sweep.inc` (new), `tests/test_cases.rs` (new), `tests/test_cases/all_pass.gloss`, `.out`, `tests/test_cases/expect_fails.gloss`, `.out`, `.status`.

`tests/test_sweep.inc` (shared with `src/irsim/test_cases.rs` through `include!`): `const MIN_TEST_CASES: usize = 2;` and `fn test_sweep(dir: &Path) -> Result<Vec<PathBuf>, String>`: the `.gloss` programs, sorted, an error naming every file that is not part of a case (extensions other than `gloss`, `out`, `err`, `status`, or a missing `<stem>.gloss`, or a folder where a file belongs). `tests/test_cases.rs`: for each case, `gloss::build_test_executable(text)` (an `Err` is a failure of the case), write the bytes to a `TempDir` file with mode 0755, `run_support::run_within(path, b"", DEADLINE)`; skip with `run_support::skip()` when there is no runner; compare standard output with `.out`, standard error with `.err` (default empty) and the status with `.status` (default 0). Tests: `the_sweep_finds_the_test_cases` (count at least `MIN_TEST_CASES`), `the_test_sweep_reports_an_orphan` (as `tests/run.rs` does), `a_case_without_an_out_file_is_reported` (a `.gloss` with no `.out` makes `test_sweep` return an error naming it), `every_test_case_gives_the_expected_output`.

Cases:
- `all_pass`: two tests, both pass. `.out`: `pass  one plus one is two\npass  a table starts empty\n\n2 passed, 0 failed\n`.
- `expect_fails`: three tests; the second has an `expect` that fails on source line 8 (`expect 1 + 1 is 3`) between two that pass. `.out`: `pass  first\nfail  second\n      line 8: expect 1 + 1 is 3\npass  third\n\n2 passed, 1 failed\n`, `.status` `1`.

**12b - `irsim` (`m4/t12b-irsim-cases`).** Files: `src/irsim/test_cases.rs` (new), `src/irsim/mod.rs`.

`every_test_case_gives_the_expected_output_in_irsim`: for each case, parse, check (it must be clean), `lower_tests`, `run_with(&module, "_start", b"", &[], Faults::default(), &mut Files::default())`, compare standard output, standard error and status with the same files. A sweep guard: `MIN_TEST_CASES` as above. A test, `the_irsim_harness_is_not_vacuous`, runs a case with a deliberately wrong `.out` text held in the test (not on disk) and expects the comparison function to report it.

**Fail-proof:** a harness that compares only the status passes `expect_fails`' status but not its `.out`; both are compared. A sweep that finds no case fails `MIN_TEST_CASES`. A harness that accepts a missing `.out` is caught by `a_case_without_an_out_file_is_reported`.
**Review:** sonnet.
**Estimate:** 12a ~120 lines (×1.4 = 168, pre-split), 7 files; 12b ~80 lines (×1.4 = 112), 2 files.

### Task 13: Isolation cases

**13a (`m4/t13a-failure-cases`).** Files: `tests/test_cases/failure_in_test.gloss`, `.out`, `.status`; `failure_10_in_test.gloss`, `.out`, `.status`; `show_is_quiet.gloss`, `.out`, `.status`; `tests/test_sweep.inc` (`MIN_TEST_CASES` to 5).

- `failure_in_test`: tests `fine before` (passes), `divides by zero` (a feature divides by a variable that is 0 on line 7), `fine after` (passes). `.out`: `pass  fine before\nfail  divides by zero\n      line 7: failure 1: dividing by zero (people is 0)\npass  fine after\n\n2 passed, 1 failed\n`; status 1. This proves failure 1 under the title, not on standard error, and the next test still runs.
- `failure_10_in_test`: a feature recurses without a base (`countdown`), `calls nested too deep (countdown)`; the tests before and after pass. `.out` has the failure under its title; status 1. (The real run and `irsim` differ in stack size, not in the message, D21.)
- `show_is_quiet`: a passing test that shows two lines prints nothing under its title; a failing test (`expect 1 is 2`) that shows `Largest so far: Bus at 40` prints `      line N: expect 1 is 2\n      shown:\n        Largest so far: Bus at 40\n`.

**13b (`m4/t13b-isolation-cases`).** Files: `tests/test_cases/tables_start_empty.gloss`, `.out`; `tables_do_not_leak_between_tests.gloss`, `.out`; `tests/test_sweep.inc` (`MIN_TEST_CASES` to 7).

- `tables_start_empty`: `expect count of expenses is 0`, then `append` makes it 1, then `clear` and `append` twice makes 2. All pass.
- `tables_do_not_leak_between_tests`: test A appends two rows to `expenses`; test B asserts `count of expenses is 0`; a global-like effect through a `repeat while` counter variable does not carry either. All pass.

**Fail-proof:** a driver that forks once and runs every test in one child fails `tables_do_not_leak_between_tests` (B sees 2) and `failure_in_test` (`fine after` never runs). A report that prints failures on standard error fails `failure_in_test`'s `.out` and its empty `.err`. A `show` that goes straight to standard output fails `show_is_quiet`.
**Review:** sonnet.
**Estimate:** 13a ~70 lines (×1.4 = 98), 10 files (at the limit: three cases of three files plus the sweep constant; if a case needs a fourth file, split it out); 13b ~45 lines, 5 files.

---

## Part M4b: fixtures

### Task 14: `input` answers must be text

**Files:** modify `src/check/stmt.rs`.

`StmtKind::Input(items)`: each item goes through `self.require(stmt.line, i, &Ty::Text, "An input answer must be text")`, producing, for `input 5` on line 3, `line 3: input 5. An input answer must be text; 5 is a number.` (the form `require` writes for every other typed position).

Tests (binding): `input_answers_must_be_text` (a number, a truth, a list: each gives its message with the right description); `text_answers_check_clean`; `spec_9_input_example_still_checks` (the spec's own `input "Lunch", "120"` and `input "Dinner", "300", "food"`).

**Fail-proof:** a check that accepts any type passes the clean test; the three error assertions fail it.
**Review:** sonnet.
**Estimate:** ~30 lines (×1.4 = 42), 1 file.

### Task 15: `input` and `ask`

**15a - the question message (`m4/t15a-ask-message`).** Files: `src/runtime/test_backend.rs`, `src/runtime/mod.rs` (texts), `src/runtime/tests.rs`.

New texts: `GAVE` = `the test gave `, `ANSWER` = ` answer`, `BUT_LINE` = ` but line `, `ASKED_A` = ` asked a `, `QUESTION` = ` question`, `ASKED_NUMBER` = ` asked question number `, `PLURAL_S` = `s`, and ten ordinal texts `first` to `tenth`.
```text
test_ask_fail(line, given, asked)    asked = the number of the question (1-based); given = the answers queued so far;
                                      fail_add of the message, then test_fail()
```
The message is `the test gave <given> answer[s] but line <line> asked a <ordinal> question` (`answer` exactly when `given` is 1; the ordinal is the word for `asked` when it is 1 to 10, else the text ` asked question number <asked>` replaces ` asked a <ordinal> question`). The numbers are written with `err_int` (through `write_all`, so into the failure text).

Tests (binding, `irsim`, exact output): `(0, line 7, asked 1)` gives `      the test gave 0 answers but line 7 asked a first question`; `(1, 7, 2)` gives `… 1 answer but line 7 asked a second question`; `(2, 31, 3)` gives `the test gave 2 answers but line 31 asked a third question` (the spec's example); `(10, 4, 11)` gives `the test gave 10 answers but line 4 asked question number 11`; and all ten ordinals once.

**15b - `input` and `ask` in test mode (`m4/t15b-input-ask`).** Files: `src/runtime/test_backend.rs`, `src/lower/stmt.rs`, `src/lower/test.rs`, `src/runtime/tests.rs`.

`REPLACED` gains `"ask"`. Routines:
```text
test_input(text)           appends the text to the ANSWERS list (created with list_new(4, 0) on first use) 
ask(question, line)        (test version) takes ANSWERS[NEXT_ANSWER] and increments NEXT_ANSWER; prints nothing; with none left calls test_ask_fail(line, count of ANSWERS, NEXT_ANSWER + 1)
```
Lowering in `Mode::Test` lifts the refusals for `ask` (real lowering unchanged: the same `call("ask")`, which the backend swaps) and `input`: each item is evaluated, pushed with the line, `call test_input`. `lower_tests` adds `test_input` to `called`.

Tests (binding): `input_queues_answers_in_order` (a program `input "a", "b"`, then two `ask`s: the answers arrive in order, and nothing is shown); `ask_with_no_answer_fails_the_test_with_the_spec_message` (one answer, two asks: stdout `fail  t\n      the test gave 1 answer but line 4 asked a second question\n`); `a_second_input_statement_adds_to_the_queue`; `ask_never_reads_standard_input` (the module under `irsim` with stdin `"x\n"` and `Faults::failing_read`: no failure, the test still asks its queued answer); lowering: `real_ask_is_unchanged` (the IR of `ask` in real mode is the same as before this task).

**Fail-proof:** a swapped `ask` that falls back to reading standard input when the queue is empty passes the queued tests and fails `ask_with_no_answer…` (the failure would not appear) and `ask_never_reads_standard_input`.
**Review:** 15a sonnet; 15b risky (it replaces a runtime routine that touches input) - first review `opus`.
**Estimate:** 15a ~100 lines (×1.4 = 140: pre-split already); 15b ~120 lines (×1.4 = 168).

### Task 16: `ask` and `input` cases

**Files:** `tests/test_cases/ask_answers.gloss`, `.out`; `ask_runs_out.gloss`, `.out`, `.status`; `ask_no_input.gloss`, `.out`, `.status`; `tests/test_sweep.inc` (`MIN_TEST_CASES` to 10).

- `ask_answers`: test one `input "Lunch", "120"`, `ask "What?" into a`, `ask "How much?" into b`, `expect a is "Lunch"`, `expect b is "120"`; passes, nothing printed (prompts are not shown). A second test shows `shown` is separate. `.out`: both `pass`, `2 passed, 0 failed`.
- `ask_runs_out`: `input "only"` and two `ask`s on lines 4 and 5; another test passes. `.out`: `fail  asks twice\n      the test gave 1 answer but line 5 asked a second question\npass  other\n\n1 passed, 1 failed\n`, status 1.
- `ask_no_input`: no `input`, one `ask` on line 3: `the test gave 0 answers but line 3 asked a first question`, status 1.

**Fail-proof:** a missing-answer failure reported as failure 9 (`input ended while ask was waiting`) fails both `.out` files. A queue shared between tests fails `ask_answers`' second test if it asks again.
**Review:** sonnet.
**Estimate:** ~50 lines (×1.4 = 70), 9 files.

### Task 17: `shown`

**Files:** modify `src/runtime/test_backend.rs`, `src/lower/expr.rs`, `src/lower/test.rs`, `src/runtime/tests.rs`, `tests/test_cases/shown_list.gloss`, `.out`, `tests/test_sweep.inc` (`MIN_TEST_CASES` to 11).

```text
shown_list(line)    flush; a fresh list of text: one entry per line of the shown bytes (a line ends at \n; the \n is not part of it; a final fragment without \n is not a line)
```
`ExprKind::Shown` lowers to `push line; call shown_list`; the refusal is lifted. The list is a copy: a later `show` does not change it.

Tests (binding): `shown_splits_at_newlines` (`irsim`: shown bytes `a\nbb\n` give a 2-item list `a`, `bb`); `a_fragment_is_not_a_line`; `shown_is_a_copy` (take `shown`, show another line, the earlier list has the old count); case `shown_list`: a test shows `Total`, `Largest`, then `expect count of shown is 2`, `expect first of shown is "Total"`, `expect last of shown is "Largest"`, and asks nothing (prompts excluded: a second test asks a question with an `input`, runs, and `expect count of shown is 0`). Passes.

**Fail-proof:** a `shown` that includes prompts fails the second case; one that reads the output buffer without flushing returns an empty list when the lines are still buffered (the first test shows two lines and reads them straight away).
**Review:** sonnet.
**Estimate:** ~110 lines (×1.4 = 154: pre-split already; if the draft passes 110, move the case files to a separate PR 17b), 7 files.

### Task 18: Split `table_read`

**Files:** modify `src/runtime/table_read.rs`, `src/runtime/tests.rs`.

```text
table_read(path, shown, desc, line)         unchanged signature and behaviour: list_new(4); file_bytes; if 0 return the empty list; table_rows(text, shown, desc, line, list)
table_rows(text, shown, desc, line, list)   the parser half: csv_cursor onwards, pushing records into `list`; returns the list
```
(A pure refactor of the real runtime: `table_read`'s IR keeps its name and behaviour, and `table_rows` is called from it and, in Task 19, from the pretend `load`.)

Tests (binding): every existing `load` run case and the CSV unit tests pass unchanged (the gate); new `table_rows_parses_what_table_read_parses` (an `irsim` program calls `table_rows` directly with a text `"name,amount\nBus,40\n"` and gets one record); `table_rows_reports_failures_like_table_read` (a bad cell: failure 6 with the same message).

**Fail-proof:** a split that changes the order of `list_new` and `file_bytes` changes allocation order and is caught by the existing run cases' heap-dependent outputs only indirectly, so a unit test asserts the IR text of `table_read` still starts `const a, 4` ... `call list_new`.
**Review:** sonnet.
**Risks:** real runtime code (untrusted CSV parser) is touched; all M3 failure 6 cases are the net.
**Estimate:** ~60 lines (×1.4 = 84), 2 files.

### Task 19: Pretend files

**19a - the routines (`m4/t19a-pretend-routines`).** Files: `src/runtime/test_backend.rs`, `src/runtime/tests.rs`.

```text
pretend_save(index, list, desc, line)     text = csv_text(list, desc, line); store its address in the table's PRETEND word
pretend_read(index, shown, desc, line)    list = list_new(4, line); text = the PRETEND word (0 = missing: return the empty list, no notice); return table_rows(text, shown, desc, line, list)
```
The tables block is read through `Global::Tables`: an entry is 24 bytes in a test build, `[list, loaded flag, pretend file]`, and `index` selects it. A missing file (0) gives an empty list and writes nothing anywhere.

Tests (binding, `irsim`, hand-made module with a record `Expense` of a text, a number and a truth): `a_saved_table_reads_back` (save two rows, read: equal field by field); `a_missing_pretend_file_loads_empty` (no notice: stdout, stderr and the failure text all empty); `save_twice_keeps_the_last` (second save replaces the first); `a_second_table_has_its_own_file` (index 1 does not see index 0's file); `pretend_save_uses_the_real_writer` (the stored text equals `csv_text`'s result for the same list, byte for byte, including a quoted cell); `pretend_load_uses_the_real_parser` (a pretend file seeded by the test with a bad cell fails 6 with the M3 message `line N: failure 6: e.csv line 2: amount "x" is not a number`, as an `irsim` unit test that writes the word directly: the one way a failure 6 is reachable, ruling P7).

**19b - lowering (`m4/t19b-pretend-lowering`).** Files: `src/lower/table.rs`, `src/lower/stmt.rs`, `src/lower/mod.rs`, `src/lower/tests.rs`.

In `Mode::Test`: `ENTRY` is 24 (`entry_bytes(mode)`), `tables_init` allocates `24 * n` and still stores flag 1 (Task 10); `read_table` pushes `index`, the display name, the descriptor and the line and calls `pretend_read`; `save_table` skips the real path (there is no failure 8 check: flag 1 always) and calls `pretend_save(index, list, desc, line)`; the refusals of `load` and `save` are lifted. `clear` is unchanged (it replaces the list and keeps the pretend file: `clear` empties the table, not its file). In `Mode::Real` nothing changes.

Tests (binding): `a_test_load_and_save_use_the_pretend_routines` (IR text: calls `pretend_read` and `pretend_save`, never `table_read` or `table_save`); `a_test_module_has_no_real_file_primitive` (reuses `has_file_primitive`, on a program that loads and saves: false; the same program under `lower` gives true: the assertion bites); `real_mode_load_and_save_are_unchanged`; `entry_bytes_are_24_in_test_mode_and_16_in_real`; the tables-block layout test from Task 10 is updated to three words.

**Fail-proof:** a lowering that left `table_read` in place would pass the pretend unit tests and fail `a_test_module_has_no_real_file_primitive` (the `Open` of `file_bytes` is reachable). A `clear` that also reset the pretend word is caught by Task 20's `clear_keeps_the_pretend_file` case.
**Review:** 19a risky (the swap the no-file claim rests on) - first review `opus`; 19b risky - first review `opus`.
**Risks:** `file_bytes` and `table_save` stay in `runtime::routines()`; the safety is that nothing in a test module calls them. The assertion in 19b is that safety's only guard, so it must scan *reachable* functions, which `add_reachable` already restricts the module to.
**Estimate:** 19a ~130 lines (×1.4 = 182, at the edge: move the two last unit tests to 19b if the draft passes 130); 19b ~90 lines (×1.4 = 126).

### Task 20: Pretend-file cases

**20a (`m4/t20a-pretend-cases`).** Files: `pretend_round_trip.gloss/.out`, `pretend_missing_is_empty.gloss/.out`, `pretend_files_are_per_test.gloss/.out`, `tests/test_sweep.inc` (`MIN_TEST_CASES` to 14).

- `pretend_round_trip`: append two rows, `save expenses`, `clear expenses`, `load expenses`, `expect count of expenses is 2`, `expect name of first of expenses is "Lunch"`. Passes.
- `pretend_missing_is_empty`: `load expenses` with no earlier save: count is 0; `.err` is empty (no `not found` notice) - the harness already asserts empty standard error.
- `pretend_files_are_per_test`: test A saves a row; test B loads and `expect count of expenses is 0`. Passes.

**20b (`m4/t20b-pretend-cases-2`).** Files: `clear_keeps_the_pretend_file.gloss/.out`, `csv_quirks_round_trip.gloss/.out`, `tests/test_sweep.inc` (`MIN_TEST_CASES` to 16).

- `clear_keeps_the_pretend_file`: save, `clear`, `expect count is 0`, `load`, `expect count is 1`.
- `csv_quirks_round_trip`: a row whose text holds a comma, a quote and a line break, a negative number and a `no` truth, saved and loaded: every field equal (the real writer and reader).

**Fail-proof:** a pretend file shared across tests fails `pretend_files_are_per_test`; a `load` that prints the real notice fails `pretend_missing_is_empty` through the empty-`.err` assertion; a backend that wrote its own CSV would pass round trips but not `csv_quirks_round_trip` against the real reader, and Task 19a's `pretend_save_uses_the_real_writer` compares bytes.
**Review:** sonnet.
**Estimate:** 20a ~60 lines (×1.4 = 84), 7 files; 20b ~50 lines, 5 files.

---

## Part: spec, examples, as built

### Task 21: The language spec's example

**21a (`m4/t21a-spec-13`).** Files: `tests/test_cases/expense_tracker.gloss`, `.out`; `tests/test_sweep.inc` (`MIN_TEST_CASES` to 17).

`expense_tracker.gloss` is the §13 program of `spec/v0.md`, character for character as in the spec (the spec guard in 21b keeps them equal). `.out`: `pass  category total counts only the matching category\npass  largest expense is found\npass  a cost that is not a number is asked for again\npass  a new expense is added to the saved ones and reported\n\n4 passed, 0 failed\n`.

**21b (`m4/t21b-spec-text`).** Files: `spec/v0.md`, `tests/spec_examples.rs`.

- §13: delete "Nothing has compiled or run this program yet; …" and say that `tests/test_cases/expense_tracker.gloss` runs it.
- §11: `gloss test`'s row says it prints each title and a summary; the statuses table keeps 0, 1, 3; add one sentence marked as a deviation: on a host that cannot run x86-64 Linux programs `gloss test` refuses with `gloss test runs tests on Linux only` and status 3 (D38); `No tests in x.gloss` for a program with none.
- §9 "Running": the report format as D33 (the preview, in a `text` block, not a `gloss` block) and the `ended without a result` line; "Output is quiet" matches it.
- `tests/spec_examples.rs`: `the_spec_13_program_is_the_expense_tracker_case` (the §13 block equals the case file's text); `every_complete_spec_program_with_tests_passes` (each complete spec program that has tests builds with `build_test_executable`; those that need no input run through the runner and pass); the §9 snippets (`input "Dinner", "300", "food"`, `expect first of shown is …`) still parse, check and format.

**Fail-proof:** a spec edit that drifts from the case fails `the_spec_13_program_is_the_expense_tracker_case`; a spec program whose tests fail fails the second test (a vacuous version that builds but never runs is caught by asserting the run output `N passed, 0 failed` for the §13 and §9 programs).
**Review:** sonnet.
**Estimate:** 21a ~50 lines (×1.4 = 70), 4 files; 21b ~80 lines (×1.4 = 112), 2 files.

### Task 22: Examples gain tests

**22a (`m4/t22a-example-tests`).** Files: `examples/gcd.gloss`, `examples/primes.gloss`, `examples/fibonacci.gloss`, `examples/gcd.tests.out`, `examples/primes.tests.out`, `examples/fibonacci.tests.out`.

Each program gains two or three tests after its features (`gcd`: `gcd with 12, 18` is 6 and `gcd with 7, 13` is 1; `primes`: 7 is prime, 9 is not; `fibonacci`: the loop and the recursion agree for 10). Each `.tests.out` is the exact output of `gloss test` on it. The programs' own `.out` files do not change (test blocks are not part of the built program).

**22b (`m4/t22b-example-tests-2`).** Files: `examples/expenses.gloss`, `examples/expenses.tests.out`, `examples/README.md`, `tests/examples.rs`.

`expenses` gains a test that appends, saves and loads through pretend files (`expect count of expenses is 2` after a save and a load); `tests/examples.rs` gains `every_example_with_tests_passes_them`: for each example that has a `<name>.tests.out`, build with `gloss::build_test_executable`, run through the runner and compare standard output, with status 0; `MIN_EXAMPLES_WITH_TESTS = 4`; a sweep guard that every `.tests.out` has its `.gloss`. The README gains a line per example saying it has tests, and how to run `gloss test`.

**Fail-proof:** an example whose test is wrong fails its `.tests.out` comparison; a harness that finds no `.tests.out` fails the minimum.
**Review:** sonnet.
**Estimate:** 22a ~60 lines (×1.4 = 84), 6 files; 22b ~70 lines (×1.4 = 98), 4 files.

### Task 23: As built

**Files:** `README.md` (M4 done; `gloss test`; the report format; fixtures; the Linux-only rule), design spec §6 "As built — Milestone 4" (what shipped, deviations by decision id, PRs, rulings M4-R*, tests, known limits), the status line and the plan list at the top of the design spec.

**Review:** sonnet. **Estimate:** ~150 lines (×1.4 = 210: split the README into 23a and the design-spec section into 23b).

---

## Self-review (done while writing)

- **Spec coverage.** §9 title and checks (8, 9, 12); tables start empty and cleared (10, 13b); files are pretend, `load` and `save` as in a real run (17-20); `ask` from `input` and its message (14, 15, 16); `shown` (7, 17); output is quiet (7b, 13a); how fixtures are built, a second backend over shared CSV code (5a, 7b, 18, 19); running (`run start` works in a test: 8a lowers `run`; used by 16, 21). §10 failures under a title (13a); output format (6, 8b). §11 `gloss test`, statuses 0, 1, 3 (11), `No tests` (11b), `test` never asks (11b). §13's program (21). Decisions D30 (the two parts), D31 (8), D32 (1-3), D33 (6, 7b), D34 (7b, 9, 13), D35 (15), D36 (8a, 19b), D37 (11), D38 (11b), D39 (12, 13, 16-22), D40 (accepted).
- **Placeholder scan.** No "TBD" or "similar to Task N". Two tasks name a threshold at which they move content between PRs (7b, 19a); both say exactly which content.
- **Type consistency.** `Backend { Real, Test }` and `add_reachable(m, called, backend)` (5a, used in 8a); `Mode { Real, Test }` (8a, used in 10, 19b); `routines_swapping` (5a only); `test_begin`, `test_pass`, `test_lost`, `test_summary`, `report_*` (6, used by 8b, 7b); `fail_add`, `shown_add` and the state offsets (7a, used by 7b, 15, 17, 19a); `test_expect(line, source)` (7b, 9); `test_input`, `ask`, `test_ask_fail(line, given, asked)` (15a, 15b); `shown_list(line)` (17); `pretend_read(index, shown, desc, line)` and `pretend_save(index, list, desc, line)` (19a, 19b); `table_rows(text, shown, desc, line, list)` (18, 19a); `has_file_primitive(&Module) -> bool` (8a, 19b); `Checked::test_executable`, `Checked::test_count`, `build_test_executable` (8b, 11b, 12a); `testrun::{host_runs_tests, PrivateDir, run_test_executable, status_for}` (11a, 11b); `MIN_TEST_CASES` rises 2 (12a), 5 (13a), 7 (13b), 10 (16), 11 (17), 14 (20a), 16 (20b), 17 (21a).
- **Case counts.** 12a: 2; 13a: +3 = 5; 13b: +2 = 7; 16: +3 = 10; 17: +1 = 11; 20a: +3 = 14; 20b: +2 = 16; 21a: +1 = 17. These are the binding `MIN_TEST_CASES` values.
- **Not in M4:** `trace` (M5), `explain` (M6), `flowchart` (M7), a time limit for a looping test (D40), running tests on a non-x86-64-Linux host (D38).

## Open questions / rulings

Choices the plan makes where the spec and the approved decisions are silent. Each says why, and what it costs if wrong.

| # | Ruling | Why | Cost if wrong |
|---|---|---|---|
| P1 | The test backend replaces routines **by name** (`write_all`, `fail_begin`, `fail_end`, `ask`) in `add_reachable`, and `load`/`save` are lowered to different routines (`pretend_read`, `pretend_save`). | The real routines stay the authority and are untouched; the language spec says "a second backend for the runtime's file and console routines". By-name replacement needs no change at any call site. | If the owner wants a flag-driven single routine, the swap table becomes branches inside the real routines (touches real IR). Mechanical but wide. |
| P2 | Report routines write to descriptor 1 directly with `Prim::Write`, unbuffered, not through `write_all`. | `write_all` and the output buffer belong to the program under test and are swapped in test builds. | If a buffered report is wanted later, add a report buffer; the format does not change. |
| P3 | `input` items must be text (checker error `An input answer must be text; …`, Task 14). | The grammar allows any `arith`, and `ask` gives text; converting numbers silently would hide mistakes. | A program that wrote `input 120` stops compiling; the fix is a quote. If the owner wants conversion, Task 14 becomes a lowering conversion instead. |
| P4 | The ordinal in the missing-answer message is a word from `first` to `tenth`, then `question number K`; singular `1 answer`, plural otherwise (`0 answers`). The spec gives only the plural, third-question case. | Matches the spec's example exactly and stays grammatical. | A test that asks an eleventh question reads slightly differently than the owner might choose; one string in one routine. |
| P5 | A child that ends in a way other than exit 0 or 1 is `fail`, with `(status N)`; a signal shows as `256 + signal`. | One number, no signal-name table; the same vocabulary is `Wait`'s result and the CLI's message. | `status 267` is less friendly than `killed by signal 11`; one table later if wanted. |
| P6 | The refusal is `!cfg!(all(target_os = "linux", target_arch = "x86_64"))`, not "non-Linux" (D38's wording), with the same message and status 3. | An arm64 Linux host cannot run the x86-64 executable either; the message stays as decided. | Linux/arm64 users read "Linux only" and are puzzled; widening the message is one string. |
| P7 | Failures 6, 7 and 8 cannot occur in a test: tables count as cleared, and a pretend file only holds text the CSV writer made. They are reachable only from a unit test that writes the pretend word directly (19a). D36 says "can still occur"; it is true of the code, not of any program. | There is no way to put a bad file in a pretend file from Gloss source. | If the owner wants fixtures to inject bad files (a future `file "e.csv" holds "..."`), that is a language change, not an M4 task. |
| P8 | `gloss test` streams the test run's output as it arrives (piped, copied chunk by chunk to `out` and `err`), not captured and forwarded at the end. (Changed by the controller from the draft's capture-at-end.) | A hung test (D40) must still leave every finished test's result on screen; `cli::run` keeps its `out`/`err` writers, so unit tests still drive it. | A thread for standard error; ordering between the two streams is not exact, which the report never relies on (everything is on standard output). |
| P9 | A missing pretend file prints no `not found; starting with an empty table` notice. | The spec says notices on the error output are not in `shown`, and D34 says nothing goes to standard error. | If the owner wants the notice, it would appear in `shown`'s neighbour, the failure text, only on failure; not worth it. |
| P10 | The failure text area is 1,792 bytes and extra bytes are dropped; the shown bytes grow without a cap except while `FAILING` is set and they are full. | A failure routine must not allocate (it may be the out-of-memory report). | A failure message longer than 1,792 bytes (a very long path in a failure 6, 7) is cut; the messages are normally under 300. |
| P11 | `shown` lines are split at `\n`; a final fragment without `\n` is not a line (only possible when a failure interrupted a `show`, and then the test is over). | Matches the spec's "every line printed by `show`". | None observable from `expect`. |
| P12 | The title is printed with invisible characters spelled `\u{XXXX}`, at compile time. | Same rule as every failure message (M2-R30); a title cannot reorder the report. | A title using such characters on purpose reads escaped. |
| P13 | In M4a, test-mode lowering refuses `ask`, `load`, `save`, `input` and `shown` anywhere in the program (features included), with `this compiler cannot build this statement yet`. So a program whose `start` loads cannot be tested until Tasks 15 and 19b land. | A test build must never reach a real file primitive or standard input (D36); features are lowered whole. | M4a alone is useful only for programs without files and questions; M4a is a stepping stone, not a release. |
| P14 | If `Fork` itself fails, the parent's `Wait` gets a negative error and the test is reported `ended without a result (status -10)` (or the error `Wait` gives). | A fork failure is rare and the report is truthful. | A less clear message than `could not start the test`; one more branch in the driver. |
| P15 | A program with no tests never reaches the driver: the CLI prints `No tests in x.gloss` and exits 0. The library entry still builds a driver that prints `0 passed, 0 failed`. | The spec says nothing; D37 decides the CLI line. | None. |
| P16 | The test executable runs with standard input closed (`Stdio::null`), and it never reads it (D36, Task 15b's `ask_never_reads_standard_input`). | A test run is deterministic. | A debugging aid that pipes a person's answers is not possible; `input` is the tool. |
| P17 | If the test executable itself ends in a way other than 0 or 1 (the driver crashed, out of memory in the parent), `gloss test` prints `the test run ended without finishing (status N)` to standard error and exits 1. | 3 is a compile error; 1 is "not all tests passed", which is true. | A script cannot tell it from a failing test by status; the message tells it. |
| P18 | The task sizes assume that the cloning derives of Task 2 are one-line edits; if they spread beyond three files the derives become their own PR 2a. | Plan estimates run 40% low. | One more PR. |
