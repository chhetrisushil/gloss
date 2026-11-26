# Gloss compiler M6: `gloss explain` - implementation plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task by task. Steps use checkbox (`- [ ]`) syntax for tracking. Ledger: `.superpowers/sdd/2026-11-26-compiler-m6-explain/progress.md`.

**Goal:** `gloss explain x.gloss` prints every source line of the real build beside the x86-64 instructions it became, the inferred variables of each feature at its top, and then every runtime routine the executable contains. Nothing is left out, and nothing is printed that the executable doesn't run.

**Architecture:**
- `program::executable` is split so that `assemble` makes the instruction list and `executable` encodes it (D51).
- The x86-64 target turns that same assembly into a target-neutral listing: routines made of line markers, labels and instruction texts, each instruction with typed references (D18).
- `lower_explained` returns the real module plus `Notes`: slot names, inputs and failure labels per feature.
- `src/explain/` prints the listing with headers and comments (D52 to D55).
- The CLI prints it on standard output (D56).

**Tech stack:** Rust 2024, stable 1.87 or later, no crates. Nothing in M6 runs an executable, so every M6 test runs on the Mac as well as on Linux.

**Spec:** `docs/specs/2026-11-22-compiler-design.md` is binding. For this milestone that means D51 to D58 and §4.10. The language is defined by `spec/v0.md` §10 (Explain) and §11 (commands). Conflicts about the language resolve against the language spec; conflicts about the implementation resolve against the design spec.

**How to read the tasks.**
- **Binding:** every exact string (headers, comments, messages), the tests named in each task with their assertions, and the types and signatures in each **Interfaces** block.
- **Guidance:** implementation steps give the hook point (file:line at `main` @ de25111) and the key code; the implementer writes the rest. Line numbers drift as tasks land, so find the named function.
- A test may change only if it contradicts the language spec or the design spec. Record that change as a ruling in the ledger.

## Global constraints

- [D1] Rust 2024 edition, `rust-version = "1.87"`. [D3] No dependencies. [D4] One crate.
- [D18] Only `src/target/` knows x86-64 and Linux numbers. `src/explain/` sees only `target::listing` types, never `X`, `R` or a syscall number.
- [D51] Explain prints the instructions of the real build exactly as encoded. Real, test and trace executables are byte-for-byte unchanged by M6.
- [D53] Comments appear only for slots and inputs with names, read-only texts, jumps to failure labels and system calls. No other instruction gets a comment.
- [D56] `gloss explain` asks for no approval, writes no file, runs nothing and works on any host.
- Problem strings never end with a period (as in M1 to M5). Messages to the user are exact.

## Rules for every PR

- **One PR at a time** (owner directive). When the owner asks for a chain, each task's branch is cut from the previous task's branch, and that base is recorded on the `dispatched` line.
- **Size.** At most 200 changed lines (tests and expected files included) and at most 10 files. Estimates run about 40% low: each task states its estimate and that figure times 1.4. A task that still grows past the cap is split into sequential branches (`m6/tNNa-…`, `m6/tNNb-…`), recorded as a ruling. Never trim comments or tests to fit, and never use `#[rustfmt::skip]` or golfed code.
- **Branches:** `m6/tNN-<slug>`.
- **Pull requests:** draft (`gh pr create --draft`), merged with `gh pr merge <n> --rebase` on the owner's word.
- **Commits:** one per branch, ending with
  ```
  Co-Authored-By: Claude Sonnet 5.5 <noreply@anthropic.com>
  Claude-Session: https://claude.ai/code/session_011uGukQR6EwQQdiSVNu4ERC
  ```
  PR bodies end with `🤖 Generated with [Claude Code](https://claude.com/claude-code)`, a blank line, and the session link.
- **Verify gate** at every commit: `CARGO_BUILD_JOBS=4 cargo fmt --check && cargo clippy --all-targets -- -D warnings && cargo test`. Task 1 also runs the Linux-only tests (design spec M4 As built, "Running Linux-only tests on the Mac"), because it sits on the path of every executable.
- **Reviews:** implementers and reviewers use `sonnet`. Task 1 is **risky**: its first review uses `opus`. Every re-review uses `sonnet`.
- **Branch hygiene:** never delete a branch before `git log --oneline main..<branch>` prints nothing (or `git cherry` shows every commit as `-`).

## Resource limits

- At most 2 agents at once, reviews included. One `cargo` run at a time, with `CARGO_BUILD_JOBS=4`.
- Agents never create extra cargo target directories, give any scratch script a unique name, and never kill a process they did not start.
- An agent whose command is denied stops and reports it. It never works around the denial.

## Findings that shape this plan (verified at `main` @ de25111)

| # | Finding | Consequence |
|---|---|---|
| F1 | `program::executable` (`src/target/x86_64/program.rs:60`) lowers each function with `lower_function`, appends `arith::helpers()`, encodes the concatenation, then lays out rodata and globals | Task 1 splits off everything up to `encode` as `assemble` |
| F2 | `lower_function` (`src/target/x86_64/lower.rs:18`) emits `X::Comment("line N")` for `Inst::Line(N)` and labels `L<function>_<n>` for `Inst::Mark(Label(n))` | Task 2 maps these to `Item::Line` and `Item::Label { local: Some(n) }` |
| F3 | Slot `k` is `[rbp-8k]`; argument `k` of `p` is `[rbp+16+8(p-k)]`; the saved-`b` word is `[rbp-8(slots+1)]` (`lower.rs:58-80`) | Task 3's refs; the saved-`b` word is not a slot |
| F4 | `linux::system` (`src/target/x86_64/linux.rs:54`) puts each call's number in `rax` with `X::MovRI(R::Rax, N)` before `X::Syscall`; the numbers are the constants at `linux.rs:8-33` | Task 3 names a `syscall` from the last `MovRI` into `rax` in the same routine |
| F5 | `assign_slots` (`src/lower/mod.rs:261`) numbers variables, then loop variables, from 1; a loop variable with a variable's name shares its slot | Task 4 takes slot names from `Lowerer::slots` |
| F6 | `Fail` (`src/lower/fail.rs:6`) holds the label, line, code, three message parts and the run-time pieces (`value`, `shown`, `item`); `emit_fails` drains them in `finish` | Task 5 records a note per failure before the drain |
| F7 | `lower_program` (`src/lower/mod.rs:311`) already adds the reachable runtime; `Checked` (`src/lib.rs:57`) keeps the program, the source and the types | Task 8 adds `Checked::explanation` |
| F8 | `cli::run` refuses `explain` as "not built yet" (`src/cli.rs:63`), and `tests/cli.rs:103` pins that | Task 9 changes both |
| F9 | `FeatureInfo` (`src/check/mod.rs:61`) has `inputs: Vec<(String, Ty)>`, `vars: Vec<(String, Ty, u32)>` and `loops: Vec<LoopVar>`; `Ty::describe` gives `number`, `list of expense` | Task 6's inferred list |
| F10 | `Source::line(n)` returns line `n` as written (`src/source.rs:39`) | Line headers print it trimmed |

## File map

| File | Responsibility | Task |
|---|---|---|
| `src/target/x86_64/program.rs` | `Assembly`, `assemble`; `executable` over it | 1 |
| `src/target/listing.rs`, `src/target/mod.rs` | `Routine`, `Item`, `Ref`; `Target::listing` | 2 |
| `src/target/x86_64/listing.rs` | Assembly to listing: items, label renaming, refs | 2, 3 |
| `src/target/x86_64/linux.rs` | `syscall_name` | 3 |
| `src/lower/notes.rs`, `src/lower/mod.rs`, `src/lower/fail.rs` | `Notes`, `lower_explained` | 4, 5 |
| `src/explain/mod.rs`, `src/explain/comment.rs` | The listing text | 6, 7, 8 |
| `src/lib.rs` | `Checked::explanation` | 8 |
| `src/cli.rs`, `tests/cli.rs` | `gloss explain` | 9 |
| `tests/explain_cases.rs`, `tests/explain_cases/*` | Golden cases, invariants | 10, 11 |
| `spec/v0.md`, `tests/spec_examples.rs`, `README.md`, design spec | Docs | 12, 13 |

---

### Task 1: `assemble` (`m6/t01-assemble`) - risky

**Files:** Modify `src/target/x86_64/program.rs`. Test: the same file's tests module, or `src/target/x86_64/tests.rs`.

**Interfaces:**
- Produces:
  ```rust
  /// One function or helper of the executable, in layout order.
  pub struct Assembled { pub name: String, pub code: Vec<X> }
  pub struct Assembly { pub routines: Vec<Assembled> }
  pub fn assemble(m: &Module) -> Result<Assembly, String>;
  ```
  `executable(m)` = `assemble(m)?`, then the existing `encode` over every routine's `code` concatenated in order, then rodata, globals and `write` as today.
- `arith::helpers()` returns one flat `Vec<X>`. Split it into routines at each `X::Label` whose name starts with `arith::PREFIX` (`x86_64.multiply_number`, `x86_64.divide_number`). Each routine's `name` is that label's text.

- [ ] **Step 1: write the failing tests.**
  - `assembly_lists_every_function_then_the_helpers`: for a module from `lower` of a small program (`show 1 + 2`), the routine names equal the module's function names in order, followed by `x86_64.multiply_number` and `x86_64.divide_number`.
  - `assembly_concatenated_is_what_executable_encodes`: `encode` of the concatenated code equals the `.text` bytes inside `executable`'s output. Compare the whole ELF: build it from the concatenation with the same rodata and globals steps, through a private helper both paths share.
  - `each_routine_starts_with_its_own_label`: the first `X` of every routine is `X::Label(name)`.
- [ ] **Step 2:** run `cargo test assembly` and see the failures (the functions don't exist yet).
- [ ] **Step 3:** implement as in Interfaces. Keep the reserved-prefix check in `assemble`.
- [ ] **Step 4:** run the verify gate, plus the Linux-only tests (`$S/m5-linux-tests.sh` style: cross-compile for musl and run the test executables through `orb -m gloss-amd64`). Every executable test must pass unchanged.
- [ ] **Step 5:** commit: `Split assembling from encoding so Explain can print what is encoded`.

**Fail-proof:** a wrong `assemble` that reorders the helpers, or drops one, still lets `executable` pass if `executable` doesn't use it. The second test forces one shared path.
**Review:** opus first, then sonnet. **Estimate:** ~90 lines (×1.4 = 126).

### Task 2: The listing types and items (`m6/t02-listing-items`)

**Files:** Create `src/target/listing.rs` and `src/target/x86_64/listing.rs`. Modify `src/target/mod.rs` and `src/target/x86_64/mod.rs`.

**Interfaces:**
- Produces (in `src/target/listing.rs`, re-exported as `crate::target::listing`):
  ```rust
  pub struct Routine { pub name: String, pub items: Vec<Item> }
  pub enum Item {
      /// A `Line` marker: the code after it belongs to source line N.
      Line(u32),
      /// A label; `local` is the IR label number of the function's own label, printed `.L<n>`.
      Label { text: String, local: Option<u32> },
      Inst { text: String, refs: Vec<Ref> },
  }
  pub enum Ref { Slot(u32), Input(u32), Text(u32), Jump(u32), System(&'static str) }
  ```
  All four derive `Clone, Debug, PartialEq, Eq`.
- `Target` gains `fn listing(&self, m: &Module) -> Result<Vec<Routine>, String>;`. `X86_64Linux` implements it with `x86_64::listing::listing(&assemble(m)?, m)`.
- In this task every `Inst` has `refs: vec![]`. Task 3 fills them.

Rules:
- `X::Comment("line N")` becomes `Item::Line(N)`. No other `Comment` exists today; any other comment is an error (`"unexpected comment …"`).
- `X::Label(l)` becomes `Item::Label`. When `l` is `L<routine name>_<n>`, the item is `{ text: ".L<n>", local: Some(n) }`. Otherwise it is `{ text: l, local: None }`.
- The routine's own first label is not an item: the routine has a name.
- Every other `X` becomes `Item::Inst { text: inst::text(x) }`, except that a jump or call operand of the form `L<routine name>_<n>` is rewritten to `.L<n>`.

- [ ] **Step 1: failing tests** in `src/target/x86_64/listing.rs`:
  - `the_listing_has_one_instruction_per_assembled_instruction`: for the module of a program with an `if` and a `for each`, walk every routine. The `Inst` texts, with `.L<n>` mapped back to `L<name>_<n>`, equal `inst::text` of every non-label, non-comment `X` of the assembly, in order. This is the D57 listing invariant.
  - `line_markers_and_local_labels`: a hand-made `Function` with `Line(3)`, `Mark(Label(1))`, `Jump(Label(1))` lists `Line(3)`, `Label { text: ".L1", local: Some(1) }` and `Inst { text: "jmp .L1" }`.
  - `helper_labels_stay_as_they_are`: a label inside `x86_64.divide_number` that isn't `L<name>_<n>` keeps its text with `local: None`.
- [ ] Steps 2 to 5 as in Task 1, without the Linux run. Commit: `List the assembly as routines of lines, labels and instructions`.

**Fail-proof:** a listing that skipped `Push`/`Pop` would still read well. The first test counts every instruction.
**Review:** sonnet. **Estimate:** ~140 lines (×1.4 = 196). If it runs over, split off the `Target::listing` wiring as `m6/t02b-listing-target`.

### Task 3: References (`m6/t03-listing-refs`)

**Files:** Modify `src/target/x86_64/listing.rs` and `src/target/x86_64/linux.rs`.

**Interfaces:**
- Produces `pub fn syscall_name(n: i64) -> Option<&'static str>` in `linux.rs`. It covers every number `system` uses: `read`, `write`, `mmap`, `getrlimit`, `exit_group`, `close`, `fsync`, `fstat`, `fork`, `wait4`, `rt_sigaction`, `fchmod`, `renameat`, `openat2`.
- `listing` now needs each IR `Function`'s `slots` and `params`, taken from `m.functions` by name. Helpers have neither.

Refs on an `Inst`, in operand order:
- `Mem::Base(Rbp, d)` with `d < 0`, `d % 8 == 0` and `1 <= -d/8 <= slots`: `Slot(-d/8)`.
- `Mem::Base(Rbp, d)` with `d >= 16` and `(d-16) % 8 == 0` and `k = params - (d-16)/8` in `1..=params`: `Input(k)`.
- `Mem::Rip(Sym::Text(k))`: `Text(k)`.
- A `Jcc` or `Jmp` to `L<name>_<n>`: `Jump(n)`.
- `X::Syscall`: `System(name)`, from the last `X::MovRI(R::Rax, N)` earlier in the same routine, named by `syscall_name(N)`. An unnamed number is an error (`"syscall … has no name"`), so a new primitive cannot slip in unnamed.

- [ ] **Step 1: failing tests:**
  - `slot_and_input_refs`: a function with 2 params and 3 slots. `LoadSlot(A, 2)` gives `Slot(2)`. `LoadArg(A, 1)` gives `Input(1)`. The saved-`b` load in `Return` (`[rbp-32]`) gives no ref.
  - `text_and_jump_refs`: `AddressOf(A, Data::Text(4))` gives `Text(4)`. `JumpIf(Eq, Label(2))` gives `Jump(2)`.
  - `every_syscall_is_named`: list the module of a program that loads and saves a table and asks for input. Every `syscall` instruction has exactly one `System` ref, and the names seen include `write`, `read`, `openat2`, `renameat` and `exit_group`.
  - `syscall_names_cover_system`: every constant `system` emits into `rax` has a name. Build one routine per `Prim` variant through `linux::system`, list it, and expect no error.
- [ ] Steps 2 to 5. Commit: `Give listed instructions their slot, input, text, jump and system call references`.

**Fail-proof:** a rule that read `[rbp-8(slots+1)]` as a slot would name the saved `b` word. The first test pins that it has no ref.
**Review:** sonnet. **Estimate:** ~130 lines (×1.4 = 182).

### Task 4: Notes: slots and inputs (`m6/t04-notes`)

**Files:** Create `src/lower/notes.rs`. Modify `src/lower/mod.rs`.

**Interfaces:**
- Produces:
  ```rust
  #[derive(Default, Debug, PartialEq)]
  pub struct Notes { pub functions: BTreeMap<String, FunctionNotes> }   // key: IR function name (`f_<feature>`)
  #[derive(Default, Debug, PartialEq)]
  pub struct FunctionNotes {
      pub feature: String,
      pub line: u32,                      // the line of the feature's header
      pub slots: BTreeMap<u32, String>,   // named slots only, from `Lowerer::slots`
      pub inputs: Vec<String>,            // in order; input k is inputs[k-1]
      pub fails: BTreeMap<u32, FailNote>, // filled by Task 5
  }
  #[derive(Debug, PartialEq)]
  pub struct FailNote { pub code: i64, pub line: u32, pub message: String }
  pub fn lower_explained(program: &Program, types: &TypeInfo, src: &Source) -> Result<(Module, Notes), Diagnostic>;
  ```
  `lower` becomes `lower_explained(..).map(|(m, _)| m)`. Test and trace lowering keep notes off: `lower_program` takes `Option<&mut Notes>`.
- Two slot names for one slot (a loop variable sharing a variable's slot, F5) keep the first name.

- [ ] **Step 1: failing test** `notes_name_every_slot_and_input`. For
  ```
  to total_of with prices
    set total to 0
    for each price in prices
      add price to total
    give total
  ```
  plus a `start` that calls it, `notes.functions["f_total_of"]` has `slots` = `{1: "total", 2: "price"}`, `inputs` = `["prices"]` and empty `fails`. `notes.functions["f_start"]` also exists.
- [ ] **Step 2: failing test** `lower_and_lower_explained_make_the_same_module`. For every `examples/*.gloss`, `ir::module_text` of both modules is equal.
- [ ] Steps 3 to 5. Commit: `Record each feature's slot and input names while lowering`.

**Review:** sonnet. **Estimate:** ~110 lines (×1.4 = 154).

### Task 5: Notes: failures (`m6/t05-fail-notes`)

**Files:** Modify `src/lower/fail.rs`, `src/lower/notes.rs` and `src/lower/mod.rs`.

**Interfaces:**
- `Fail::note(&self) -> FailNote` builds the message the program would write, with the pieces known only at run time as `…`:
  - plain: `parts[0] + parts[1] + parts[2]`, for example `a number went out of range (price * 1000000)`;
  - with `value`: `parts[0] + parts[1] + " is …" + parts[2]`, for example `an item position must be whole (n is …)`;
  - with `shown`: `"\"…" + parts[0] + parts[1] + parts[2]`, for example `"…" is not a number (answer as number)`;
  - with `item`: `there is no item … in <parts[1]>; it has … items`.
  - An `expect` failure never occurs in a real build. If one does, `lower_explained` panics with `"an expect in a real build"`.
- `finish` records `fails[label.0] = f.note()` into the function's notes, when notes are on, before `emit_fails` drains them.

- [ ] **Step 1: failing test** `failure_notes_say_the_message`. For
  ```
  to start
    set xs to list of 1, 2
    set n to 3
    set t to "x"
    set a to 9 / n
    show item n of xs
    show t as number
    show n * 1000000000000
  ```
  the `f_start` notes' failures, sorted by label, have exactly these `(code, message)` pairs:
  - `(1, "dividing by zero (n is 0)")`
  - `(3, "an item position must be whole (n is …)")`
  - `(2, "there is no item … in xs; it has … items")`
  - `(5, "\"…\" is not a number (t as number)")`
  - `(4, "a number went out of range (n * 1000000000000)")`

  The test asserts the set and each pair's line. If the program lowers more failure labels than these (for example failure 4 on `9 / n`), the expected list grows; record that in the report.
- [ ] Steps 2 to 5. Commit: `Note what each failure block reports`.

**Review:** sonnet. **Estimate:** ~90 lines (×1.4 = 126).

### Task 6: The listing text: features and lines (`m6/t06-explain-lines`)

**Files:** Create `src/explain/mod.rs`. Modify `src/lib.rs` (`mod explain`).

**Interfaces:**
- Produces `pub fn listing(name: &str, src: &Source, types: &TypeInfo, notes: &Notes, routines: &[Routine], texts: &[Vec<u8>]) -> String`. `name` is the file name, as in `x.gloss`.
- Output, in order:
  1. `Explain x.gloss for x86-64 Linux`, then a blank line.
  2. For each routine named in `notes.functions` (in routine order), the header `feature NAME (line N)`. `N` is `FunctionNotes::line`.
  3. Under the header, each input `  rows: list of expense, input 1, at [rbp+24]`, each variable `  total: number, created on line 13, at [rbp-8]`, and each loop variable `  r: expense, loop variable on line 14, at [rbp-16]`, in `FeatureInfo` order. A name already listed (F5's shared slot) is not listed twice. Then a blank line.
  4. Then the items: `Line(N)` prints `line N: <src.line(N) trimmed>`. When `N` was already printed in this routine, and a different line came in between, it prints `line N: <text> (continued)` instead. A repeat with nothing in between prints nothing. `Label` prints `  <text>:`. `Inst` prints `    <text>`.
  5. A blank line between features.
- In this task the runtime section and the comments are out of scope: non-feature routines are skipped.

- [ ] **Step 1: failing tests** (on a lowered small program, through `lower_explained` and `x86_64_linux().listing`):
  - `a_feature_starts_with_its_inferred_names`: the exact header and names block for `total_of` from Task 4's program.
  - `lines_repeat_as_continued`: for a `for each` loop, the loop's line appears once plain, and again with ` (continued)` after the body's line.
  - `labels_and_instructions_are_indented`: labels have 2 spaces before them and instructions 4.
- [ ] Steps 2 to 5. Commit: `Print each feature's inferred names, lines and instructions`.

**Review:** sonnet. **Estimate:** ~150 lines (×1.4 = 210, so pre-split: 6a header and names block, 6b items, if 6 runs past 200).

### Task 7: Comments and failure headers (`m6/t07-explain-comments`)

**Files:** Create `src/explain/comment.rs`. Modify `src/explain/mod.rs`.

**Interfaces:**
- `fn comment(refs: &[Ref], notes: Option<&FunctionNotes>, texts: &[Vec<u8>]) -> Option<String>` joins the parts with `, `:
  - `Slot(k)`: the name in `notes.slots`; nothing without a note.
  - `Input(k)`: `notes.inputs[k-1]`.
  - `Text(k)`: the text as a quoted Gloss text. Each `"` is doubled, each `chars::unshowable` character is spelled `\u{XXXX}` (4 or more upper-case hex digits), invalid UTF-8 uses lossy decoding, and the text is cut after 30 characters, ending `…"`.
  - `Jump(n)` with `notes.fails[n]`: `failure C: <message>`.
  - `System(name)`: `name`.
- An instruction with a comment prints `    <text>` padded with spaces to column 36 (counting from 0), then `; <comment>`. When `    <text>` is 36 characters or longer, it prints one space and then `; <comment>`.
- A `Label { local: Some(n) }` with `notes.fails[n]` prints the header `failure C, line N` (no indent) before the label line.

- [ ] **Step 1: failing tests:**
  - Unit tests of `comment` for each ref kind, including text cut at exactly 30 and 31 characters, a doubled quote, and an invisible character.
  - `an_overflow_jump_names_its_failure`: in the listing of `add price to total`, the `jo` line ends `; failure 4: a number went out of range (price + total)` (or the source as lowering writes it; assert against the note).
  - `a_failure_block_has_a_header`: the block begins `failure 4, line 4`.
  - `the_comment_column`: a commented instruction has `;` at index 36.
- [ ] Steps 2 to 5. Commit: `Comment slots, texts, failures and system calls in the listing`.

**Review:** sonnet. **Estimate:** ~150 lines (×1.4 = 210, pre-split 7a `comment` with its unit tests, 7b layout and failure headers if needed).

### Task 8: The runtime section and `Checked::explanation` (`m6/t08-explain-runtime`)

**Files:** Modify `src/explain/mod.rs` and `src/lib.rs`.

**Interfaces:**
- After the features: a blank line, `runtime`, a blank line, then every routine not in `notes`, in order. Each one is `routine NAME`, its items (no inferred block; comments from refs with `notes = None`, so only texts and system calls), and a blank line between routines.
- `Checked::explanation(&self, name: &str) -> Result<String, Diagnostic>`: `lower_explained`, then `target::x86_64_linux().listing(&module)` (an error becomes `Diagnostic::program`), then `explain::listing`.

- [ ] **Step 1: failing tests:**
  - `every_routine_is_listed`: for each `examples/*.gloss`, the `routine NAME` lines plus the `feature` headers cover every routine name of `assemble` exactly once. This is the D57 completeness invariant.
  - `runtime_routines_have_no_slot_comments`: no line under `runtime` has a comment other than a quoted text or a system call name.
- [ ] Steps 2 to 5. Commit: `List every runtime routine after the program`.

**Review:** sonnet. **Estimate:** ~100 lines (×1.4 = 140).

### Task 9: `gloss explain` (`m6/t09-explain-command`)

**Files:** Modify `src/cli.rs` and `tests/cli.rs`.

- `"explain" => 0` in the milestone table. `explain(name, text, out, err)` does the following:
  1. Print the diagnostics as `test` does (sorted, on `err`). Any error gives status 3.
  2. Write `checked.explanation(name)` to `out`. A lowering error is rendered on `err` with status 3.
  3. A write error (or a failed flush) prints `cannot write the explanation` on `err` and gives status 3.
  4. Otherwise status 0.
- `--allow` with `explain` is a usage error: `ALLOW_COMMANDS` stays `["build", "trace"]`. The usage text stays the same.

- [ ] **Step 1: failing tests:**
  - In `tests/cli.rs`, replace the "not built yet" assertion for `explain` (keep `flowchart`'s).
  - `explain_prints_the_listing_and_exits_0`: the output starts `Explain x.gloss for x86-64 Linux\n\nfeature start (line 1)`, contains `runtime\n`, and nothing is written beside the source (the folder holds only `x.gloss`).
  - `explain_of_a_program_with_errors_exits_3`.
  - `explain_with_allow_is_a_usage_error`.
  - `explain_of_a_table_program_asks_nothing`: a program that loads a table explains with status 0, with a prompt that panics if asked.
  - A unit test in `src/cli.rs`: a writer that fails gives `cannot write the explanation` and 3.
- [ ] Steps 2 to 5. Commit: `Add gloss explain`.

**Review:** sonnet. **Estimate:** ~120 lines (×1.4 = 168).

### Task 10: Explain cases (`m6/t10-explain-cases`)

**Files:** Create `tests/explain_cases.rs`, `tests/explain_cases/{shows_text,add_overflow,loop_continued}.{gloss,out}` and `tests/explain_sweep.inc` (or the sweep inline).

- The harness runs `gloss::check_program`, then `explanation("<stem>.gloss")`, keeps the text up to and including the `runtime` line, then the names from each `routine NAME` line, one per line. It compares the result with `<stem>.out` and prints the first differing line.
- The sweep has `MIN_EXPLAIN_CASES = 3`, and a test fails on any file in the folder that is not part of a case.
- `GLOSS_BLESS=1` rewrites the `.out` files. Look for an existing bless convention first (`tests/test_cases.rs`, `tests/run.rs`) and follow it. If there is none, record a ruling.
- Every `.out` is read by a person before it is committed. The report quotes `add_overflow.out` in full.

**Review:** sonnet. **Estimate:** harness ~80 lines plus cases ~90 lines (×1.4 = 238). Pre-split: 10a harness plus `shows_text`; 10b the other two.

### Task 11: More cases (`m6/t11-more-cases`)

**Files:** `tests/explain_cases/{feature_inputs,item_failure,divide,if_otherwise}.{gloss,out}`, and the sweep floor.
- These cover a call with inputs (input refs, `feature` blocks for two features), failure 2 with `…`, failure 1, and `if`/`otherwise if`/`otherwise` labels. `MIN_EXPLAIN_CASES = 7`.

**Review:** sonnet. **Estimate:** ~150 lines (×1.4 = 210; split into 11a and 11b at two cases each if needed).

### Task 12: The language spec (`m6/t12-spec`)

**Files:** `spec/v0.md`, `tests/spec_examples.rs`, `tests/explain_cases/spec_explain.{gloss,out}`.
- §10 Explain: replace the example with the program part of `spec_explain.out` for a program whose body includes `add price to total` (keep it under about 25 lines: one feature, its names block and that line's code). Document the following:
  - the header line;
  - the names block;
  - `(continued)`;
  - failure headers;
  - the comment kinds;
  - `.L<n>` labels;
  - the `runtime` section.
- §11 gets the `gloss explain` statuses and `cannot write the explanation`.
- Add a row to the design-changes table: "Explain's example showed idealised code | The example is real output | §10" (D58).
- The test `the_spec_explain_example_is_an_explain_case` checks that the §10 block is a contiguous run of lines of `spec_explain.out`.

**Review:** sonnet. **Estimate:** ~120 lines (×1.4 = 168).

### Task 13: As built (`m6/t13-as-built`)

**Files:** `README.md` (milestone 6 Done; an Explain section with a short example; the command table row), design spec §6 "As built - Milestone 6" (rulings, deviations, tests, known limits).
**Review:** sonnet. **Estimate:** ~100 lines.

---

## Rulings made while planning

- P1: notes travel beside the module (`lower_explained`), not inside `ir::Function`, because 168 places build a `Function`. Cost if wrong: if a later mode also needs notes, it goes through the same function.
- P2: the listing is target-neutral, and refs are computed by the target (D18). Cost if wrong: a second target writes its own `listing`, which it would need anyway.
- P3: system call names come from the last `mov rax, N` in the routine (F4), and a number with no name is an error. Cost if wrong: a future primitive that computes `rax` at run time fails the listing loudly and gets a ref of its own.
- P4: golden files cover the program part and the routine names only (D57). The bodies are covered by the listing invariant (Task 2) and the completeness invariant (Task 8).

## Self-review

- Spec coverage. Each decision and section maps to a task:
  - D51: Tasks 1 and 2.
  - D52: Task 6 for lines and `(continued)`, Task 7 for failure headers.
  - D53: Tasks 3 and 7.
  - D54: Task 6.
  - D55: Tasks 2 and 8.
  - D56: Task 9.
  - D57: Tasks 2, 8, 10 and 11.
  - D58: Task 12.
  - §4.10 CLI pipe safety: Task 9's failing-writer test.
- Types are used the same way across tasks: `Routine`, `Item` and `Ref` (Task 2) in 3, 6, 7 and 8; `Notes`, `FunctionNotes` and `FailNote` (Task 4) in 5, 6, 7 and 8; `Checked::explanation` (Task 8) in 9 and 10.
