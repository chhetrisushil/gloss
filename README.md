# Gloss

Gloss is a small language that reads like plain sentences and compiles straight to a native executable. It is for command-line programs that work on records and tables stored in files.

```gloss
record Expense
  name as text
  amount as number
  category as text

table expenses of Expense in "expenses.csv"

feature category_total
  needs wanted as text
  set total to 0
  for each expense in expenses
    if category of expense is wanted
      add amount of expense to total
  give total

test "category total is zero when nothing matches"
  expect category_total with "rent" is 0
```

## Why

Agents now write most code, and fewer people read it. Gloss aims to make program logic readable again, for three groups:

- **Developers who have drifted from code** use it to stay close to algorithms and to what the machine does.
- **People who have never programmed** use it to read what an agent wrote for them, check it through tests, and change it themselves.
- **Agents** write it. The whole language is small enough to fit in a prompt.

## Design rules

1. **Structured like code, worded like English.** Every statement starts with a fixed keyword.
2. **One form per idea.** There are no synonyms and no optional words.
3. **No grammar words.** There are no articles, pronouns or plurals.
4. **Symbols for arithmetic, words for everything else.**
5. **The compiler infers but never guesses.**
6. **Nothing hidden.** Every line can show the machine instructions it became, and file access is approved at build time.

## Status

Milestones 1 and 2 are built. The compiler reads a Gloss program, reports every compile error and warning the spec defines, formats source into its one accepted form, and `gloss build` writes a static x86-64 Linux executable. Programs that declare a table are refused (`tables arrive in milestone 3`); so are `load`, `save`, `clear`, `expect` and `input`, which belong to milestones 3 and 4.

| Milestone | What it adds | State |
|---|---|---|
| 1 | Lexer, parser, checker, `gloss build` (checks only), `gloss format` | Done |
| 2 | Code generation and runtime: `gloss build` writes an x86-64 Linux executable | Done |
| 3 | Tables and files: CSV `load`/`save`, file permissions | Next |
| 4 | `gloss test` | Planned |
| 5–7 | `gloss trace`, `gloss explain`, `gloss flowchart` | Planned |

## Getting started

### Prerequisites

- Rust **1.87 or later** (edition 2024), with `rustfmt` and `clippy`: `rustup component add rustfmt clippy`.
- No other dependencies. The compiler uses only the Rust standard library.
- Running the executables Gloss produces needs **x86-64 Linux**. On an Apple Silicon Mac this means an x86-64 Linux machine in [OrbStack](https://orbstack.dev); see [OrbStack setup](#orbstack-setup-macos). Building the compiler itself works on any platform Rust supports.

### Build

```sh
git clone git@github.com:chhetrisushil/gloss.git
cd gloss
cargo build --release          # the compiler is target/release/gloss
cargo install --path .         # optional: put `gloss` on your PATH
```

### Try it

Write a program, `hello.gloss`:

```gloss
feature start
  show "hello"
```

Build it. This checks the program and writes the executable `hello` beside the source:

```console
$ gloss build hello.gloss
Built hello
```

Run it: `./hello` on x86-64 Linux, or `orb -m gloss-amd64 ./hello` on a Mac. It prints `hello`.

The [`examples/`](examples/README.md) folder holds more programs (FizzBuzz, greatest common divisor, primes, Collatz, square root, binary search, ...), each with the output it must produce.

Errors name the line, quote it, explain the problem and suggest the fix:

```console
$ gloss build oops.gloss
line 2: set total as 0. set needs to. Did you mean set total to 0?
```

`gloss format` rewrites a file in place into its one accepted form, including rewrites such as `set t to t + 1` → `add 1 to t`:

```console
$ gloss format messy.gloss
Formatted messy.gloss
```

## Commands

| Command | What it does | Available |
|---|---|---|
| `gloss build FILE` | Checks the program, then writes the executable beside the source and prints `Built NAME`. Prints errors and warnings to standard error. | Now |
| `gloss format FILE` | Rewrites the file into the one accepted form. A file that does not parse is left untouched. | Now |
| `gloss test FILE` | Runs every test block and prints each title with pass or fail | Milestone 4 |
| `gloss trace FILE` | Runs the program and narrates each step in plain words | Milestone 5 |
| `gloss explain FILE` | Prints each source line beside its machine instructions | Milestone 6 |
| `gloss flowchart FILE` | Writes one SVG flowchart per feature | Milestone 7 |

Exit status is **0** on success and **3** on a compile error or wrong usage. Warnings alone do not fail a build. The full set of statuses (including failed tests and run-time failures) is in section 11 of the spec.

## Working on the compiler

### Tests and the verify gate

Run this before every pull request; CI runs the same three steps:

```sh
cargo fmt --check && cargo clippy --all-targets -- -D warnings && cargo test
```

- **Run tests** (`tests/run.rs`, `tests/examples.rs` and others) build real programs and run the executables. They use the command in `GLOSS_RUNNER`, split on spaces: `orb -m gloss-amd64` on macOS when `orb` is on the path, nothing (run directly) on x86-64 Linux when it is unset. With no runner they are skipped, and print `run tests skipped: no x86-64 Linux runner (set GLOSS_RUNNER)`. Every case also runs under `irsim`, the test-only IR interpreter, on any host.
- **Unit tests** sit beside each module (`#[cfg(test)]`) and assert exact diagnostic text, because messages are part of the language design.
- **`tests/cli.rs`** runs the real `gloss` binary.
- **`tests/spec_examples.rs`** compiles every ` ```gloss ` example in `spec/v0.md`, so the spec and the compiler cannot drift apart.
- **`tests/format_generated.rs`** checks that formatting never changes a program and is idempotent.

- `GLOSS_OBJDUMP=1 cargo test --test objdump` also checks every encoded instruction against `objdump` inside the machine (opt-in; needs a runner).
- On a Mac the machine runs x86-64 under Rosetta, not on a real x86-64 kernel. CI runs the same tests on a real kernel, so it is the check of the real loader.

### OrbStack setup (macOS)

Once per Mac:

```sh
brew install --cask orbstack
orb start
orb create --arch amd64 alpine gloss-amd64
orb -m gloss-amd64 sudo apk add binutils   # only for the objdump check
```

OrbStack shares `/Users` with the machine, so `orb -m gloss-amd64 ./hello` runs a binary built on the Mac. The executables are static, so Alpine's small size is enough.

### Repository layout

| Path | What it holds |
|---|---|
| `spec/v0.md` | The language spec (binding). Appendix A is the complete grammar. |
| `docs/specs/` | Implementation design specs: decisions with ids (D1, D2, …) and rejected alternatives |
| `docs/plans/` | Milestone plans, one task per section |
| `src/source.rs`, `src/diag.rs` | Source text and diagnostics (`line N: <source>. <problem>.`) |
| `src/number.rs` | The fixed-point number type |
| `src/lexer/` | Tokens, line scanning, indentation, notes, continuation lines |
| `src/ast/` | The syntax tree, plus printers (source form and a compact debug form for tests) |
| `src/parser/` | Recursive-descent parser following Appendix A |
| `src/check/` | Names, types, statements, feature results, flow and mutation rules |
| `src/ir/`, `src/lower/`, `src/runtime/` | The Gloss IR, lowering from the checked program, and the runtime written in IR |
| `src/target/` | The x86-64 backend, encoder, static ELF writer and Linux system calls |
| `src/irsim/` | Test-only IR interpreter |
| `examples/` | Programs that build and run, with expected output |
| `src/format/` | `gloss format`: canonical printing, rewrites, line wrapping |
| `src/cli.rs`, `src/main.rs` | The `gloss` command line |

### Contributing

- Read the spec section and the plan task before changing behaviour; the spec wins any disagreement.
- Keep pull requests small: at most 200 changed lines and 10 files. Larger work is split into sequential PRs.
- Open pull requests as drafts. CI runs when a PR is marked ready. Merges are rebase-only.
- No `#[rustfmt::skip]`, no new crates without a recorded decision, and no weakening of existing tests.
