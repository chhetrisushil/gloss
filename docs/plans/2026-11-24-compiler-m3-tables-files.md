# Gloss compiler M3: tables and files - implementation plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task by task. Steps use checkbox (`- [ ]`) syntax for tracking. Ledger: `.superpowers/sdd/2026-11-24-compiler-m3-tables-files/progress.md`.

**Goal:** `gloss build` approves a program's file access (permission list, question, `--allow`, `.allow`, status 4) and builds programs with tables; the executable loads and saves CSV tables safely (`openat2` without symbolic links, write-then-rename), stops with failures 6–8 in the agreed words, and prints its list with `--permissions`.

**Architecture:** approval lives in the CLI and a new `permissions` module (pure Rust, unit tested). Files at run time follow D19: four new OS primitives (`Open`, `Close`, `Sync`, `Rename`) are the only target-specific part; everything above them (reading a whole file, UTF-8 check, CSV reading and writing, safe saving, failure messages) is runtime written in IR, and `irsim` gets an in-memory file system so every file case is also checked there (D21, D29).

**Tech stack:** Rust 2024, stable 1.87 or later, no crates. OrbStack Alpine machine `gloss-amd64` (Linux kernel ≥ 5.6, needed for `openat2`) runs executables on the Mac.

**Spec:** `docs/specs/2026-11-22-compiler-design.md` (binding; §4.7 and D23–D29 for this milestone). The language is defined by `spec/v0.md`: §8 (records and tables, file format, saving, file permissions), §10 (run-time failures), §11 (building, options, exit status), §12 (runtime: files). Conflicts about the language resolve against the language spec; about the implementation, against the design spec.

**How to read the tasks.**
- **Binding:** every exact string (messages, list lines, `.allow` lines), the tests named in each task with their assertions, the types and signatures in each **Interfaces** block, and the primitive conventions in Task 5.
- **Guidance:** implementation steps give the algorithm and key code; the implementer writes the rest.
- A test may change only if it contradicts the language spec or the design spec. Record that as a ruling in the ledger.

## Global constraints

- [D1] Rust 2024 edition, `rust-version = "1.87"`. [D3] No dependencies of any kind. [D4] One crate.
- [D18] Nothing outside `src/target/` names an x86-64 register, instruction, system-call number or Linux flag value. `lower`, `runtime` and `irsim` speak only IR.
- [D19] Every runtime routine is written in IR. The only new target-specific code is the four primitives of Task 5.
- [D21] `irsim` is `cfg(test)` only; the real executable is the authority; both agree on every `tests/run` case, including the files left behind.
- [D23] Approval (Tasks 1–4) lands before any table program can build. No PR may let `gloss build` write an executable for a program with a table unless its access was approved.
- [D24] File names in the permission list, `.allow` and `--permissions` use the display form: printable ASCII as is; otherwise `"<name>" (<escaped>)`, escaped = every character outside printable ASCII as `\u{X…}` (upper-case hex, no padding: `\u{E9}`), as in D24's example.
- [D25] No answer is ever read from standard input unless it is a terminal.
- [D26] Failures 6–8 use exactly the messages in design §4.7. Cell and column text from a file goes through `err_shown` (≤ 40 characters, invisible characters as `\u{XXXX}`, `"` doubled).
- [D27] The list is printed by `gloss build` only for programs that declare a table.
- [D28] After a successful build, `.allow` holds exactly the current list.
- §8 safe writing: `save` writes `<name>.gloss-new` in the table's folder, `fsync`s it, renames it over the table, then `fsync`s the folder. A failure before the rename leaves the table file untouched.
- §8: every table path is opened relative to the current folder with "beneath, no symbolic links" resolution; nothing in the runtime opens any other path.
- Problem strings never end with a period (as in M1, M2).

## Rules for every PR

- **One PR at a time** (owner directive). The next PR opens only after the previous one merges; branched from the new `main`. Never stacked.
- **Size.** At most 200 changed lines (tests and expected files included) and at most 10 files. Estimates run about 40% low: every task below is estimated at ≤ 140 lines; a task that still grows past the cap is split into sequential PRs (`m3/tNNa-…`, `m3/tNNb-…`), recorded as a ruling. A run case is two or more files; budget files, not just lines.
- **Never** `#[rustfmt::skip]` or golfed code to fit the cap; split instead.
- **Branches:** `m3/tNN-<slug>` from `origin/main`.
- **Pull requests:** draft (`gh pr create --draft`); merged with `gh pr merge <n> --rebase` on the owner's word.
- **Commits** end with:
  ```
  Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>
  Claude-Session: https://claude.ai/code/session_011uGukQR6EwQQdiSVNu4ERC
  ```
  PR bodies end with `🤖 Generated with [Claude Code](https://claude.com/claude-code)`, a blank line, and the session link.
- **Verify gate** before a task is done, at every commit: `CARGO_BUILD_JOBS=4 cargo fmt --check && cargo clippy --all-targets -- -D warnings && cargo test`. The report says whether the run tests ran or were skipped.
- **Reviews:** implementers and reviewers use `sonnet`. Tasks marked **risky** get their **first** review on `opus` (security boundary, encoder, untrusted input); every re-review uses `sonnet`.
- **Branch hygiene:** never delete an implementer's branch before `git log --oneline main..<branch>` prints nothing.

## Resource limits

- At most 2 agents at once, reviews included; one `cargo` run at a time; `CARGO_BUILD_JOBS=4`.
- Tests that write files use `TempDir` under `CARGO_TARGET_TMPDIR` (run tests) or the existing `temp_named` helpers (CLI tests), removed on drop.
- The Linux machine is shared: never start or stop it from a test.

## Findings that shape this plan

| # | Finding (verified 2026-11-24, `.superpowers/sdd/m3-facts.md`) | Consequence |
|---|---|---|
| F1 | The D22 refusal is in `lower::lower` (`src/lower/mod.rs:189`), not the CLI; `load`/`save`/`clear` fall to `cannot_build` (`src/lower/stmt.rs:141`); a table name expression to `src/lower/expr.rs:156` | Task 7 lifts them one by one (D23 as amended) |
| F2 | `cli::run(args, out, err)` has no flags and no stdin; a third argument is a usage error | Task 3 adds `--allow` and a `Prompt` |
| F3 | `Checked`'s fields are private; nothing lists tables with their access | Task 1 adds `Checked::permissions()` |
| F4 | `Global::EntrySp` already holds the kernel's initial stack pointer (`argc` at `[EntrySp]`, `argv[i]` at `[EntrySp + 8 + 8i]`) | Task 4 needs no new primitive for arguments; `irsim`'s initial stack gains `args` |
| F5 | Primitives are `Write, Read, Map, Exit, StackSize`; `irsim` supports only fds 0–2 | Task 5 adds four primitives and an in-memory file system |
| F6 | The global layout is a fixed array duplicated in `src/target/x86_64/program.rs:13` and `src/irsim/memory.rs:17` | Task 7 adds one global, `Tables`, to both, with a test that the two arrays agree |
| F7 | The run harness never sets the program's folder; the sweep rejects any suffix but `.out .in .err .status .ulimit` | Task 6 adds a work folder and the D29 suffixes |
| F8 | `err_shown` takes a Gloss text address and uses `NumBuf` as scratch | CSV cells that may appear in a message are made into texts first; no message depends on `NumBuf` surviving |
| F9 | `write_all` exits 2 silently on a failed write | Saving uses a new `file_write` that returns the error, so it can become failure 7 |
| F10 | `std::io::IsTerminal` is stable (1.70) | `main.rs` decides "is a terminal"; the CLI takes it as input (D25) |

## File map

| File | Responsibility | Task |
|---|---|---|
| `src/permissions.rs` | The list, D24 display form, `.allow` parse and render, what needs approval | 1, 2 |
| `src/lib.rs` | `Checked::permissions()` | 1 |
| `src/cli.rs`, `src/approve.rs`, `src/main.rs`, `tests/cli.rs` | `--allow`, the question, status 4, writing `.allow` | 3 |
| `src/runtime/args.rs`, `src/lower/mod.rs`, `src/irsim/machine.rs` | `--permissions` in the executable | 4 |
| `src/ir/mod.rs`, `src/ir/text.rs`, `src/irsim/files.rs`, `src/irsim/system.rs` | The four file primitives in IR and `irsim` | 5a |
| `src/target/x86_64/linux.rs`, difftest | The primitives on Linux | 5b |
| `tests/run_sweep.inc`, `tests/run.rs`, `tests/run_support/mod.rs`, `src/irsim/cases.rs` | File run cases (D29) | 6a, 6b |
| `src/lower/table.rs`, globals in `program.rs` and `irsim/memory.rs` | Tables in memory, `clear` | 7a, 7b |
| `src/runtime/csv_write.rs`, `src/runtime/save.rs` | `save`, failure 8 | 8a, 8b |
| `src/runtime/file_read.rs`, `src/runtime/csv_read.rs`, `src/runtime/load.rs` | `load` | 9a–9d |
| `tests/run/table_*` | Failures 6–7 end to end, links, root skips | 10 |
| `spec/v0.md`, `tests/spec_examples.rs`, `examples/` | Spec wording, examples | 11 |
| `README.md`, design spec §6 | As built | 12 |

---

### Task 1: The permission list and the display form

**Files:** create `src/permissions.rs`; modify `src/lib.rs` (module, `Checked::permissions`).

**Interfaces (produces):**
```rust
pub enum Access { ReadOnly, ReadReplace }
pub struct Entry { pub file: String, pub access: Access }
/// Every declared table's file, in declaration order. ReadReplace when a `save` of
/// that table appears in some feature body (test blocks do not count).
pub fn list(program: &Program) -> Vec<Entry>;
/// D24: printable ASCII as is; otherwise `"<name>" (<escaped>)`.
pub fn display_name(file: &str) -> String;
/// The lines a person sees: "This program can:\n", one line per entry, "  nothing else\n".
pub fn render(entries: &[Entry]) -> String;
impl Checked { pub fn permissions(&self) -> Vec<Entry> }
```
An entry line is two spaces, the access words padded to 19 columns (`read and replace   `, `read only          `), then the display name. "Printable ASCII" is bytes 0x20–0x7E.

**Tests (binding), in `src/permissions.rs`:**
- `a_saved_table_is_read_and_replace` — `table es of E in "e.csv"` + `table rs of E in "r.csv"`, a feature that loads both and saves `es`: `render` gives exactly
  `"This program can:\n  read and replace   e.csv\n  read only          r.csv\n  nothing else\n"`.
- `a_save_inside_a_test_does_not_count` — the only `save es` is in a `test` block: `es` is read only.
- `no_tables_lists_nothing_else` — `"This program can:\n  nothing else\n"`.
- `display_names` — `display_name("expenses.csv") == "expenses.csv"`; `display_name("data/a b.csv") == "data/a b.csv"`; `display_name("résumé.csv") == "\"résumé.csv\" (r\\u{E9}sum\\u{E9}.csv)"`; a Cyrillic `а` (U+0430) in `"dаta.csv"` gives `"\"dаta.csv\" (d\\u{430}ta.csv)"`; an astral character gives five hex digits (`\u{1F600}`).
- `checked_permissions_match_list` — `check_program` then `Checked::permissions()` equals `list(&program)` for the first test's program.

**Steps:**
- [ ] Write the tests; run `cargo test permissions` — fails (module missing).
- [ ] Implement `list` by walking `program.features()` bodies with `crate::ast::walk::for_each_stmt` for `StmtKind::Save(name)`, collecting saved table names, then mapping `program.tables()` in order.
- [ ] Implement `display_name` and `render`; wire `Checked::permissions`.
- [ ] Gate; commit "List the files a program can touch".

**Fail-proof:** a `list` that ignores test blocks only by accident (e.g. never walks anything) would make every table read only — `a_saved_table_is_read_and_replace` catches it. An escape using lower-case hex or zero padding fails `display_names`.
**Review:** risky (it decides what a person approves) — first review `opus`.
**Estimate:** ~120 lines, 2 files.

### Task 2: `.allow` — parse, render, and what needs approval

**Files:** modify `src/permissions.rs`.

**Interfaces (produces):**
```rust
/// The file body for `<source>.allow` (D28).
pub fn allow_file(source_name: &str, entries: &[Entry]) -> String;
/// The approvals a `.allow` text records, as (display-name, access) pairs.
/// Err((line, problem)) for a line that is not the note, blank, or an entry line.
pub fn parse_allow(text: &str) -> Result<Vec<(String, Access)>, (usize, String)>;
/// The entries that need a fresh approval, given the recorded approvals and the
/// `--allow` files (raw names as written in table lines).
pub fn needing_approval<'a>(entries: &'a [Entry], recorded: &[(String, Access)], flags: &[String]) -> Vec<&'a Entry>;
```
`allow_file("expenses.gloss", …)` is `"note Files this program may touch; approved for expenses.gloss.\n"` followed by the entry lines of `render` (without the "This program can:" and "nothing else" lines). Recorded approvals are matched to entries by display name, so `parse_allow` never decodes the escaped form. An entry needs approval when no flag names its file and no recorded pair has its display name with an access that covers it (`ReadReplace` covers both; `ReadOnly` covers only `ReadOnly`).

**Problems (binding strings):** `this line is not an approval; expected read only or read and replace and a file name` for any other line.

**Tests (binding):**
- `allow_file_round_trips` — `parse_allow(&allow_file(...))` gives the display names and accesses back.
- `a_read_only_approval_does_not_cover_replace`, `a_flag_covers_any_access`, `a_recorded_replace_covers_read_only`.
- `an_unknown_recorded_file_is_ignored` — a stale line approves nothing and causes no error.
- `a_bad_line_names_its_number` — `"note x\nread sometimes  e.csv\n"` gives `Err((2, …))` with the string above.
- `blank_lines_and_the_note_are_skipped` — any line starting `note ` is skipped, as are empty lines.

**Fail-proof:** a matcher comparing raw names instead of display names would pass ASCII tests only; `allow_file_round_trips` includes `résumé.csv`.
**Review:** risky — first review `opus`.
**Estimate:** ~120 lines, 1 file.

### Task 3: Approval in `gloss build`

Split into two PRs.

**3a — arguments and the prompt (`m3/t03a-allow-args`).** Files: `src/cli.rs`, `src/main.rs`, `tests/cli.rs`.

Interfaces:
```rust
pub trait Prompt { fn is_terminal(&self) -> bool; fn ask(&mut self, question: &str) -> Option<String>; }
pub fn run(args: &[String], out: &mut dyn Write, err: &mut dyn Write, prompt: &mut dyn Prompt) -> i32;
```
`main.rs` passes a `Prompt` built on `std::io::stdin()`, with `is_terminal()` from `std::io::IsTerminal`; `ask` writes the question to standard output, flushes, and reads one line (None at end of input). Arguments: `gloss build <file.gloss> [--allow <file>]...`. `--allow` is accepted only by `build` (and later `trace`); for `format` it is a usage error. A `--allow` with no file name after it is a usage error. Existing usage text gains one line: `options: --allow <file> (build, trace)`.

Tests: `extra_arguments_print_the_usage` keeps passing for a stray third argument that is not `--allow`; new `allow_needs_a_file_name`, `format_refuses_allow`. Unit tests in `cli.rs` use a fake `Prompt`.

**3b — the approval flow (`m3/t03b-approve`).** Files: `src/approve.rs` (new), `src/lib.rs`, `src/cli.rs`, `tests/cli.rs`.

Order in `build`: check → if the program declares a table: list, print it to standard output, approve (below), write `.allow` if it changed → lower and write the executable (still refused with D22 for table programs until Task 7a, which is fine: approval is what this task tests).

Approval:
1. Read `<stem>.allow` if it exists (`cannot read <path>: <error>`, status 3). Parse it; a bad line prints `<stem>.allow line N: <problem>.` and exits 3.
2. A `--allow` naming a file that is not in the list: `--allow x.csv: this program has no table in x.csv.`, exit 3.
3. `needing_approval` empty → continue, nothing asked.
4. Otherwise, if `prompt.is_terminal()`: ask `Allow? `; `yes` or `y`, ignoring case and surrounding spaces, approves; anything else or None: print `Not approved.` to standard error, exit 4, no executable.
5. Not a terminal: print `Not approved: <display names, comma-separated>. Build in a terminal to answer, or add --allow <first file as written>.` to standard error; exit 4; no executable; `.allow` untouched.
6. After approval: write `allow_file(...)` atomically (mode 0644) only if it differs from what is there; print `Approval saved in <stem>.allow` to standard output when written.

Tests (binding, `tests/cli.rs`, non-terminal path through the real binary):
- `a_table_program_without_approval_exits_4` — no `.allow`, no flag: status 4; stdout is the list; stderr is the `Not approved: …` line; no executable; no `.allow`.
- `allow_writes_the_approval` — `--allow e.csv`: `.allow` written with the exact D28 body; stdout ends `Approval saved in t.allow\n` followed by the D22 refusal on stderr (status 3) until Task 7a changes it.
- `a_recorded_approval_asks_nothing` — second build with no flag: no `Approval saved`, list still printed.
- `read_only_to_replace_needs_approval_again` — `.allow` says read only, program now saves: status 4.
- `a_dropped_table_shrinks_allow` — a program with one of its two tables removed, built with its remaining approval: `.allow` rewritten with one line.
- `a_broken_allow_exits_3` — exact message `t.allow line 2: this line is not an approval; expected read only or read and replace and a file name.`
- `an_allow_for_an_unknown_file_exits_3`.
- `a_program_without_tables_prints_no_list` — `Built x` only (D27).
Unit tests (fake `Prompt`): `yes_approves`, `y_in_capitals_approves`, `no_refuses_with_status_4`, `end_of_input_refuses`, `the_question_is_not_asked_when_covered`.

**Fail-proof:** a flow that reads standard input when it is not a terminal would hang or approve — the binary tests run with stdin null and must exit 4 promptly. A writer that rewrites `.allow` on every build is caught by `a_recorded_approval_asks_nothing` (asserts the file's modification time or content unchanged and no "Approval saved").
**Review:** risky — first review `opus` (each PR).
**Estimate:** 3a ~110 lines; 3b ~180 lines (pre-split further into 3b1 flow + unit tests, 3b2 binary tests, if over).

### Task 4: `--permissions` in the executable

**Files:** create `src/runtime/args.rs`; modify `src/runtime/mod.rs`, `src/runtime/core.rs` (`_start`), `src/lower/mod.rs`, `src/irsim/machine.rs`, `src/irsim/mod.rs`, tests.

**Interfaces:**
- Lowering always emits a function `permissions_text` (no params) that leaves in `a` the address of a module text holding `permissions::render(&list)` for this program.
- Runtime `args_check()`: if `[EntrySp] == 2` and the NUL-terminated `argv[1]` equals `--permissions` byte for byte: `out_text(permissions_text())`, `flush`, exit 0. Otherwise return. `_start` becomes `heap_init; stack_init; args_check; f_start; flush; exit(0)`.
- `irsim::run_with` gains the program arguments (`args: &[&str]`, after `argv[0]`); `initial_stack` writes `argc = 1 + args.len()` and the strings.

**Tests (binding):**
- irsim: `permissions_prints_the_list_and_exits_0` (a table program, args `["--permissions"]`: stdout is the list, status 0, `start` not run); `other_arguments_are_ignored` (`["--permissions", "x"]` and `["-p"]` run `start`); `a_program_without_tables_prints_nothing_else`.
- `_start`'s IR text test updated.

**Fail-proof:** a prefix compare (`--perm` accepted) is caught by an extra irsim case `["--permissionsX"]` running `start`.
**Review:** sonnet.
**Estimate:** ~140 lines.

### Task 5: File primitives

**5a — IR and irsim (`m3/t05a-file-prims`).** Files: `src/ir/mod.rs`, `src/ir/text.rs`, `src/irsim/files.rs` (new), `src/irsim/system.rs`, `src/irsim/mod.rs`.

Interfaces (binding conventions; arguments pushed left to right, result in `a`, a negative result is `-errno`):
```rust
pub enum OpenMode { Read, Folder, Create, Probe }
pub enum Prim { …, Open(OpenMode), Close, Sync, Rename }
```
- `Open(mode)` (folder_fd, path) → fd or negative. `folder_fd` is an fd from `Open(Folder)` or `CWD` (`pub const CWD: i64 = -100` in `src/ir/mod.rs`, the only place the value appears outside `src/target/`). `path` is the address of NUL-terminated bytes. Resolution is always "beneath the folder, no symbolic links". Modes: `Read` (read only), `Folder` (a folder handle for later opens and `Sync`), `Create` (write only, create, truncate, permissions 0644), `Probe` (no access; used to see whether a name exists and is not a link).
- `Close` (fd) → 0 or negative. `Sync` (fd) → 0 or negative. `Rename` (folder_fd, from, to) → 0 or negative, both names in that folder.
- Error numbers the runtime tests: `pub const NOT_FOUND: i64 = -2`, `pub const LINK: i64 = -40`, `pub const ESCAPES: i64 = -18` in `src/ir/mod.rs` (Linux values; documented as the IR's error vocabulary, which other targets map to).
- `Write`/`Read` accept any fd from `Open`.
- IR text: `system open read`, `system open folder`, `system open create`, `system open probe`, `system close`, `system sync`, `system rename`.

irsim file system (`src/irsim/files.rs`): `pub struct Files { pub files: BTreeMap<String, Vec<u8>>, pub folders: BTreeSet<String>, pub links: BTreeSet<String>, pub synced: Vec<String> }` keyed by normalised relative path; an fd table in `Machine`. A path component that is in `links` gives `LINK`; a missing file or folder gives `NOT_FOUND`; `Create` in a missing folder gives `NOT_FOUND`. `Rename` replaces; `Sync` records the path (folder or file) in `synced` so tests can check the order. `Faults` gains `failing_open: Option<String>` (that path's open gives `-13`) and `failing_file_write: bool` (writes to fd ≥ 3 give `-5`). `run_with` takes `&mut Files`; `Outcome` gains `files` (the final state).

Tests: unit tests in `files.rs` for each rule (open beneath, link refusal at any depth, create truncates, rename replaces, missing folder); `ir/text.rs` round trip of the new instructions.

**5b — Linux (`m3/t05b-linux-files`).** Files: `src/target/x86_64/linux.rs`, `src/target/x86_64/difftest/sys.rs`, `src/target/x86_64/lower_tests.rs`.

`Open(mode)`: build `struct open_how { u64 flags; u64 mode; u64 resolve; }` in 24 bytes reserved on the stack; `openat2(dirfd, path, &how, 24)` = syscall 437; `resolve = RESOLVE_BENEATH (0x08) | RESOLVE_NO_SYMLINKS (0x04)`; flags: Read `O_RDONLY|O_CLOEXEC`, Folder `O_PATH|O_DIRECTORY|O_CLOEXEC`, Create `O_WRONLY|O_CREAT|O_TRUNC|O_CLOEXEC` with mode 0o644, Probe `O_PATH|O_CLOEXEC`. `Close` = 3, `Sync` = `fsync` 74, `Rename` = `renameat` 264 (same folder fd twice). `rcx` (IR `b`) is kept as for every primitive.

Tests: difftest programs per primitive (open a file the test created, read it back; open through a link gives the link error; create and rename; `b` survives each); `lower_tests` instruction text for `Open(Read)`.

**Fail-proof:** a missing `RESOLVE_NO_SYMLINKS` passes every test except the link difftest, which must open `link/x` where `link` → a real folder and expect `LINK` (it is not enough for the final component to be the link).
**Review:** 5a sonnet; 5b risky — first review `opus`.
**Estimate:** 5a ~170 lines (split 5a1 IR + text, 5a2 irsim files if over); 5b ~130 lines.

### Task 6: File run cases (D29)

**6a — real runs (`m3/t06a-file-cases`).** Files: `tests/run_sweep.inc`, `tests/run.rs`, `tests/run_support/mod.rs`.

- Suffixes accepted by the sweep: `.files` (folder), `.after` (folder), `.links`, `.allow`, `.args`, besides the existing ones, each only beside a `.gloss` of the same stem. The orphan test gains `x.files` and `y.links` without a `.gloss`.
- Each case runs in `TempDir/<name>/` holding the source, `<name>.allow` (copied when present) and the executable, and a `work/` folder: `.files/` is copied into `work/`; `.links` lines `link -> target` become symbolic links inside `work/`; `.args` lines become arguments. The program's current folder is `work/` (`current_dir` when run directly; under `orb`, the `sh -c` wrapper `cd`s first — extend the existing wrapper with the folder as one more positional argument).
- After the run, the files under `work/` (regular files only, recursively, relative paths, symbolic links listed as `link -> target`) must equal the files under `.after/` plus the `.links` lines, byte for byte, when `.after` exists; when it does not, `work/` must equal `.files/` (nothing changed).
- `run_support` gains `run_in(path, stdin, deadline, folder, args)`.

Tests: `the_sweep_reports_an_orphan` extended; unit tests of the folder comparison (equal, an extra file, a missing file, different bytes, a link listed differently); run case `permissions_flag` (`.args` holding `--permissions`, `.out` the list of a table program, `.allow` committed); run case `files_untouched` (a program without tables, `.files/` seeded, no `.after/`: proves seeding and comparison run).

**6b — irsim (`m3/t06b-irsim-files`).** Files: `src/irsim/cases.rs`. Seeds `Files` from `.files/` and `.links`, passes `.args`, compares `Outcome.files` with `.after/` (or `.files/`) the same way.

**Fail-proof:** a comparison that ignores extra files would miss a leftover `.gloss-new`; a test case writes a stray file and the harness must report it (a unit test of the comparison function with an extra file).
**Review:** sonnet.
**Estimate:** 6a ~170 lines; 6b ~90 lines.

### Task 7: Tables in memory

**7a — table values (`m3/t07a-tables`).** Files: `src/ir/mod.rs` (`Global::Tables`), `src/target/x86_64/program.rs`, `src/irsim/memory.rs`, `src/lower/table.rs` (new), `src/lower/mod.rs`, `src/lower/expr.rs`, `src/lower/tests.rs`, `tests/cli.rs`, `tests/spec_examples.rs`, run cases.

- `Global::Tables` (8 bytes) holds the address of a block of two words per table, in declaration order: the table's list and its loaded flag.
- Lowering always emits `tables_init` (allocates `16 * n` bytes, each list `list_new(4)`, each flag 0); `_start` calls it after `stack_init`.
- A table name expression loads its list from the block. Everything a list does now works on a table: `for each`, `count of`, `item`, `first`, `last`, `append`, field changes, passing to a `list of T` input.
- The D22 refusal is removed. `load` and `save` stay refused as `this compiler cannot build this statement yet` until Tasks 8b and 9d; `clear` comes in 7b.
- A test asserts `program.rs`'s and `irsim/memory.rs`'s global arrays list the same globals in the same order.

Tests: run cases `table_append` (append three records, show the table, count) and `table_input` (a table given to a feature input), both with `.allow`; `tests/cli.rs` tests from 3b that expected the D22 refusal now expect `Built t`; spec_examples' table check now expects either a build or the `cannot build this statement yet` problem.

**7b — `clear` (`m3/t07b-clear`).** Files: `src/lower/table.rs`, `src/lower/stmt.rs`, run cases.

- `clear t`: a new empty list (`list_new(4)`) into the table's word, flag set to 1.

Tests: `table_clear` run case (append, copy the table to a variable, clear, show both and the count).

**Fail-proof:** a `clear` that empties the old list in place instead of replacing it would change a list a caller copied earlier — `table_clear` copies the table to a variable first and shows it after the clear.
**Review:** sonnet.
**Estimate:** 7a ~170 lines (split if needed: globals + init, then lowering); 7b ~110 lines.

### Task 8: `save`

**8a — CSV writing (`m3/t08a-csv-write`).** Files: `src/runtime/csv_write.rs` (new), `src/runtime/mod.rs`, irsim unit tests.

`csv_text(list, desc, line) -> text`: a fresh text holding the header (field names in declaration order, comma-separated) and one line per record; each line ends `\n`. Number cells: the `show` form (`number_format`). Truth: `yes`/`no`. Text: as is, unless it contains `,`, `"`, `\r` or `\n` — then wrapped in `"` with each `"` doubled. Builds into a growing text (`grow_text`).

Tests (irsim, exact bytes): empty table → header line only; numbers `120.5`, `-3`, `0`; truths; a text with a comma, a quote, a line break; a field name with a space (written as is — names are identifiers, so this cannot happen; no test).

**8b — safe saving (`m3/t08b-save`).** Files: `src/runtime/save.rs` (new), `src/runtime/output.rs` (`file_write(fd, addr, len) -> 0 or negative`, looping on short writes), `src/lower/table.rs`, `src/lower/stmt.rs`, run cases.

Lowering passes compile-time constants as NUL-terminated module texts: the folder (`"."` or the path before the last `/`), the file name, the temporary name `<file name>.gloss-new`, and the display name (D24) for messages.
`table_save(list, desc, folder, name, temp, shown, line)`:
1. `Open(Folder)` (CWD, folder): `NOT_FOUND` → failure 7 `… cannot be written (its folder <folder> does not exist)`; `LINK`/`ESCAPES` → failure 7 symbolic link; other negative → failure 7 cannot be written.
2. `Open(Probe)` (dir, name): `LINK` → failure 7 symbolic link; `NOT_FOUND` or success is fine (close it).
3. `Open(Create)` (dir, temp); write `csv_text`; `Sync`; `Close`; `Rename` (dir, temp, name); `Sync` (dir); `Close` (dir). Any negative → failure 7 cannot be written. (A leftover `.gloss-new` after a failed write is allowed by §8.)
`save t` = the flag check — 0 gives failure 8, `line N: failure 8: <table> is saved here but was not loaded or cleared first` — then `table_save`.

Tests: run case `table_save_unloaded` (a feature saves a table that `start` did not load or clear on this path: status 2, exact failure 8, no file written); run cases `table_save_new` (clear, append, save: `.after/` has the file), `table_save_replace` (`.files/` has an old file; load-free: clear then save — `.after/` has the new content), `table_save_subfolder` (`data/e.csv`, folder exists), `table_save_no_folder` (status 2, exact failure 7 message). irsim unit test: `Files.synced` order is file, then folder, and the rename happens between them.

**Fail-proof:** writing straight to the table file passes every content test; `table_save_replace` seeds the table file and a `.links`-free `.after/`, and the irsim order test catches a missing temp file or rename. A save that skips the folder `fsync` is caught by the order test.
**Review:** 8a sonnet; 8b risky — first review `opus`.
**Estimate:** 8a ~140 lines; 8b ~170 lines.

### Task 9: `load`

**9a — reading a whole file (`m3/t09a-file-read`).** `src/runtime/file_read.rs`: `file_bytes(folder_fd, path, shown, line) -> text or 0`. `Open(Read)`; `NOT_FOUND` → write `<shown> not found; starting with an empty table\n` to standard error (after `flush`) and return 0; `LINK`/`ESCAPES` → failure 6 symbolic link; other negative → failure 6 cannot be read. Read in a loop into a growing text; a negative read → failure 6 cannot be read. `Close`. Then the UTF-8 check: the first invalid sequence gives failure 6 `<shown> is not UTF-8 text (line N)`, N counted by `\n` before it. Tests: irsim units for each outcome; the UTF-8 checker against Rust's `std::str::from_utf8` on a table of byte strings (overlongs, surrogates, truncated, > U+10FFFF).

**9b — CSV cells (`m3/t09b-csv-cells`).** `src/runtime/csv_read.rs`: a cursor over the text that yields one row at a time as a list of cell texts plus the line the row started on. RFC 4180: cells split on `,`; a cell starting with `"` runs to the closing `"`, `""` inside is one `"`, and may hold `,`, `\r`, `\n`; a row ends at `\n` or `\r\n` outside quotes; end of text ends the last row; one final empty line is ignored. An unclosed quote → failure 6 `<shown> line N: a quoted cell is not closed`, N the row's first line. Tests: irsim units with exact cell lists for each rule, including a quoted cell spanning lines (the next row's line number accounts for it).

**9c — the header (`m3/t09c-csv-header`).** Map header cells to fields by name: unknown → `<shown> line 1: column <cell> is not a field of <Record>`; repeated → `… column <cell> appears twice`; missing → `… column <field> is missing` (first missing field in declaration order). Unknown is checked before repeated, repeated before missing, left to right. An empty file (no header) loads as an empty table. Tests: irsim units with exact messages.

**9d — rows into records and `load` (`m3/t09d-load`).** For each row: wrong cell count → `<shown> line N: has C cells; the header has H`; per cell by field kind: text as is; number via `parse_number` — empty (after the number check's trimming) → `<field> is empty`, otherwise not a number → `<field> "<cell>" is not a number`; truth exactly `yes` or `no`, else `<field> "<cell>" is not yes or no`. Build the record, `list_push` it into a new list; only after the last row, store the new list in the table's word and set the flag. Lower `load t`. Tests: run cases `table_load` (load, show, count), `table_load_missing` (notice on stderr, empty table, status 0), `table_round_trip` (load, append, save; `.after/`), `table_load_quoted` (cells with commas, quotes, line breaks, `\r\n` endings).

**Fail-proof:** a failing load ends the program (failure 6), so building into a new list and swapping it in last is not observable and has no test; it is kept so a later `gloss test` (M4) that catches failures cannot see half a table. What is tested: a bad row 3 after two good rows fails with row 3's line number and shows nothing that `start` would show after the load; `table_round_trip` proves a loaded table is the one saved; every message is exact.
**Review:** 9a–9d risky (untrusted input) — first review `opus` for each.
**Estimate:** each ~140–170 lines.

### Task 10: Failures 6 and 7 end to end

**Files:** `tests/run/table_fail_*.gloss` with `.files/`, `.links`, `.err`, `.status`, `.allow`; `tests/run.rs` (root skip).

One run case per row of the design's failure table for 6 and 7 not already covered by Tasks 8b and 9d, each with the exact `.err` and status 2 and `.after/` proving the table file is untouched where it exists: number, empty number, truth, cell count, unknown column, missing column, repeated column, unclosed quote, not UTF-8, symbolic link on load (a linked folder in the middle of `data/e.csv`), symbolic link on save (the table itself is a link), cannot be read (mode 000: the harness applies `.mode` lines `path 000` after seeding — add the suffix here), cannot be written (a read-only folder, same `.mode`). The two permission cases skip visibly when running as root (`running_as_root()` moves to `run_support`). Cell text over 40 characters and with an invisible character, once each.

**Fail-proof:** each `.err` is exact; `MIN_CASES` rises with every case; the root-skip prints `SKIP` and is counted so a skip cannot hide a pass.
**Review:** sonnet.
**Estimate:** many small files — split by failure number (10a failure 6 data, 10b failure 6 header and file, 10c failure 7 and links), each ≤ 10 files.

### Task 11: The language spec and examples

**Files:** `spec/v0.md` (§10 table rows 6–8 get the design's messages; §8 gets one sentence for D27, marked as a deviation), `tests/spec_examples.rs` (the failures 6–8 check now compares the spec table with the design's messages; table programs in spec examples must build), `examples/expenses.gloss` (+ `.files`-style data is not available to examples: the example clears, appends and saves, then loads and shows; `.out`), `examples/report.gloss` (read-only: loads a committed `examples/report.csv`, prints totals), `examples/README.md`, `tests/examples.rs` (examples run in a temp copy of `examples/` as the current folder; `MIN_EXAMPLES` 11).

**Review:** sonnet. **Estimate:** ~170 lines; split examples into 11b if over.

### Task 12: As built

**Files:** `README.md` (M3 done; tables; `--allow`; `.allow`; `--permissions`), design spec §6 "As built — Milestone 3" (what shipped, deviations by decision id, PRs, rulings M3-R*, tests, known limits), status line.

**Review:** sonnet. **Estimate:** ~150 lines.

---

## Self-review (done while writing)

- **Spec coverage:** §8 file format (9b–9d, 8a), saving table (7b, 8b), safe writing (8b), permissions and approval (1–4), symbolic links (5b, 8b, 9a, 10), missing file notice (9a), `clear` (7b); §10 failures 6–8 (8b, 9a–9d, 10, 11); §11 `--allow`, status 4, `--permissions` (3, 4). `trace`'s `--allow` is M5.
- **Not in M3:** test fixtures and pretend files (M4), `trace` (M5).
- **Conflicts ruled in this plan:** P1 — the D22 refusal is lifted in 7a, not the last task (design D23 amended 2026-11-24). P2 — a `save` in a test block does not make a table read and replace (design §4.7 amended). P3 — `--allow` for a file the program does not declare is a compile-level error (status 3), so a typo in a script cannot silently approve nothing.
