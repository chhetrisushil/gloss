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

Milestones 1 to 4 are built. The compiler reads a Gloss program, reports every compile error and warning the spec defines, formats source into its one accepted form, and `gloss build` writes a static x86-64 Linux executable. Programs can declare tables, and `load` and `save` read and write CSV files, and `clear` empties a table. A program that touches files needs approval at build time (see [Tables and files](#tables-and-files)). Test blocks (`expect`, `input`) are left out of the built program; `gloss test` runs them (see [Tests](#tests)).

| Milestone | What it adds | State |
|---|---|---|
| 1 | Lexer, parser, checker, `gloss build` (checks only), `gloss format` | Done |
| 2 | Code generation and runtime: `gloss build` writes an x86-64 Linux executable | Done |
| 3 | Tables and files: CSV `load`/`save`/`clear`, file permissions, `--allow`, `.allow` | Done |
| 4 | `gloss test`: test blocks, `expect`, `input`, `shown`, pretend files | Done |
| 5 | `gloss trace` | Next |
| 6–7 | `gloss explain`, `gloss flowchart` | Planned |

## Getting started

### Prerequisites

- Rust **1.87 or later** (edition 2024), with `rustfmt` and `clippy`: `rustup component add rustfmt clippy`.
- No other dependencies. The compiler uses only the Rust standard library.
- Running the executables Gloss produces needs **x86-64 Linux**. On an Apple Silicon Mac this means an x86-64 Linux machine in [OrbStack](https://orbstack.dev), run under QEMU; see [OrbStack setup](#orbstack-setup-macos). Building the compiler itself works on any platform Rust supports.

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

## Tables and files

A `table` holds records in memory and names a CSV file. The header row holds the field names; each later row is one record.

```gloss
table expenses of Expense in "expenses.csv"
```

- `load expenses` reads the file into the table. A missing file prints a notice and gives an empty table. Bad data stops the program with run-time failure 6 and names the file line.
- `save expenses` writes the table back. It writes `expenses.csv.gloss-new` beside the file, syncs it, and renames it over the original, so a failed save never leaves a half-written file. A table must be loaded or cleared before it is saved (failure 8). Failure 7 is a write that cannot happen.
- `clear expenses` empties the table in memory; a later `save` writes it.
- Files are found relative to the folder the program runs from. Paths that go through a symbolic link are refused.

File access is approved when you build, not when the program runs. `gloss build` prints what the program can touch and asks:

```console
$ gloss build expenses.gloss
This program can:
  read and replace   expenses.csv
  nothing else
Allow? y
Approval saved in expenses.allow
Built expenses
```

- Approval is stored in `expenses.allow` beside the source. After every successful build the file holds exactly the current list, so a file the program stops using loses its approval.
- `gloss build expenses.gloss --allow expenses.csv` approves a file without the question, once per file. `--allow` comes after the source file. Use it where there is no terminal. A file the program does not declare is a compile error (status 3).
- With no terminal and no approval, the build prints `Not approved: ...`, writes no executable and exits with status **4**.
- `./expenses --permissions` prints the same list from the built executable and exits 0.

## Tests

A `test` block names a promise in plain words and checks it with `expect`:

```gloss
test "expenses saved to a file load back"
  append Expense with name "Lunch", amount 12.5, category "food" to expenses
  append Expense with name "Bus", amount 3, category "travel" to expenses
  save expenses
  clear expenses
  load expenses
  expect count of expenses is 2
  expect name of first of expenses is "Lunch"
```

`gloss test expenses.gloss` builds a test executable, runs every test in source order and prints one block per test, then a blank line and a summary:

```console
$ gloss test expenses.gloss
pass  category total counts only the matching category
fail  largest expense is found
      line 23: expect name of largest_expense is "Dinner"
      shown:
        Largest so far: Bus at 40
pass  a cost that is not a number is asked for again

2 passed, 1 failed
```

- A test passes when every `expect` holds and no run-time failure happens. A failure stops that test only and is reported under its title, as `line N: expect <condition>` or as the usual failure message. `shown:` appears only when the test printed something.
- `gloss test` exits **0** when every test passed or the program has none (it prints `No tests in expenses.gloss`), **1** when a test failed, and **3** on a compile error. `--allow` is a usage error, because tests never ask.
- Each test runs in its own process, so a failing or crashing test cannot disturb the next one. A test that ends without a result is reported as `ended without a result (status N)`.
- Never a real file: every table starts empty and counts as cleared, and its file is a pretend file in memory, so `load` and `save` work as in a real run, with no approval and nothing written. `input` queues the answers `ask` gives, and `shown` lists what `show` printed. Output is quiet unless the test fails.
- The test executable is written to a private folder, run, and deleted. Nothing is written beside the source.
- **x86-64 Linux only.** A test is run as a real program, so on any other host `gloss test` prints `gloss test runs tests on Linux only` and exits 3, after reporting any compile errors. On a Mac, run it inside the OrbStack machine (see [OrbStack setup](#orbstack-setup-macos)). A test that loops forever hangs the run; there is no time limit.

The examples `gcd`, `primes`, `fibonacci` and `expenses` carry tests; `tests/examples.rs` checks their reports.

## Commands

| Command | What it does | Available |
|---|---|---|
| `gloss build FILE [--allow F]...` | Checks the program, then writes the executable beside the source and prints `Built NAME`. Prints errors and warnings to standard error. | Now |
| `gloss format FILE` | Rewrites the file into the one accepted form. A file that does not parse is left untouched. | Now |
| `gloss test FILE` | Runs every test block and prints each title with `pass` or `fail`. x86-64 Linux only. | Now |
| `gloss trace FILE` | Runs the program and narrates each step in plain words | Milestone 5 |
| `gloss explain FILE` | Prints each source line beside its machine instructions | Milestone 6 |
| `gloss flowchart FILE` | Writes one SVG flowchart per feature | Milestone 7 |

Exit status is **0** on success, **1** when a test failed, **3** on a compile error, wrong usage, or a test run that `gloss test` could not start, and **4** when file access was not approved. Warnings alone do not fail a build. The full set of statuses (including failed tests and run-time failures) is in section 11 of the spec.

## Working on the compiler

### Tests and the verify gate

Run this before every pull request; CI runs the same three steps:

```sh
cargo fmt --check && cargo clippy --all-targets -- -D warnings && cargo test
```

- **Run tests** (`tests/run.rs`, `tests/examples.rs` and others) build real programs and run the executables. They use the command in `GLOSS_RUNNER`, split on spaces: `orb -m gloss-amd64` on macOS when `orb` is on the path, nothing (run directly) on x86-64 Linux when it is unset. With no runner they are skipped, and print `run tests skipped: no x86-64 Linux runner (set GLOSS_RUNNER)`. Every case also runs under `irsim`, the test-only IR interpreter, on any host.
- **Unit tests** sit beside each module (`#[cfg(test)]`) and assert exact diagnostic text, because messages are part of the language design.
- **`tests/cli.rs`** runs the real `gloss` binary.
- **Test cases** (`tests/test_cases/*.gloss`, with `.out`, `.err` and `.status`) are programs with test blocks. Each is built with `build_test_executable`, run as a real executable through the runner, and also run under `irsim`; the two must agree. The tests of `gloss test` itself (`src/testrun.rs`, the Linux-only cases of `tests/cli.rs`) need a real x86-64 Linux host; on a Mac, see below.
- **`tests/spec_examples.rs`** compiles every ` ```gloss ` example in `spec/v0.md`, so the spec and the compiler cannot drift apart.
- **`tests/format_generated.rs`** checks that formatting never changes a program and is idempotent.

- `GLOSS_OBJDUMP=1 cargo test --test objdump` also checks every encoded instruction against `objdump` inside the machine (opt-in; needs a runner).
- On a Mac the machine runs x86-64 under QEMU, not on a real x86-64 kernel. QEMU ignores `ulimit -v` for x86-64 code, so the runs that need a memory limit are skipped, with the message `memory-limit run skipped: the runner ignores ulimit -v for x86-64 code (QEMU)`. CI runs the same tests on a real kernel, so it is the check of the real loader and of the memory limit.

### Running the Linux-only tests on a Mac

The tests that start `gloss test` as a program need an x86-64 Linux host. Cross-compile them and run them in the OrbStack machine:

```sh
rustup target add x86_64-unknown-linux-musl
CARGO_TARGET_X86_64_UNKNOWN_LINUX_MUSL_LINKER=rust-lld \
  cargo test --target x86_64-unknown-linux-musl --no-run
orb -m gloss-amd64 sh -c "cd $PWD && ./target/x86_64-unknown-linux-musl/debug/deps/<test executable>"
```

`--no-run` prints the path of each test executable. The machine has no network and no Rust, which is why the build happens on the Mac.

### OrbStack setup (macOS)

Once per Mac:

```sh
brew install --cask orbstack
orb start
orb config set rosetta false               # run x86-64 under QEMU
orb create --arch amd64 alpine gloss-amd64
orb -m gloss-amd64 sudo apk add binutils   # only for the objdump check
```

Rosetta is turned off because it does not implement `openat2`, which the file code uses. OrbStack shares `/Users` with the machine, so `orb -m gloss-amd64 ./hello` runs a binary built on the Mac. The executables are static, so Alpine's small size is enough.

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
| `src/permissions.rs`, `src/approve.rs` | The permission list, the `.allow` file, and build-time approval |
| `src/ir/`, `src/lower/`, `src/runtime/` | The Gloss IR, lowering from the checked program, and the runtime written in IR |
| `src/target/` | The x86-64 backend, encoder, static ELF writer and Linux system calls |
| `src/irsim/` | Test-only IR interpreter |
| `src/testrun.rs` | `gloss test`: the private folder, running the test executable, and its status |
| `examples/` | Programs that build and run, with expected output; programs that use files carry a `.allow` |
| `src/format/` | `gloss format`: canonical printing, rewrites, line wrapping |
| `src/cli.rs`, `src/main.rs` | The `gloss` command line |

### Contributing

- Read the spec section and the plan task before changing behaviour; the spec wins any disagreement.
- Keep pull requests small: at most 200 changed lines and 10 files. Larger work is split into sequential PRs.
- Open pull requests as drafts. CI runs when a PR is marked ready. Merges are rebase-only.
- No `#[rustfmt::skip]`, no new crates without a recorded decision, and no weakening of existing tests.
