# Gloss v0 compiler - design spec

- **Date:** 2026-11-22
- **Status:** built (milestones 1 and 2, section 6); milestone 3 designed (§4.7); milestones 4–7 not started
- **Context:** Implements the language defined in `spec/v0.md` (the *language spec*). This document decides *how* the compiler is built. Where the two disagree about the language, the language spec wins. Where they disagree about the implementation, this document wins.
- **Plans:** `docs/plans/2026-11-22-compiler-m1-front-end.md` (milestone 1). Milestone 2: `docs/plans/2026-11-23-compiler-m2-codegen.md`. Milestone 3: `docs/plans/2026-11-24-compiler-m3-tables-files.md`. Later milestones get their own plans.

## 1. Goals and non-goals

Goals:
1. A single `gloss` binary that implements every mode in section 10 of the language spec: build, test, format, trace, explain and flowchart.
2. The front end is complete before code generation starts. It covers every compile error and warning the language spec names, with the exact message forms.
3. Every example in the language spec is checked by the compiler's own tests, so the spec and the compiler cannot drift apart silently.

Non-goals (and why):
- **Optimisation.** The language spec forbids it in v0, because it would make Explain unreadable.
- **Error recovery in the parser.** The first parse error stops parsing; type and flow checks then report every error they find. Recovery is complex and not needed for programs this size.
- **An interpreter.** It was removed in the second review: there is one execution engine.
- **Platforms beyond x86-64 Linux** for the executables Gloss produces. The compiler itself runs anywhere Rust does.

## 2. Decisions

| Id | Question | Decision | Why |
|---|---|---|---|
| D1 | Which language is the compiler written in? | Rust, 2024 edition, stable toolchain (1.87 or later) | Sum types and pattern matching suit the AST, the checker and the instruction encoder. Byte buffers are easy, and it builds one binary. Chosen by the owner, 2026-11-22. |
| D2 | How are produced executables run on the arm64 Mac? | *Machine superseded by D17.* An OrbStack Linux machine named `gloss-amd64` (Ubuntu, `--arch amd64`, run through Rosetta). The test harness prefixes executions with the command in `GLOSS_RUNNER`, set to `orb -m gloss-amd64` on the Mac and left empty on Linux. | The target stays x86-64 Linux without changing the language spec. Chosen by the owner, 2026-11-22. |
| D3 | Dependencies? | None, in both `[dependencies]` and `[dev-dependencies]`. Only the standard library. | Rule 6 is about Gloss programs, but a compiler with no dependencies is easier to audit and keeps the build fast. Test harnesses are small enough to write by hand. |
| D4 | Crate layout? | One crate, `gloss`, holding a library (`src/lib.rs`) and a binary (`src/main.rs`), with one module per stage | It's simple, and the stages share types freely. Split into a workspace only if compile times hurt. |
| D5 | Milestones? | M1 front end (lexer, parser, checker, format, CLI for `build` and `format`). M2 code generation and runtime (numbers, text, show, ask, control flow, features, lists, records). M3 tables and files (CSV, load, save, clear, permissions, `.allow`, symbolic links, safe writing). M4 `gloss test` (plain tests, then fixtures). M5 Trace. M6 Explain. M7 Flowchart. | This follows the build order in section 12 of the language spec. Each milestone ends with working, testable software. |
| D6 | What does `gloss build` do before M2? | It runs every check and prints `Checked <file>: no errors (code generation arrives in milestone 2)`, exiting 0, or prints the diagnostics and exits 3. It writes no executable. | Lets the front end ship and be used on its own. Replaced in M2. |
| D7 | Diagnostic text form | `line N: <source line>. <problem>.`, then ` Did you mean <form>?` when there is a suggestion. Warnings put `warning: ` before the source line. Diagnostics about the whole program, with no line, print as `<problem>.` | This matches the language spec's example exactly: `line 7: set total as 0. set needs to. Did you mean set total to 0?` |
| D8 | Diagnostic order | Sorted by line, errors before warnings on the same line. Printed to standard error. | Predictable for agents and tests. |
| D9 | Parser technique | A hand-written recursive-descent parser that follows Appendix A of the language spec level by level | The grammar is small, and hand-written code gives exact error messages. |
| D10 | Expression identity | Every `Expr` carries an `ExprId(u32)` assigned by the parser in source order. The checker records each expression's type in a `Vec<Ty>` indexed by that id. | Code generation and Explain need types without re-checking. |
| D11 | Comments in the AST | Notes are kept as AST nodes, both as statements and as top-level items | `gloss format` must preserve them. |
| D12 | Testing | Unit tests sit beside each module and assert the exact rendered diagnostic strings. `tests/spec_examples.rs` extracts every `gloss` code block from `spec/v0.md` and requires each one to parse, check and format as its kind demands. `tests/cli.rs` runs the real binary end to end. | Exact strings pin the messages, which are part of the language design. The spec guard catches spec examples that stop compiling. |
| D13 | Sweep guards | Every case-file harness asserts a minimum case count, hard-coded and raised by each task that adds cases | An empty or misnamed directory cannot pass. |
| D14 | Pull requests | At most 200 changed lines and 10 files each. Opened as drafts and marked ready by the owner. Merged by rebase. CI skips drafts. | The owner's global rules. |
| D15 | Verify gate | `cargo fmt --check && cargo clippy --all-targets -- -D warnings && cargo test` | One command, run before every PR is marked ready |
| D16 | Exit status for wrong usage of `gloss` itself (unknown command, missing file) | 3, with a usage message | The language spec defines only statuses 0 to 4, and a usage mistake is closest to a compile error |
| D17 | Which Linux runs the executables on the Mac? | An OrbStack **Alpine** machine, `orb create --arch amd64 alpine gloss-amd64` (about 1 MB). Supersedes D2's Ubuntu. | The executables are static and need nothing from the distribution, so the smallest one works. Ubuntu takes far more space; Arch would not boot under x86-64 emulation. Owner, 2026-11-23. |
| D18 | How is code generated? | Through a target-neutral **Gloss IR** (section 4.6). `lower` turns the AST into IR; a backend for each CPU turns IR into that CPU's instructions; an encoder and an object writer produce the file. | The compiler must later produce executables for other CPUs and OSes. Only the backend, encoder, object writer and OS primitives are per target. Owner, 2026-11-23. |
| D19 | Where is the runtime written? | Once, in Gloss IR. Only the OS primitives (write, read, get memory, exit, stack size, and in M3 open and rename) differ per target. | A new target must not mean rewriting number printing, 128-bit arithmetic, lists, copying and equality by hand |
| D20 | Which targets does v0 ship? | x86-64 Linux only, static ELF, direct system calls. The `Target` interface leaves room for arm64 and for macOS and Windows, which import their OS primitives from `libSystem` and `kernel32` as declared borrowed code. | The language spec's v0 target is unchanged |
| D21 | How is the runtime tested without Linux? | `irsim`, a test-only IR interpreter. It is never shipped and is not a second execution engine; the real executables stay the authority, and both must agree on every `tests/run` case. | Fast checks on any host, and a reference comparison against the Rust `number` module |
| D22 | What does M2's `gloss build` do with a program that declares a table? | It refuses: `tables arrive in milestone 3`, exit status 3, no executable written | No throwaway in-memory tables. Owner, 2026-11-23. |
| D23 | In what order is milestone 3 built? | One milestone, one plan: build-time approval first (permission list, `.allow`, `--allow`, the question, status 4, `--permissions`), then files at run time. The D22 refusal is lifted when tables first live in memory, after approval is in; until each statement's task lands, `load` and `save` are refused as statements this compiler cannot build yet. | `main` never builds an executable that touches files without approval. Owner, 2026-11-24. |
| D24 | How does the permission list show a file name? | A name of printable ASCII prints as it is. Any other name prints quoted, followed by the same name with every non-ASCII character as `\u{XXXX}` in parentheses: `"résumé.csv" (r\u{E9}sum\u{E9}.csv)`. The same form is used at build, in `.allow` and by `--permissions`. | Lookalike letters and non-normalised forms cannot pass as another name, and names in other languages stay readable. Owner, 2026-11-24. |
| D25 | What does a build that needs approval do when standard input is not a terminal? | It prints the list, then `Not approved: <files>. Build in a terminal to answer, or add --allow <file>.`, writes no executable and exits 4. It never reads an answer from a pipe. | An agent or script must approve with `--allow`, which shows in its command; piping `yes` in would weaken what an answer means. Owner, 2026-11-24. |
| D26 | Wording of run-time failures 6–8 | As in §4.7, one message per cause, naming the file and, for bad data, the file line where the row starts | Owner, 2026-11-24 |
| D27 | Which builds print the permission list? | Only programs that declare a table. The language spec says every build; a program with no tables would print only `nothing else`, so it prints `Built x` as in M2. `--permissions` works on every executable. | Avoids noise on every build. Deviation from spec §8, accepted by the owner, 2026-11-24. |
| D28 | What does `.allow` hold? | Exactly the current program's list after every successful build, written atomically and only when it changes. A file the program no longer uses, or no longer saves, loses its approval. | The safer direction: adding access back needs a new approval, and every change shows in a diff. Owner, 2026-11-24. |
| D29 | How are files tested? | Run cases carry a `<name>.files/` folder copied in before the run and a `<name>.after/` folder that must match the folder afterwards exactly; symbolic links come from `<name>.links`. `irsim` gets an in-memory file system seeded from the same folder, so file cases are differential too (D21). | One case format for real runs and `irsim`; links are not committed to git. |

## 3. Rejected alternatives

| Alternative | Rejected because | Revisit if |
|---|---|---|
| Go, Zig | Owner chose Rust (D1) | n/a |
| x86-64 instructions built directly in Rust, runtime written in x86-64 (no IR) | Every new target would rewrite the whole runtime (D18, D19) | Multiple targets are dropped for good |
| Runtime as `.s` assembly text plus an assembler inside the compiler | About 400 more lines, no type checking of runtime code, and still tied to one CPU | n/a |
| Emitting bytes directly, with no instruction type | Explain would need a disassembler | n/a |
| Ubuntu or Arch for the Linux machine | Ubuntu is much larger; Arch would not start under emulation (D17) | n/a |
| AArch64 Linux target | It would mean rewriting the language spec's Explain and register model | The x86 workflow through Rosetta proves too slow |
| GitHub Actions as the only place to run executables | It spends limited Actions minutes and gives slow feedback | OrbStack cannot be installed |
| A parser generator, or `logos`/`chumsky` | Adds dependencies (D3) and makes exact messages harder | n/a |
| A snapshot-testing crate (`insta`) | Adds a dependency (D3); `.expected` files do the same job | n/a |
| Error recovery in the parser | Complex, and the first error is enough at this program size | Users routinely hit several parse errors at once |

## 4. Design

### 4.1 Modules

| Module | Responsibility | Milestone |
|---|---|---|
| `diag` | `Diagnostic`, `Severity`, rendering (D7), sorting (D8) | M1 |
| `source` | `Source`: the text split into lines, so diagnostics can quote the line | M1 |
| `number` | `Number`, the fixed-point value: literal parsing, text parsing (`as number`), show form, `with N places` | M1 (also used by M2's constant data) |
| `lexer` | Tokens, keywords, indentation, continuation, notes | M1 |
| `ast` | The syntax tree, plus `sexpr` debug printing for tests | M1 |
| `parser` | Tokens to `Program`; the first error stops parsing | M1 |
| `check` | Names, types, flow and mutation rules; produces `TypeInfo` | M1 |
| `format` | Canonical printing and the rule-2 rewrites | M1 |
| `cli` | Argument handling, exit statuses | M1, extended later |
| `ir` | The Gloss IR: instructions, labels, functions, read-only and read-write data, and a text form | M2 |
| `lower` | AST + `TypeInfo` to IR for the user's features: frames, slots, one value register | M2 |
| `runtime` | Runtime routines written in IR, and the interface to the OS primitives | M2, M3 |
| `target` | The `Target` trait: backend, encoder, object writer, OS primitives | M2 |
| `target::x86_64` | IR to typed x86-64 instructions, their Intel-syntax text, and the encoder | M2 |
| `target::elf`, `target::linux` | Static ELF writer; OS primitives as x86-64 Linux system calls | M2 |
| `irsim` | Test-only IR interpreter (D21) | M2 |
| `trace`, `explain`, `flowchart` | The remaining modes | M5, M6, M7 |

### 4.2 Pipeline

`source::Source::new(text)` → `lexer::lex(&Source) -> Result<Vec<Token>, Diagnostic>` → `parser::parse(&[Token], &Source) -> Result<Program, Diagnostic>` → `check::check(&Program, &Source) -> CheckResult`. `CheckResult` holds the diagnostics and the types; when there are errors, the types are `None`. In M2, `codegen` takes `&Program` and `&TypeInfo`.

`gloss format` stops after parsing. It formats any program that parses, even one with check errors, because two of the check errors are exactly what Format fixes.

### 4.3 Types inside the checker

```rust
pub enum Ty { Number, Text, Truth, Record(String), List(Box<Ty>), Table(String), Nothing }
```

`Table(name)` behaves as `List(Record(..))` wherever the language spec says tables behave like lists. `Nothing` is the result of a feature with no `give`.

### 4.4 Running produced executables (M2 onward)

- **On the Mac:** `brew install --cask orbstack`, `orb start`, then `orb create --arch amd64 alpine gloss-amd64` (D17). OrbStack shares `/Users` with the machine, so `orb -m gloss-amd64 /Users/.../program` runs a binary built on the Mac.
- **The harness** reads `GLOSS_RUNNER` and splits it on spaces to form the command prefix. Output capture (the program writes stdout and stderr to files in its shared folder, read back after it exits, because `orb` can drop the tail of piped output) applies when the runner's first word is `orb`; a wrapped runner such as `env orb ...` set via `GLOSS_RUNNER` is not covered.
- **CI** runs on `ubuntu-latest` (x86-64) with `GLOSS_RUNNER` empty.

### 4.5 Security

Table paths are the only security boundary in M1. The checker rejects absolute paths, `..`, empty parts, backslashes, and the same file declared by two tables. The run-time half of the boundary (symbolic links, `openat2`) is M3. Tasks that touch the boundary get a first review on the top tier.

### 4.6 Milestone 2: code generation and runtime

**Scope.** `gloss build x.gloss` writes the executable `x` beside the source, atomically and with mode 0755, and prints `Built x`. That covers every program without tables: numbers, text and truths, `show` and `ask`, `if`, the three loops, `skip` and `stop`, features, inputs, `give`, `run` and recursion, lists and records with copies, equality, show forms and every conversion, and run-time failures 1–5, 9 and 10. Tables are refused (D22).

**Pipeline.** check → `lower` (IR for the program and the reachable runtime routines) → backend (x86-64 instructions) → encoder → ELF. Only reachable routines are included.

**The IR machine.**
- Every value is 8 bytes. Numbers are counts of ten-thousandths, truths are 1 or 0, and text, lists and records are block addresses.
- There are two value registers: `a` holds each expression's result and `b` the second operand. Intermediate values go on the stack with push and pop, and each variable has a numbered frame slot. Runtime routines use slots for their locals the same way.
- x86-64 maps `a`→`rax`, `b`→`rcx` and slot k→`[rbp-8k]`.
- There are about 35 instructions:
  - moves, push and pop, and 8-byte and byte memory access
  - `add`, `sub` and `neg` that check the range
  - `mul_round` and `div_round`, with a 128-bit intermediate and rounding half away from zero
  - `rem`, `whole`, and plain integer arithmetic
  - compare, conditional jump and set; jump, labels, `call`, `return` and `enter`
  - `system <primitive>`
  - `line N` markers
- Calls push arguments left to right, the result is returned in `a`, and the caller drops the arguments.
- The IR has a text form, used by unit tests and a developer dump.

**Data.**
- Read-only: text constants (length, then bytes), failure messages, and type descriptors. A record's descriptor holds its name and its fields' names and descriptors; a list's holds its item's descriptor.
- Read-write: the memory pointers, the stack limit, a 4 KiB output buffer and an input buffer.
- `show_value`, `copy_value` and `equal_values` work from the descriptors.

**Runtime rules.** The language-visible rules (copies, `ask`, failure messages) are in the language spec: §7, §10 and §12.
- **Memory** comes from the OS in 64 MiB pieces, or larger for one big request. It is placed one value after another, 8-byte aligned, and never freed. A refusal is failure 10.
- **Stack.** The limit is measured from the top of the initial stack: the entry code saves the stack pointer at `argc`, and `_start` walks argv, envp and the auxiliary vector to `AT_EXECFN`, the executable's name, the highest string; its end rounded up to a page is the top (the entry stack pointer's own position is the fallback when the vector has no `AT_EXECFN`). The limit is that top minus the stack size (from `getrlimit`; 8 MiB if unlimited, capped at 1 GiB), plus a 64 KiB margin. Every feature call checks it (failure 10), so a large environment cannot make deep recursion fault. Runtime routines never recurse.
- **Output** is buffered (4 KiB). It is flushed when full, before `ask` reads, before a failure message and at exit. A failed write ends the program with status 2 and no message; writing to a closed pipe ends it by SIGPIPE instead, as is usual on Unix.

**Testing.**
- IR text tests for lowering.
- Golden bytes for every encoder form, plus an opt-in `objdump` cross-check run in the Alpine machine.
- `irsim` runs the runtime routines against the Rust `number` module on thousands of generated values, including range ends, halves and negatives.
- `tests/run/*.gloss` cases, each with `.out` and optionally `.in`, `.err` and an expected status, run as real executables through `GLOSS_RUNNER` and also under `irsim`.
- The spec guard runs every spec program without tables and checks every §10 failure message.
- Sweep guards on the case count.
- Without the Linux machine, the run tests are skipped with a printed warning; CI runs them natively.

**Reviews.** The encoder, the ELF writer and the OS primitives get their first adversarial review on the top tier.

### 4.7 Milestone 3: tables and files

**Approval at build time (built first, D23).**
- **The list.** Built from the checker's table declarations: a table that a `save` in some feature reaches is `read and replace`, any other is `read only`. A `save` inside a test block does not count: tests use pretend files (M4). Names are shown as D24 says. One function makes the list for the build, for `.allow` and for `--permissions`. It is printed only for programs that declare a table (D27).
- **What is approved.** `<source>.allow` plus every `--allow <file>` on the command line. A file needs a fresh approval if neither covers it, or if it moves from read only to read and replace.
- **Asking.** In a terminal, the build prints the list and `Allow? `; `yes` or `y`, in any case, approves; anything else or end of input refuses. Outside a terminal it refuses as D25 says. A refusal prints `Not approved: …`, writes no executable and exits 4. The question reads through a small trait and takes "is a terminal" as an input, so tests can drive both paths without a pseudo-terminal.
- **`.allow`.** A `note` line, then one line per file in the list's own form:

  ```
  note Files this program may touch; approved for expenses.gloss.
  read and replace   expenses.csv
  read only          rates.csv
  ```

  After a successful build it holds exactly the current list (D28), written atomically and only when it changed. A line that does not parse stops the build with status 3 and names the `.allow` line.
- **`--permissions`.** The executable carries its list as read-only data. Started with exactly one argument, `--permissions`, it prints the list and exits 0; other arguments are ignored. `_start` learns to read `argc` and `argv`. An executable with no tables prints `This program can:` and `  nothing else`.

**Files at run time.**
- **Tables in memory.** Each table is a list held in its own global slot, with a flag that is set by `load` and `clear`. Every list operation from M2 works on it unchanged, and a table passed to a `list of T` input is that list. `clear` gives the table a new empty list. `load` builds the new list in full and swaps it in only after the whole file parsed. `save` with the flag unset is failure 8.
- **OS primitives (D19).** `openat2`, `read` and `write` on a file descriptor, `fsync`, `renameat`, `close`, and the start-up arguments. Everything above them is runtime written in IR.
- **Load.** Open with `RESOLVE_BENEATH | RESOLVE_NO_SYMLINKS`. A missing file prints `expenses.csv not found; starting with an empty table` to the error output and loads an empty table. `ELOOP` or `EXDEV` is failure 6 (symbolic link); any other error is failure 6 (cannot be read). Read the whole file into one growing block, check it is UTF-8, then parse RFC 4180: the header is matched to fields by name, quoted cells may hold doubled quotes, commas and line breaks, `\n` and `\r\n` both end a line, and one final empty line is ignored. Number cells use the runtime's `parse_number`; truth cells must be `yes` or `no`.
- **Save.** Open the table's folder with `O_PATH | O_DIRECTORY` and the same resolve flags; a missing folder is failure 7 (folder does not exist). A target that is a symbolic link is failure 7. Write `<name>.gloss-new` in that folder (`O_CREAT | O_TRUNC`, mode 0644): the header in field order, then each row, numbers as `show` prints them, truths as `yes` and `no`, quotes only where needed, `\n` line endings. Then `fsync` the file, `renameat` it over the target and `fsync` the folder. Any write error is failure 7 (cannot be written). Output already shown is flushed before a failure, as in M2.
- **Checks already in place.** The M1 checker already treats `load` and `clear` as changing a list for the walking rule, refuses a saved table that nothing loads or clears, and warns when a `load` would replace rows appended earlier. M3 lifts the D22 refusal (D23).

**Failures 6–8 (D26).** `N` is the source line of the `load` or `save`; file lines count from 1, the header being line 1, and a row's line is the line where it starts.

| # | Cause | Message |
|---|---|---|
| 6 | A cell that is not a number | `line N: failure 6: expenses.csv line 7: amount "12,50" is not a number` |
| 6 | An empty number cell | `line N: failure 6: expenses.csv line 7: amount is empty` |
| 6 | A truth cell that is not `yes` or `no` | `line N: failure 6: expenses.csv line 7: paid "maybe" is not yes or no` |
| 6 | A row with the wrong number of cells | `line N: failure 6: expenses.csv line 7: has 4 cells; the header has 3` |
| 6 | An unknown column | `line N: failure 6: expenses.csv line 1: column amout is not a field of Expense` |
| 6 | A missing column | `line N: failure 6: expenses.csv line 1: column amount is missing` |
| 6 | A repeated column | `line N: failure 6: expenses.csv line 1: column amount appears twice` |
| 6 | A quoted cell left open | `line N: failure 6: expenses.csv line 9: a quoted cell is not closed` |
| 6 | Bytes that are not UTF-8 | `line N: failure 6: expenses.csv is not UTF-8 text (line 4)` |
| 6 | A symbolic link on the path | `line N: failure 6: expenses.csv is reached through a symbolic link` |
| 6 | Any other read error | `line N: failure 6: expenses.csv cannot be read` |
| 7 | The folder does not exist | `line N: failure 7: expenses.csv cannot be written (its folder data does not exist)` |
| 7 | Any other write error | `line N: failure 7: expenses.csv cannot be written` |
| 7 | A symbolic link on the path | `line N: failure 7: expenses.csv is reached through a symbolic link` |
| 8 | Saved before a load or clear | `line N: failure 8: expenses is saved here but was not loaded or cleared first` |

Cell text in a message follows failure 5's rules: at most 40 characters, invisible characters as `\u{XXXX}`, a `"` doubled. File and column names follow D24.

**Testing (D29).** Every message above has a run case with an exact `.err` and status 2. The unreadable-file and unwritable-folder cases skip visibly when run as root. Durability cannot be tested by pulling the power; a unit test on the generated IR checks the order `fsync`, `renameat`, `fsync` of the folder. `tests/cli.rs` covers: a first build answered yes and no, no terminal and no `--allow` (status 4, no executable), `--allow` writing `.allow`, a build covered by `.allow` (asks nothing, still prints the list), read only becoming read and replace, a dropped file shrinking `.allow`, and a broken `.allow` (status 3). Examples gain an expense tracker (`load`, `append`, `save`) and a read-only report.

**Reviews.** The security boundary gets its first review on the top tier: the approval logic and `.allow` parsing, the open and save paths, symbolic-link handling, safe writing, the new primitives in the encoder, and the CSV reader, which parses untrusted input.

## 5. Open questions for the owner

| # | Question | Default if unanswered | Answered |
|---|---|---|---|
| Q1 | Install OrbStack before M2 starts? | Yes. M2's plan begins with the install commands in §4.4. | Installed 2026-11-23, with an Alpine machine (D17) |
| Q2 | Should CI also run on pushes to `main`? | No: only pull requests that are not drafts, to save Actions minutes | |
| Q3 | Can a record body contain `note` lines? The language spec allows notes anywhere a line can be, but Appendix A's `record` rule lists only fields. | Yes: notes are allowed in record bodies, and the grammar gets the fix | |
| Q4 | Is `more` really a reserved word? It is in the language spec's list but no form uses it. | Keep it reserved, as the spec says | |

## 6. As built

### Milestone 1 (front end), finished 2026-11-23, `main` @ b15b5a7

**What shipped:** the lexer, parser, checker (names, types, feature results, flow, mutation, walking, save/load rules), `gloss format`, and the CLI for `build` and `format`. All 22 plan tasks are in, plus two waves that fixed every deferred review finding and every finding of the final whole-milestone review. There are 253 tests: 180 unit tests and 73 integration tests across the CLI, spec examples, generated format programs, nesting, scaling and other suites. Checking time grows linearly; a reverse-written 32,000-variable chain checks in about 0.7 s on a release build.

**Pull requests:** tasks #3–#74, README #75, deferred-findings wave and its fixes #76–#92, final-review fixes #93–#98. All were rebase-merged.

**Deviations from the decisions above** (rulings are numbered in the milestone ledger):
- **D14 (200-line PRs):** tasks over the cap were split into sequential PRs, never stacked (R3–R5, R7–R10, R12, R14, R19, R22, R24, R26, R28, R30, R33, R37). The owner allowed a one-time exception for the final-review fixes: six themed PRs, some over 200 lines (R47).
- **D10 (expression ids):** ids are assigned in post-order (children before parent). The order is still deterministic and unique (R11).
- **D7 (diagnostic text):** a problem ending in `?` gets no trailing period (R35). An echoed source line is the whole logical statement, cut at 200 characters, with invisible characters written as `\u{XXXX}` (R49).
- **D8 (order):** program-level diagnostics come first, then by line, with errors before warnings.
- **D16 (usage errors):** a source file that does not end in `.gloss` is also refused with exit status 3, so milestone 2's executable can never overwrite the source.
- **Q3:** yes. Notes are allowed in record bodies, and blank lines between fields are kept by `gloss format`.

**Language-spec changes made during milestone 1:**
- `add` added to the reserved words (R16).
- §8 path rules: rules 6–7 (unshowable characters, no `.gloss-new` ending; R25); no leading `-`; no spaces at the edges of a part; the reserved endings `.gloss`, `.allow`, `.trace` and `.svg`, compared without regard to case; a warning for paths that differ only in case.
- §3: tabs are not allowed in comments either.
- §7: the `run` message reads `set result to …`.
- "To consider later" notes: choosing by value (§7) and text ordering (§4).

**Carried into later milestones:**
- **M3 (requirement):** the permission list must display file names unambiguously, quoting or escaping lookalike and non-normalised Unicode. The compiler already refuses invisible and unshowable characters, but how the list shows the rest belongs to M3, where the list is built.
- **M2 (notes from the final review):**
  - Use `TypeInfo.names` instead of resolving scopes again.
  - Keep per-loop and per-test variable tables.
  - Represent a table as a list at run time.
  - Add a debug assert that expression ids are dense.
  - Keep the executable-name clash and `.gloss` checks.
  - Mind stack depth in generated code.
  - Trap when control falls off the end of a feature.
  - Negating the most negative number is failure 4.

The milestone ledger (every ruling R2–R49 and the review record) is kept at `~/.gloss/ledgers/2026-11-22-compiler-m1-front-end/`.

### Milestone 2 (code generation and runtime), finished 2026-11-23, `main` @ ae91377 plus this documentation

**What shipped:** `gloss build x.gloss` checks the program and writes the static x86-64 Linux ELF `x` beside the source (atomically, mode 0755), printing `Built x`. The pipeline is check → `lower` → Gloss IR (the user's features plus the reachable runtime routines, which are written in IR, D19) → x86-64 backend → encoder → ELF writer. Every statement and expression of the language spec except tables, `load`, `save` and `clear` builds, and test blocks (`expect`, `input`) are left out of the built program until milestone 4; run-time failures 1–5, 9 and 10 stop the program with the spec's messages. `irsim`, a test-only IR interpreter, is the reference the real executables are compared with (D21). A run harness (`tests/run_support`) builds, runs and times executables through `GLOSS_RUNNER`, and 97 `tests/run/*.gloss` cases run both as real executables and under `irsim`. `examples/` holds nine programs that are built, run and format-checked by `tests/examples.rs`. The language spec's complete programs and its §10 failure messages are checked end to end.

**Pull requests** (all rebase-merged, one at a time; the plan itself was #101):

| Task | What | PRs |
|---|---|---|
| 1 | Number arithmetic (checked add/sub/mul/div/rem/whole) | #102 |
| 2a, 2b | IR types, module and interning; IR text form | #103–#104, #105–#106 |
| 3a, 3b | `irsim` core; number, system and memory primitives | #107–#114, #115–#118 |
| 4 | x86-64 instruction type and Intel text | #119–#122 |
| 5a, 5b, 5c | Encoder: moves and memory; arithmetic; flow | #123–#124, #125–#126, #127–#128 |
| 6 | Static ELF writer, run support | #129–#132 |
| 7 | Opt-in `objdump` cross-check | #133–#135 |
| 8a, 8b, 8c | Backend; number routines and differential tests; Mac output capture | #136–#140, #141–#157, #158 |
| 9 | Runtime core (start, output, failures, memory, stack) | #159–#168 |
| 10a, 10b | Lowering skeleton; `gloss build` and the run cases | #169–#170, #171–#172 |
| 11, 12a–c | Showing numbers; number expressions, verbs, reference sweep | #173–#176, #177–#179, #180–#181, #182 |
| 13, 14 | Truths, `if`, `repeat while`; counting loops | #183–#186, #187–#193 |
| 15 | Features, `give`, `run`, recursion, stack check | #194–#201 |
| 16, 17 | Text, equality, `as text`, `with N places`; `as number` | #202–#207, #208–#221 |
| Examples | Seven programs, then binary search | #222–#224 |
| 18 | `ask` | #225–#231 |
| 19, 20, 21, 22 | Records; lists; showing and comparing lists and records; copies | #232–#237, #238–#248, #249–#254, #255–#263 |
| 23, 24 | Out of memory; the spec's programs and failure messages | #264, #265–#266 |

**Deviations from the decisions above** (rulings are numbered M2-R1 … M2-R36 in the milestone ledger):
- **D14 (200-line PRs):** tasks over the cap were split into sequential PRs, never stacked and never trimmed to fit (M2-R2, M2-R4, M2-R5). Work happened in the main checkout, one branch at a time, after a worktree was deleted under a running session (M2-R6).
- **D2 and D17:** the machine is Alpine 3.23 `gloss-amd64` under Rosetta; `objdump` comes from `apk add binutils`. Rosetta is not a real kernel, so CI (`ubuntu-latest`) is the check of the real loader.
- **D21 (irsim):** the `.ulimit` run cases are skipped under `irsim`, which has no address-space limit; the skips are counted against the `.ulimit` files so none can vanish.
- **D22:** the refusal prints as `tables arrive in milestone 3.` (the renderer adds the period to a program-level problem).
- **§4.2 pipeline:** there is no `codegen` module; `lower` and `target` take its place.
- **IR machine (§4.6):** `b` (`rcx`) survives a call, so each callee saves it in an extra frame word (M2-R12). `cmp r,[mem]` and `movzx` were not added to the encoder; the backend uses existing forms (M2-R13).
- **Plan rulings:** P1: a `for each … from … to` start that is not whole is failure 3, `the start of a count must be whole (<source> is <value>)`. P2: a feature that falls off its end stops with `internal error: feature <name> ended without giving a value`, status 2 (worded and placed by M2-R25). P3: failure routines take the line as an argument; there is no current-line global.

**Rulings** (final state; later rulings are noted against the ones they supersede):

*Numbers and arithmetic*
- M2-R1: `#[allow(clippy::should_implement_trait)]` on `Number::rem`; the plan fixes the name.
- M2-R7: IR `Int(Div)`/`Int(Rem)` need a divisor above 0 (the runtime uses only positive constants); `irsim` panics on others.
- M2-R8: after a failed `Num*` jump `a` is undefined (x86 has already written `rax`) and `b` is unchanged; `irsim` poisons `a`.
- M2-R22: the grammar admits a minus after a built-in's `of` and both `remainder` operands, because the spec's own prose examples (`whole of -3.5`, `remainder of -7 by 2`) did not parse; parser, formatter and `spec/v0.md` changed.
- M2-R23: the words in a failure's parentheses are the failing expression as `gloss format` writes it (the AST has no column spans); `spec/v0.md` §10 reworded.

*IR, instruction encoder and backend*
- M2-R3: `Module::text`/`desc` keep a linear scan.
- M2-R9: `MovRI` prints as `movabs`, because the encoder always emits an imm64 form.
- M2-R10: the encoder keeps `u8` shift counts; the backend asserts 0..63.
- M2-R12, M2-R13: see the deviations above.

*Run support and Mac execution*
- M2-R11: `GLOSS_RUNNER` is split on spaces by design; a runner path with a space needs a wrapper script.
- M2-R14: when the runner starts with `orb`, output goes to fresh files in the shared test directory and is read back, because OrbStack's forwarding drops the tail of piped output under load (found with plain `head -c 1MiB`; 0 of 4800 runs short afterwards, against 10 of 3200 before). An `#[ignore]` stress test keeps the evidence.

*Runtime*
- M2-R15, M2-R32: a failed write (result 0 or below) or a failed read ends the program with status 2 and no message, after flushing. A write to a closed pipe never gets that far: the kernel's SIGPIPE ends the program first (M2-R36).
- M2-R16: `stack_limit(size, sp)` is a separate routine so the cap cases can be tested.
- M2-R17: `_start` calls `f_start`.
- M2-R18, superseded by M2-R26: the stack limit is measured from the top of the initial stack (the end of the `AT_EXECFN` string rounded up to a page), falling back to the entry stack pointer; the walks of envp, auxv and the name are bounded (65536, 64, 4096). The 65536 bound is untested (M2-R27).
- M2-R20, superseded by M2-R21: showing a number never allocates (a 32-byte scratch, `Global::NumBuf`); `number_text(n, line)` allocates and is used for joins and `as text`.
- M2-R25: the fall-off message has no `line N:` prefix, flushes first and exits 2; it is lowered inline.
- M2-R28: `parse_number` returns its status in `a` and its value in the first 8 bytes of `NumBuf`, valid until the next `number_format` or `parse_number`.
- M2-R29: `err_shown` never reads past the end; an invalid UTF-8 byte counts as one character and is written `\u{FFFD}`.
- M2-R30, M2-R31: the source in every failure message escapes invisible characters as `\u{XXXX}`; a quote inside failure 5's quoted text is doubled (`spec/v0.md` §10 updated).

*Lowering and checking*
- M2-R19: placeholders for unlowered constructs read `this compiler cannot build this statement/expression yet`.
- M2-R24: the spec's §9 program needed lists, so `features.gloss` was added with lists.
- M2-R33: no run case above 10000 bytes of input; growth is unit-tested at 100000 bytes and 1 MiB was checked by hand.
- M2-R34: replacing an item of a list a loop is walking (`set`, or an arithmetic verb on an item) is refused with the existing `cannot change while the loop on line N walks it`; changing a field of an item stays allowed (`spec/v0.md` §4 updated).
- M2-R35: the out-of-memory case runs with a 1 GiB `.ulimit` (the plan said 64 MiB), since Rosetta needs about 800 MB of address space to start.
- M2-R36: SIGPIPE is not ignored, so writing to a closed pipe ends the program by signal (status 141 under a shell), as is usual on Unix; a failed write that returns an error exits with status 2.

**Tests:** 518 pass and 1 is ignored (the stress test): 415 unit tests and 103 integration tests, including the CLI, 97 run cases (real and `irsim`), the examples, the spec guard, the 2000-result number reference, the objdump check (opt-in, so it skips by default), and the differential tests of backend against `irsim`. Encoder sweeps against `objdump` found 0 mismatches over more than 40,000 instructions; the number routines agreed on 220,000 generated pairs.

**Known limits and deferred items:**
- **M3 and M4:** tables (refused, D22), `load`, `save`, `clear`, `expect` and `input`. Run-time failures 6–8 are worded in M3. The permission list must show file names unambiguously (carried from M1).
- **Memory:** nothing is freed. A run under Rosetta needs a `ulimit -v` above about 800 MB, so the out-of-memory case uses 1 GiB (M2-R35), unverified on native x86-64.
- **Stack:** `irsim` models a 1 MiB stack (smaller than the real 8 MiB default, to keep `too_deep` cheap), so the depth at which failure 10 triggers differs from a real run; the message is the same. The stack limit is capped at 1 GiB.
- **Mac runs:** every run under `orb` is Rosetta. The `tests/scaling.rs` reverse-chain test once took 10.5 s under heavy compile load (bound 10 s), and the difftest once failed in a cold run that could not be reproduced; both are WATCH items. Once, a runaway `dhcpcd` inside the machine kept OrbStack at 100% CPU and slowed the run tests about 20 times; if run tests crawl, check the machine's processes first.
- **Failure messages** quote the expression as `gloss format` writes it (M2-R23).
- **Reviews:** the encoder, ELF writer, runtime memory, input and descriptor code each had a first review on the top tier.

The milestone ledger (every ruling, review record and carry) is kept in `.superpowers/sdd/2026-11-23-compiler-m2-codegen/progress.md`.
