# Gloss v0 compiler - design spec

- **Date:** 2026-11-22
- **Status:** built (milestone 1); milestone 2 approved for planning (section 4.6); milestones 3–7 not started
- **Context:** Implements the language defined in `spec/v0.md` (the *language spec*). This document decides *how* the compiler is built. Where the two disagree about the language, the language spec wins. Where they disagree about the implementation, this document wins.
- **Plan:** `docs/plans/2026-11-22-compiler-m1-front-end.md` (milestone 1). Later milestones get their own plans.

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
- **Stack.** The limit is the stack's start minus its size (from `getrlimit`; 8 MiB if unlimited, capped at 1 GiB), plus a 64 KiB margin. Every feature call checks it (failure 10). Runtime routines never recurse.
- **Output** is buffered (4 KiB). It is flushed when full, before `ask` reads, before a failure message and at exit. A failed write ends the program with status 2 and no message.

**Testing.**
- IR text tests for lowering.
- Golden bytes for every encoder form, plus an opt-in `objdump` cross-check run in the Alpine machine.
- `irsim` runs the runtime routines against the Rust `number` module on thousands of generated values, including range ends, halves and negatives.
- `tests/run/*.gloss` cases, each with `.out` and optionally `.in`, `.err` and an expected status, run as real executables through `GLOSS_RUNNER` and also under `irsim`.
- The spec guard runs every spec program without tables and checks every §10 failure message.
- Sweep guards on the case count.
- Without the Linux machine, the run tests are skipped with a printed warning; CI runs them natively.

**Reviews.** The encoder, the ELF writer and the OS primitives get their first adversarial review on the top tier.

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
