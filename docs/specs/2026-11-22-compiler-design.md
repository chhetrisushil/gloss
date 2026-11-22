# Gloss v0 compiler - design spec

- **Date:** 2026-11-22
- **Status:** draft
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
| D2 | How are produced executables run on the arm64 Mac? | An OrbStack Linux machine named `gloss-amd64` (Ubuntu, `--arch amd64`, run through Rosetta). The test harness prefixes executions with the command in `GLOSS_RUNNER`, set to `orb -m gloss-amd64` on the Mac and left empty on Linux. | The target stays x86-64 Linux without changing the language spec. Chosen by the owner, 2026-11-22. |
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

## 3. Rejected alternatives

| Alternative | Rejected because | Revisit if |
|---|---|---|
| Go, Zig | Owner chose Rust (D1) | n/a |
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
| `asm` | The x86-64 instruction encoder and the small assembly language the runtime is written in | M2 |
| `codegen` | Code for each statement, with naive stack slots | M2 |
| `runtime` | Runtime routines written with `asm` | M2, M3 |
| `elf` | Static ELF writer | M2 |
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

- **On the Mac:** `brew install orbstack`, then `orb create --arch amd64 ubuntu gloss-amd64`. OrbStack shares `/Users` with the machine, so `orb -m gloss-amd64 /Users/.../program` runs a binary built on the Mac.
- **The harness** reads `GLOSS_RUNNER` and splits it on spaces to form the command prefix.
- **CI** runs on `ubuntu-latest` (x86-64) with `GLOSS_RUNNER` empty.

### 4.5 Security

Table paths are the only security boundary in M1. The checker rejects absolute paths, `..`, empty parts, backslashes, and the same file declared by two tables. The run-time half of the boundary (symbolic links, `openat2`) is M3. Tasks that touch the boundary get a first review on the top tier.

## 5. Open questions for the owner

| # | Question | Default if unanswered | Answered |
|---|---|---|---|
| Q1 | Install OrbStack before M2 starts? | Yes. M2's plan begins with the install commands in §4.4. | |
| Q2 | Should CI also run on pushes to `main`? | No: only pull requests that are not drafts, to save Actions minutes | |
| Q3 | Can a record body contain `note` lines? The language spec allows notes anywhere a line can be, but Appendix A's `record` rule lists only fields. | Yes: notes are allowed in record bodies, and the grammar gets the fix | |
| Q4 | Is `more` really a reserved word? It is in the language spec's list but no form uses it. | Keep it reserved, as the spec says | |

## 6. As built

Empty until implementation ends.
