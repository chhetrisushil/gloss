# Gloss compiler M1: front end - implementation plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task by task. Steps use checkbox (`- [ ]`) syntax for tracking. Ledger: `.superpowers/sdd/2026-11-22-compiler-m1-front-end/progress.md`.

**Goal:** A `gloss` binary that lexes, parses and checks any v0 program, reports every compile error and warning in the language spec with its exact message, and formats source into its one accepted form.

**Architecture:** One Rust crate, std only. The pipeline is `Source` → `lexer::lex` → `parser::parse` → `check::check`, with `format` working on the parsed AST. The CLI exposes `gloss build` (check only until M2) and `gloss format`.

**Tech stack:** Rust 2024 edition, stable 1.87 or later, no crates.

**Spec:** `docs/specs/2026-11-22-compiler-design.md` (binding). The language is defined by `spec/v0.md` (the *language spec*). Conflicts about the language resolve against the language spec; conflicts about the implementation resolve against the design spec.

**How to read the tasks.**
- **Binding:** the tests in each task, with their exact strings, and the types and signatures in each task's **Interfaces** block.
- **Guidance:** implementation steps give the algorithm and the key code. The implementer writes the rest.
- A test may be adjusted only if it contradicts the language spec. Record that as a ruling in the ledger.

## Global constraints

- [D1] Rust 2024 edition; `rust-version = "1.87"` in `Cargo.toml`.
- [D3] No `[dependencies]` and no `[dev-dependencies]`. A task that wants a crate stops and asks.
- [D4] One crate named `gloss`, with the library in `src/lib.rs` and the binary in `src/main.rs`.
- [D7] Diagnostics render as `line N: <source line trimmed>. <problem>.`, plus ` Did you mean <form>?` when there is a suggestion. Warnings put `warning: ` before the source line. Program-level diagnostics render as `<problem>.`
- [D8] Diagnostics are sorted by line, with program-level ones first, then errors before warnings on the same line.
- [D10] Every `Expr` has an `ExprId(u32)`, numbered from 0 in parse order.
- [D11] Notes are AST nodes.
- [D16] Wrong usage of `gloss` exits with status 3.
- Problem strings never end with a period; `render` adds it.

## Rules for every PR

- **Size.** At most 200 changed lines, tests included, and at most 10 files. Estimates run about 40% low: every task below is estimated at 140 lines or fewer. If a task grows past 200, split it, and record the split in the ledger.
- **Branches.** One branch per task: `m1/tNN-<slug>`. Stack each branch on the previous task's branch, and open the PR with `--base` set to that branch.
  - When the owner rebase-merges a parent, rebase the child: `git rebase --onto origin/main <old-parent-tip> <child>`.
  - Record each task's base branch on its `dispatched` ledger line.
- **Pull requests.** Open as a draft: `gh pr create --draft`. The owner marks PRs ready and merges them with `gh pr merge <n> --rebase`. Never squash.
- **Commits** end with `Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>`. PR bodies end with `🤖 Generated with [Claude Code](https://claude.com/claude-code)`.
- **Verify gate**, run before reporting a task done: `cargo fmt --check && cargo clippy --all-targets -- -D warnings && cargo test`. Expected: every test passes, with no warnings.
- **Reviews.**
  - Implementers and reviewers use `sonnet`.
  - Task 12 (table-path rules, a security boundary) gets its first review on `opus`. Any follow-up review uses `sonnet`.
  - Re-check a one-line fix with the targeted test only.

## Resource limits

- At most 2 agents at once, reviews included. Only one may run `cargo` at a time.
- Tell every agent to use `CARGO_BUILD_JOBS=4`.
- Tests that write files use `std::env::temp_dir()` with a unique subfolder, and delete it at the end of the test.

## Findings that shape this plan

| # | Finding (verified 2026-11-22) | Consequence |
|---|---|---|
| F1 | The Mac is arm64, with no container or VM tooling installed | M1 runs no produced executables, so it needs nothing. M2 starts by installing OrbStack (design spec §4.4). |
| F2 | rustc and cargo are 1.87.0, with clippy and rustfmt | Edition 2024 is available |
| F3 | The repo holds only `spec/v0.md`, `README.md` and `docs/` | Task 1 creates the crate |
| F4 | Blank lines inside blocks must survive `gloss format` (language spec §10) | Tokens carry `blank_before`, and so do statements |

## File map

| File | Responsibility | Task |
|---|---|---|
| `Cargo.toml`, `.gitignore`, `.github/workflows/ci.yml` | Crate and CI | 1 |
| `src/lib.rs` | Module list, plus `parse_source` and `check_source` helpers | 1, grows |
| `src/main.rs` | Calls `cli::run` | 1, 20 |
| `src/source.rs` | `Source` | 1 |
| `src/diag.rs` | `Diagnostic`, `Severity`, `sort` | 1 |
| `src/number.rs` | `Number` | 2 |
| `src/lexer/mod.rs`, `src/lexer/token.rs`, `src/lexer/scan.rs`, `src/lexer/layout.rs` | Tokens | 3, 4 |
| `src/ast/mod.rs`, `src/ast/sexpr.rs`, `src/ast/print.rs`, `src/ast/stmt.rs`, `src/ast/program.rs` | AST, test printer, source printer | 5, 6, 9, 11 |
| `src/parser/mod.rs`, `src/parser/expr.rs`, `src/parser/stmt.rs`, `src/parser/top.rs` | Parser | 7, 8, 9, 10, 11 |
| `src/check/*.rs` | Checks | 12 to 18 |
| `src/format/mod.rs`, `src/format/wrap.rs` | Format | 19, 20 |
| `src/cli.rs`, `tests/cli.rs` | CLI | 21 |
| `tests/spec_examples.rs` | Spec guard | 22 |

---

## Task 1: Crate, CI, `Source`, `Diagnostic`  (est. 120 lines / 8 files)

**Files:**
- Create: `Cargo.toml`, `.gitignore`, `.github/workflows/ci.yml`, `src/lib.rs`, `src/main.rs`, `src/source.rs`, `src/diag.rs`

**Interfaces:**
- Produces:
  - `source::Source::new(&str) -> Source`
  - `Source::line(&self, n: u32) -> &str`, for 1-based lines; it returns `""` when out of range
  - `Source::line_count(&self) -> u32`
  - `diag::Severity { Error, Warning }`
  - `diag::Diagnostic { severity, line: Option<u32>, source_line: String, problem: String, suggestion: Option<String> }`
  - `Diagnostic::error(&Source, u32, impl Into<String>)`
  - `Diagnostic::warning(&Source, u32, impl Into<String>)`
  - `Diagnostic::program(impl Into<String>)`
  - `Diagnostic::suggest(self, impl Into<String>) -> Self`
  - `Diagnostic::render(&self) -> String`
  - `Diagnostic::is_error(&self) -> bool`
  - `diag::sort(&mut Vec<Diagnostic>)`

- [ ] **Step 1: Scaffold.** Use this `Cargo.toml`:

```toml
[package]
name = "gloss"
version = "0.0.1"
edition = "2024"
rust-version = "1.87"
publish = false

[dependencies]
```

`.gitignore`:

```
/target
.superpowers/
.worktrees/
```

`.github/workflows/ci.yml`, which skips drafts (owner rule):

```yaml
name: ci
on:
  pull_request:
    types: [opened, synchronize, reopened, ready_for_review]
jobs:
  test:
    if: github.event.pull_request.draft == false
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - run: rustup toolchain install stable --profile minimal -c clippy,rustfmt
      - run: cargo fmt --check
      - run: cargo clippy --all-targets -- -D warnings
      - run: cargo test
```

`src/lib.rs`:

```rust
pub mod diag;
pub mod source;
```

`src/main.rs`:

```rust
fn main() {}
```

- [ ] **Step 2: Write the failing tests** in `src/diag.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::source::Source;

    #[test]
    fn renders_spec_example_exactly() {
        let src = Source::new("feature start\n  x\n  set total as 0\n");
        let d = Diagnostic::error(&src, 3, "set needs to").suggest("set total to 0");
        assert_eq!(d.render(), "line 3: set total as 0. set needs to. Did you mean set total to 0?");
    }

    #[test]
    fn renders_warning_and_program_level() {
        let src = Source::new("a\nb\nc\n  set totl to 5\n");
        let w = Diagnostic::warning(&src, 4, "totl is set but never used");
        assert_eq!(w.render(), "line 4: warning: set totl to 5. totl is set but never used.");
        let p = Diagnostic::program("This program has no feature start, so it has nowhere to begin");
        assert_eq!(p.render(), "This program has no feature start, so it has nowhere to begin.");
    }

    #[test]
    fn sorts_program_first_then_line_then_errors_first() {
        let src = Source::new("a\nb\nc\n");
        let mut v = vec![
            Diagnostic::warning(&src, 2, "w2"),
            Diagnostic::error(&src, 3, "e3"),
            Diagnostic::error(&src, 2, "e2"),
            Diagnostic::program("p"),
        ];
        sort(&mut v);
        let order: Vec<&str> = v.iter().map(|d| d.problem.as_str()).collect();
        assert_eq!(order, ["p", "e2", "w2", "e3"]);
    }

    #[test]
    fn source_handles_crlf_and_out_of_range() {
        let src = Source::new("one\r\ntwo\r\n");
        assert_eq!(src.line(1), "one");
        assert_eq!(src.line(2), "two");
        assert_eq!(src.line(99), "");
    }
}
```

- [ ] **Step 2b: Run** `cargo test`. Expected: it fails to compile, because `Diagnostic` is not defined.
- [ ] **Step 3: Implement.**
  - `Source` stores the lines from `text.split('\n')`, each with a trailing `\r` stripped.
  - `render` follows [D7].
  - `sort` is a stable sort with the key `(line.map_or(0, |n| n as u64 + 1), severity)`, where `Error < Warning` by derive order.
- [ ] **Step 4: Run** the verify gate. Expected: 4 tests pass.
- [ ] **Step 5: Commit** with the message `Add crate, CI, Source and Diagnostic`.

**How this could pass vacuously:** `render` could hard-code the spec string. The warning and program-level tests use different shapes, which rules that out.

**Review model:** sonnet.

**Risks:** CI YAML syntax. A reviewer checks the `if:` line and the `ready_for_review` trigger.

---

## Task 2: `Number`  (est. 140 lines / 2 files)

**Files:**
- Create: `src/number.rs`
- Modify: `src/lib.rs` (add `pub mod number;`)

**Interfaces:**
- Produces:
  - `number::Number(pub i64)`, counting ten-thousandths. It derives `Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash`.
  - `number::NumberError { Malformed, TooManyPlaces, OutOfRange }`
  - `Number::parse_literal(&str) -> Result<Number, NumberError>`: no sign
  - `Number::parse_text(&str) -> Result<Number, NumberError>`: the `as number` rules
  - `Number::show(self) -> String`
  - `Number::with_places(self, places: u8) -> String`
  - `number::SCALE: i64 = 10_000`

- [ ] **Step 1: Failing tests** in `src/number.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use NumberError::*;
    fn n(v: i64) -> Number { Number(v) }

    #[test]
    fn literals() {
        assert_eq!(Number::parse_literal("120.50"), Ok(n(1_205_000)));
        assert_eq!(Number::parse_literal("42"), Ok(n(420_000)));
        assert_eq!(Number::parse_literal("007"), Ok(n(70_000)));
        assert_eq!(Number::parse_literal("0.12345"), Err(TooManyPlaces));
        assert_eq!(Number::parse_literal("5."), Err(Malformed));
        assert_eq!(Number::parse_literal(".5"), Err(Malformed));
        assert_eq!(Number::parse_literal("-5"), Err(Malformed));
        assert_eq!(Number::parse_literal("922337203685477.5807"), Ok(n(i64::MAX)));
        assert_eq!(Number::parse_literal("922337203685477.5808"), Err(OutOfRange));
        assert_eq!(Number::parse_literal("99999999999999999999999"), Err(OutOfRange));
    }

    #[test]
    fn text_follows_as_number_rules() {
        assert_eq!(Number::parse_text(" -7 "), Ok(n(-70_000)));
        assert_eq!(Number::parse_text("0.5"), Ok(n(5_000)));
        assert_eq!(Number::parse_text("-922337203685477.5808"), Ok(n(i64::MIN)));
        for bad in ["+3", "1,200", "1e3", ".5", "", "-", "- 3", "12a", "1.2.3"] {
            assert_eq!(Number::parse_text(bad), Err(Malformed), "{bad:?}");
        }
        assert_eq!(Number::parse_text("0.12345"), Err(TooManyPlaces));
    }

    #[test]
    fn show_drops_trailing_zeros() {
        assert_eq!(n(1_205_000).show(), "120.5");
        assert_eq!(n(1_200_000).show(), "120");
        assert_eq!(n(-35_000).show(), "-3.5");
        assert_eq!(n(3_333).show(), "0.3333");
        assert_eq!(n(0).show(), "0");
        assert_eq!(n(i64::MIN).show(), "-922337203685477.5808");
    }

    #[test]
    fn with_places_rounds_half_away_from_zero() {
        assert_eq!(n(1_205_000).with_places(2), "120.50");
        assert_eq!(n(1_205_000).with_places(0), "121");
        assert_eq!(n(-1_205_000).with_places(0), "-121");
        assert_eq!(n(3_333).with_places(2), "0.33");
        assert_eq!(n(-4_000).with_places(0), "0");
        assert_eq!(n(0).with_places(4), "0.0000");
        assert_eq!(n(i64::MAX).with_places(0), "922337203685478");
    }
}
```

- [ ] **Step 2: Run** `cargo test number`. Expected: it fails to compile.
- [ ] **Step 3: Implement:**

```rust
pub const SCALE: i64 = 10_000;

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Number(pub i64);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NumberError { Malformed, TooManyPlaces, OutOfRange }

impl Number {
    pub fn parse_literal(s: &str) -> Result<Number, NumberError> { parse_digits(s, false) }

    pub fn parse_text(s: &str) -> Result<Number, NumberError> {
        let t = s.trim_matches(' ');
        match t.strip_prefix('-') {
            Some(rest) => parse_digits(rest, true),
            None => parse_digits(t, false),
        }
    }

    pub fn show(self) -> String {
        let v = self.0 as i128;
        let (whole, frac) = (v.abs() / 10_000, v.abs() % 10_000);
        let mut s = if v < 0 { String::from("-") } else { String::new() };
        s.push_str(&whole.to_string());
        if frac != 0 {
            s.push('.');
            s.push_str(format!("{frac:04}").trim_end_matches('0'));
        }
        s
    }

    pub fn with_places(self, places: u8) -> String {
        assert!(places <= 4, "places is checked by the parser");
        let v = self.0 as i128;
        let unit = 10i128.pow(4 - places as u32);
        let rounded = (v.abs() + unit / 2) / unit;
        let p = 10i128.pow(places as u32);
        let mut s = if v < 0 && rounded != 0 { String::from("-") } else { String::new() };
        s.push_str(&(rounded / p).to_string());
        if places > 0 {
            s.push('.');
            s.push_str(&format!("{:0w$}", rounded % p, w = places as usize));
        }
        s
    }
}

fn parse_digits(s: &str, negative: bool) -> Result<Number, NumberError> {
    use NumberError::*;
    let (whole, frac) = match s.split_once('.') {
        Some((w, f)) => (w, Some(f)),
        None => (s, None),
    };
    let digits = |t: &str| !t.is_empty() && t.bytes().all(|b| b.is_ascii_digit());
    if !digits(whole) { return Err(Malformed); }
    let frac = match frac {
        None => "",
        Some(f) if !digits(f) => return Err(Malformed),
        Some(f) if f.len() > 4 => return Err(TooManyPlaces),
        Some(f) => f,
    };
    let mut v: i128 = 0;
    for b in whole.bytes() {
        v = v * 10 + i128::from(b - b'0');
        if v > i128::from(i64::MAX) { return Err(OutOfRange); }
    }
    let mut f: i128 = frac.bytes().fold(0, |a, b| a * 10 + i128::from(b - b'0'));
    for _ in frac.len()..4 { f *= 10; }
    let total = v * 10_000 + f;
    i64::try_from(if negative { -total } else { total }).map(Number).map_err(|_| OutOfRange)
}
```

  A whole part greater than `i64::MAX` stops early with `OutOfRange`, before it is scaled.

- [ ] **Step 4: Run** the verify gate. Expected: the 4 new tests pass.
- [ ] **Step 5: Commit** with the message `Add fixed-point Number`.

**How this could pass vacuously:** rounding could truncate instead. `121`, `-121` and `-4_000 → "0"` (no minus sign on zero) each fail under truncation or a sign bug.

**Review model:** sonnet.

**Risks:** the `-0` sign case and i128 overflow. Both are covered by the tests.

---

## Task 3: Tokens and line scanning  (est. 140 lines / 4 files)

**Files:**
- Create: `src/lexer/mod.rs`, `src/lexer/token.rs`, `src/lexer/scan.rs`
- Modify: `src/lib.rs` (add `pub mod lexer;`)

**Interfaces:**
- Consumes: `Source`, `Diagnostic`, `Number`.
- Produces, in `token.rs`, re-exported from `lexer`:
  - `pub enum Kw` with exactly these variants, in alphabetical order:
    `And Append As Ask At By Clear Count Divide Each Empty Expect Feature First For From Give Greater If In Input Into Is Item Last Least Less List Load More Most Multiply Needs No Not Note Number Of Or Otherwise Places Record Remainder Repeat Run Save Set Show Shown Skip Stop Subtract Table Test Text Than To Truth While Whole With Yes`
  - `Kw::ALL: [Kw; 62]`
  - `Kw::from_word(&str) -> Option<Kw>`, which matches lowercase only
  - `Kw::as_str(self) -> &'static str`
  - `Kw::is_fact(self) -> bool`, true for `Count First Last Item Whole Remainder Shown`
  - `pub enum Sym { Plus, Minus, Star, Slash, LParen, RParen, Comma }`
  - `pub enum Tok { Kw(Kw), Name(String), RecordName(String), Number(Number), Text(String), Sym(Sym), Newline, Indent, Dedent, Note(String), NoteBlock(Vec<String>) }`, deriving `Clone, Debug, PartialEq`
  - `impl Display for Tok`. `Kw` prints its word, `Name` and `RecordName` their text, `Number` its show form, `Text` quoted with `""` doubling, `Sym` its symbol, `Newline` prints `the end of the line`, and `Indent` and `Dedent` print `a change of indentation`. Notes print `note`.
  - `pub struct Token { pub tok: Tok, pub line: u32, pub blank_before: bool }`
  - In `scan.rs`: `pub fn scan_line(src: &Source, line: u32, text: &str) -> Result<Vec<Token>, Diagnostic>`. Every token it returns has `blank_before: false`.

- [ ] **Step 1: Failing tests** in `src/lexer/scan.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::lexer::{Kw, Sym, Tok};
    use crate::number::Number;

    fn scan(text: &str) -> Vec<Tok> {
        scan_line(&Source::new(text), 1, text).unwrap().into_iter().map(|t| t.tok).collect()
    }
    fn problem(text: &str) -> String { scan_line(&Source::new(text), 1, text).unwrap_err().problem }

    #[test]
    fn words_numbers_symbols() {
        assert_eq!(scan("set total to total + 1"), vec![
            Tok::Kw(Kw::Set), Tok::Name("total".into()), Tok::Kw(Kw::To),
            Tok::Name("total".into()), Tok::Sym(Sym::Plus), Tok::Number(Number(10_000)),
        ]);
        assert_eq!(scan("Expense with item_2 (x9)"), vec![
            Tok::RecordName("Expense".into()), Tok::Kw(Kw::With), Tok::Name("item_2".into()),
            Tok::Sym(Sym::LParen), Tok::Name("x9".into()), Tok::Sym(Sym::RParen),
        ]);
        assert_eq!(scan("120.50"), vec![Tok::Number(Number(1_205_000))]);
    }

    #[test]
    fn text_with_doubled_quotes() {
        assert_eq!(scan(r#""say ""hi""""#), vec![Tok::Text(r#"say "hi""#.into())]);
        assert_eq!(scan(r#""" , "a,b""#), vec![
            Tok::Text(String::new()), Tok::Sym(Sym::Comma), Tok::Text("a,b".into()),
        ]);
    }

    #[test]
    fn errors() {
        assert_eq!(problem(r#"show "abc"#), "Text is missing its closing quote");
        assert_eq!(problem("if a = 3"), "= is not part of Gloss. Comparisons are written in words, such as is");
        assert_eq!(problem("if a < 3"), "< is not part of Gloss. Comparisons are written in words, such as is less than");
        assert_eq!(problem("set x to 0.12345"), "Numbers have at most four decimal places");
        assert_eq!(problem("set x to 99999999999999999"), "This number is outside the range Gloss can hold, about 922 trillion either way");
        assert_eq!(problem("set x to 2nd"), "Put a space between a number and a word");
        assert_eq!(problem("set x to 5."), "5. is not a number Gloss can read");
        assert_eq!(problem("show\t1"), "Tabs are not allowed; use spaces");
        assert_eq!(problem("show 1 # 2"), "# is not part of Gloss");
    }

    #[test]
    fn keyword_list_matches_language_spec() {
        let spec = include_str!("../../spec/v0.md");
        let words: Vec<&str> = spec.lines()
            .skip_while(|l| !l.contains("Keywords can never be used as names"))
            .nth(1).unwrap().trim().trim_matches('`').split(' ').collect();
        let facts: Vec<&str> = spec.lines()
            .skip_while(|l| !l.contains("**Built-in fact names**"))
            .nth(1).unwrap().trim().trim_matches('`').split(' ').collect();
        assert_eq!(words.len() + facts.len(), Kw::ALL.len());
        for w in words.iter().chain(facts.iter()) {
            let k = Kw::from_word(w).unwrap_or_else(|| panic!("{w} missing from Kw"));
            assert_eq!(k.as_str(), *w);
            assert_eq!(k.is_fact(), facts.contains(w), "{w}");
        }
    }
}
```

- [ ] **Step 2: Run** `cargo test lexer`. Expected: it fails to compile.
- [ ] **Step 3: Implement** `scan_line` as a byte-index loop over `text`:
  - **Space** is skipped. **Tab** gives the tab error.
  - **A letter** reads `[A-Za-z0-9_]*`. Then:
    - if `Kw::from_word` matches, it is `Tok::Kw`
    - otherwise, a first character in `A-Z` makes it `RecordName`
    - otherwise it is `Name`
  - **A digit** reads `[0-9.]*` and calls `Number::parse_literal`.
    - `Malformed` gives `"{lexeme} is not a number Gloss can read"`.
    - `TooManyPlaces` and `OutOfRange` give the messages in the test.
    - If the next character is a letter or `_`, the error is `Put a space between a number and a word`.
  - **`"`** reads to the next `"`. A `""` pair inside appends one `"`. Reaching the end of the line gives the missing-quote error.
  - **`+ - * / ( ) ,`** map to `Sym`.
  - **`=`, `<` and `>`** give their messages. For `>` the message ends `such as is greater than`.
  - **Any other character** `c` gives `"{c} is not part of Gloss"`.
  - Diagnostics are `Diagnostic::error(src, line, ..)`.
- [ ] **Step 4: Run** the verify gate. Expected: the 4 new tests pass.
- [ ] **Step 5: Commit** with the message `Add tokens and line scanner`.

**How this could pass vacuously:** `Kw` could miss a reserved word. The spec-reading test compares against the language spec's own list. `Kw::ALL.len()` must equal 55 keywords plus 7 facts, which is 62.

**Review model:** sonnet.

**Risks:** if the language spec's reserved-word line moves or changes format, the test fails loudly. That is intended.

---

## Task 4: Layout: indentation, notes, continuation  (est. 140 lines / 2 files)

**Files:**
- Create: `src/lexer/layout.rs`
- Modify: `src/lexer/mod.rs` (`pub fn lex`)

**Interfaces:**
- Consumes: `scan_line`, `Token`, `Tok`.
- Produces: `lexer::lex(src: &Source) -> Result<Vec<Token>, Diagnostic>`.
  - Each logical line yields its tokens and then `Newline`.
  - `Indent` and `Dedent` come before the first token of a line whose level changes.
  - The first token of a logical line has `blank_before = true` when one or more blank lines precede it and it is not the first line of its block.
  - Every open level is closed with `Dedent` at the end.

- [ ] **Step 1: Failing tests** in `layout.rs`:

```rust
#[cfg(test)]
mod tests {
    use crate::lexer::{lex, Kw, Sym, Tok};
    use crate::source::Source;

    fn kinds(s: &str) -> Vec<Tok> { lex(&Source::new(s)).unwrap().into_iter().map(|t| t.tok).collect() }
    fn problem(s: &str) -> (Option<u32>, String) {
        let d = lex(&Source::new(s)).unwrap_err();
        (d.line, d.problem)
    }

    #[test]
    fn indent_and_dedent() {
        assert_eq!(kinds("feature start\n  show \"hi\"\n"), vec![
            Tok::Kw(Kw::Feature), Tok::Name("start".into()), Tok::Newline,
            Tok::Indent, Tok::Kw(Kw::Show), Tok::Text("hi".into()), Tok::Newline, Tok::Dedent,
        ]);
        let t = kinds("feature a\n  if x\n    stop\nfeature b\n  stop\n");
        assert_eq!(t.iter().filter(|t| **t == Tok::Dedent).count(), 3);
        assert_eq!(t.iter().filter(|t| **t == Tok::Indent).count(), 3);
    }

    #[test]
    fn blank_lines_are_flagged_not_emitted() {
        let toks = lex(&Source::new("feature a\n  stop\n\n\n  skip\n")).unwrap();
        let skip = toks.iter().find(|t| t.tok == Tok::Kw(Kw::Skip)).unwrap();
        assert!(skip.blank_before);
        let stop = toks.iter().find(|t| t.tok == Tok::Kw(Kw::Stop)).unwrap();
        assert!(!stop.blank_before);
    }

    #[test]
    fn notes() {
        assert_eq!(kinds("note smallest  program\n"), vec![Tok::Note("smallest  program".into()), Tok::Newline]);
        assert_eq!(kinds("note\n  Walks the list.\n\n    Indented more.\nfeature x\n"), vec![
            Tok::NoteBlock(vec!["Walks the list.".into(), "".into(), "  Indented more.".into()]),
            Tok::Newline, Tok::Kw(Kw::Feature), Tok::Name("x".into()), Tok::Newline,
        ]);
        assert_eq!(kinds("notes\n")[0], Tok::Name("notes".into()));
    }

    #[test]
    fn continuation_after_comma() {
        let t = kinds("feature a\n  show \"a\",\n    \"b\"\n");
        assert_eq!(&t[3..], &[
            Tok::Indent, Tok::Kw(Kw::Show), Tok::Text("a".into()), Tok::Sym(Sym::Comma),
            Tok::Text("b".into()), Tok::Newline, Tok::Dedent,
        ]);
    }

    #[test]
    fn layout_errors() {
        assert_eq!(problem("feature a\n\tshow 1\n"), (Some(2), "Tabs are not allowed; indent with two spaces".into()));
        assert_eq!(problem("feature a\n   show 1\n"), (Some(2), "Indentation must be a multiple of two spaces".into()));
        assert_eq!(problem("feature a\n    show 1\n"), (Some(2), "This line is indented more than one level deeper than the line above".into()));
        assert_eq!(problem("feature a\n  show 1,\n  2\n"), (Some(2), "This line ends with a comma, so the next line must continue it, indented one level deeper".into()));
        assert_eq!(problem("feature a\n  show 1,\n"), (Some(2), "This line ends with a comma, so the next line must continue it, indented one level deeper".into()));
    }
}
```

- [ ] **Step 2: Run** `cargo test layout`. Expected: it fails to compile.
- [ ] **Step 3: Implement.** Walk physical lines `i = 1..=line_count`.
  - **Blank lines** (only spaces) set a pending-blank flag and are skipped.
  - **Indentation.** Count the leading spaces. A tab among them is an error. An odd count is an error. Otherwise `level = spaces / 2`.
    - `level > current + 1` is an error.
    - `level == current + 1` pushes `Indent`.
    - A lower level pushes `current - level` copies of `Dedent`.
    - The pending-blank flag becomes `blank_before`, unless an `Indent` was just pushed.
  - **Notes.** If the trimmed line is `note` exactly, take the following lines while they are blank or have more than `spaces` leading spaces. For each, strip `spaces + 2` leading characters, or make it `""` when blank. Drop trailing empty entries and emit `NoteBlock`.
    - Trimmed lines starting `note ` become `Note(rest)`, where `rest` is everything after `note `, keeping inner spacing.
  - **Other lines** call `scan_line`.
  - **Continuation.** While the last token is `Sym(Comma)`, take the next non-blank line. It must have `level == this line's level + 1`, or it is an error reported on the comma's line. Append its tokens.
  - Push `Newline` after each logical line, and the remaining `Dedent`s at the end of the file.
- [ ] **Step 4: Run** the verify gate. Expected: the 5 new tests pass.
- [ ] **Step 5: Commit** with the message `Add layout: indentation, notes, continuation`.

**How this could pass vacuously:** a lexer that never emits `Dedent` would pass `indent_and_dedent` only if the counts happened to match. The test counts both, across a two-level drop.

**Review model:** sonnet.

**Risks:** `Indent` and `Dedent` pairing at the end of the file. The tests check it.

---

## Task 5: Expression AST and `sexpr`  (est. 140 lines / 3 files)

**Files:**
- Create: `src/ast/mod.rs`, `src/ast/sexpr.rs`
- Modify: `src/lib.rs` (add `pub mod ast;`)

**Interfaces:**
- Produces (exact):

```rust
pub type Line = u32;
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)] pub struct ExprId(pub u32);
#[derive(Clone, Debug, PartialEq)] pub struct Expr { pub id: ExprId, pub line: Line, pub kind: ExprKind }
#[derive(Clone, Copy, Debug, PartialEq, Eq)] pub enum PlainType { Number, Text, Truth }
#[derive(Clone, Debug, PartialEq, Eq)] pub enum ItemType { Plain(PlainType), Record(String) }
#[derive(Clone, Debug, PartialEq, Eq)] pub enum Type { Item(ItemType), List(ItemType) }
#[derive(Clone, Copy, Debug, PartialEq, Eq)] pub enum Fact { Count, First, Last, Whole }
#[derive(Clone, Copy, Debug, PartialEq, Eq)] pub enum Conversion { ToNumber, ToText, ToTextPlaces(u8) }
#[derive(Clone, Copy, Debug, PartialEq, Eq)] pub enum ArithOp { Add, Sub, Mul, Div }
#[derive(Clone, Copy, Debug, PartialEq, Eq)] pub enum CompareOp { Is, IsNot, Greater, Less, AtLeast, AtMost }
#[derive(Clone, Debug, PartialEq)]
pub enum ExprKind {
    Number(crate::number::Number), Text(String), Truth(bool), Name(String), Shown,
    EmptyList(ItemType),
    Paren(Box<Expr>),
    FieldOf { field: String, of: Box<Expr> },
    Fact { fact: Fact, of: Box<Expr> },
    ItemOf { position: Box<Expr>, of: Box<Expr> },
    Remainder { of: Box<Expr>, by: Box<Expr> },
    Negate(Box<Expr>),
    Convert { value: Box<Expr>, to: Conversion },
    Binary { op: ArithOp, left: Box<Expr>, right: Box<Expr> },
    Call { feature: String, inputs: Vec<Expr> },
    BuildRecord { record: String, fields: Vec<(String, Expr)> },
    BuildList(Vec<Expr>),
    Compare { op: CompareOp, left: Box<Expr>, right: Box<Expr> },
    IsNumber { value: Box<Expr>, negated: bool },
    Not(Box<Expr>),
    And(Vec<Expr>),
    Or(Vec<Expr>),
}
```

  - `ast::sexpr::sexpr(&Expr) -> String`
  - `ast::item_type_text(&ItemType) -> String`, giving `number`, `text`, `truth` or the record name
  - `ast::type_text(&Type) -> String`, which adds `list of ` for lists

- [ ] **Step 1: Failing test** in `sexpr.rs`. It builds expressions by hand with a helper `e(kind)`, using id 0 and line 1:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::ast::*;
    use crate::number::Number;
    fn e(kind: ExprKind) -> Expr { Expr { id: ExprId(0), line: 1, kind } }
    fn bx(kind: ExprKind) -> Box<Expr> { Box::new(e(kind)) }
    fn name(s: &str) -> ExprKind { ExprKind::Name(s.into()) }

    #[test]
    fn prints_every_kind() {
        use ExprKind::*;
        let cases: Vec<(ExprKind, &str)> = vec![
            (Number(Number(1_205_000)), "120.5"),
            (Text("a\"b".into()), "\"a\"\"b\""),
            (Truth(true), "yes"),
            (Shown, "shown"),
            (EmptyList(ItemType::Record("Student".into())), "(empty-list Student)"),
            (Paren(bx(name("a"))), "(paren a)"),
            (FieldOf { field: "amount".into(), of: bx(name("x")) }, "(of amount x)"),
            (Fact { fact: crate::ast::Fact::Whole, of: bx(name("x")) }, "(whole x)"),
            (ItemOf { position: bx(Number(Number(20_000))), of: bx(name("xs")) }, "(item 2 xs)"),
            (Remainder { of: bx(name("n")), by: bx(Number(Number(20_000))) }, "(remainder n 2)"),
            (Negate(bx(name("x"))), "(- x)"),
            (Convert { value: bx(name("t")), to: Conversion::ToTextPlaces(2) }, "(as-text-2 t)"),
            (Binary { op: ArithOp::Mul, left: bx(name("a")), right: bx(name("b")) }, "(* a b)"),
            (Call { feature: "f".into(), inputs: vec![e(name("a")), e(name("b"))] }, "(call f a b)"),
            (BuildRecord { record: "E".into(), fields: vec![("name".into(), e(name("n")))] }, "(record E (name n))"),
            (BuildList(vec![e(name("a"))]), "(list a)"),
            (Compare { op: CompareOp::AtLeast, left: bx(name("a")), right: bx(name("b")) }, "(at-least a b)"),
            (IsNumber { value: bx(name("c")), negated: true }, "(is-not-number c)"),
            (Not(bx(name("a"))), "(not a)"),
            (Or(vec![e(name("a")), e(name("b"))]), "(or a b)"),
        ];
        for (k, want) in cases { assert_eq!(sexpr(&e(k)), want); }
    }
}
```

- [ ] **Step 2: Run** the test. It must fail. **Step 3: Implement** `sexpr`.
  - The remaining names follow the test's pattern:
    - comparisons: `is`, `is-not`, `greater`, `less`, `at-least`, `at-most`
    - conversions: `as-number`, `as-text`, `as-text-N`
    - facts: `count`, `first`, `last`, `whole`
    - `and`, and `(is-number x)`
  - Numbers print with `Number::show`.
  - Text prints quoted, with inner quotes doubled.
- [ ] **Step 4: Run** the verify gate. **Step 5: Commit** with the message `Add expression AST and sexpr`.

**How this could pass vacuously:** an arm could be added only for the kinds that appear in the test. The table covers every `ExprKind` variant except `Truth(false)`, `Name`, `Compare` with other ops and `And`. Add those to the table during implementation: one row per remaining variant and op.

**Review model:** sonnet.

---

## Task 6: Expression source printer  (est. 120 lines / 2 files)

**Files:**
- Create: `src/ast/print.rs`
- Modify: `src/ast/mod.rs` (add `pub mod print;`)

**Interfaces:**
- Produces: `ast::print::expr(&Expr) -> String`. This is the expression in Gloss source form. It never adds parentheses: `Paren` nodes print as `(…)`.

- [ ] **Step 1: Failing tests.** Use the same helpers as Task 5.
  - Expected outputs:
    - `120.5`, `"say ""hi"""`, `yes`, `no`, `shown`, `empty list of Student`, `(a)`
    - `amount of x`, `count of xs`, `item 2 of xs`, `remainder of n by 2`, `-x`
    - `cost as number`, `t as text`, `t as text with 2 places`
    - `a + b`, `a - b`, `a * b`, `a / b`
    - `f with a, b`
    - `Expense with name "L", amount 120`
    - `list of 4, 9`
    - `a is b`, `a is not b`, `a is greater than b`, `a is less than b`, `a is at least b`, `a is at most b`
    - `c is number`, `c is not number`
    - `not a`, `a and b and c`, `a or b`
  - Plus one nested case:

    `Binary{Add, Paren(Binary{Mul,a,b}), FieldOf{amount, Name x}}` → `(a * b) + amount of x`

- [ ] **Step 2: Run**; it must fail. **Step 3: Implement** a `match` that recurses with no precedence logic. **Step 4: Run** the verify gate. **Step 5: Commit** with the message `Add expression source printer`.

**How this could pass vacuously:** the printer could drop `Paren`. The nested case requires `(a * b)`.

**Review model:** sonnet.

---

## Task 7: Parser, part 1: values up to arithmetic  (est. 140 lines / 3 files)

**Files:**
- Create: `src/parser/mod.rs`, `src/parser/expr.rs`
- Modify: `src/lib.rs` (add `pub mod parser;`)

**Interfaces:**
- Consumes: `lex`, `Token`, `Tok`, the AST types, `print::expr`.
- Produces:

```rust
pub struct Parser<'a> { toks: &'a [Token], pos: usize, src: &'a Source, next_id: u32 }
impl<'a> Parser<'a> {
    pub fn new(toks: &'a [Token], src: &'a Source) -> Self;
    pub fn peek(&self) -> Option<&Tok>;
    pub fn peek_at(&self, ahead: usize) -> Option<&Tok>;
    pub fn line(&self) -> u32;                        // line of the current token, else the last one
    pub fn bump(&mut self) -> Option<Token>;
    pub fn eat_kw(&mut self, k: Kw) -> bool;
    pub fn eat_sym(&mut self, s: Sym) -> bool;
    pub fn err(&self, problem: impl Into<String>) -> Diagnostic;
    pub fn mk(&mut self, line: u32, kind: ExprKind) -> Expr;   // assigns the next ExprId
    pub fn parse_arith(&mut self) -> Result<Expr, Diagnostic>;
    pub fn parse_item_type(&mut self) -> Result<ItemType, Diagnostic>;
}
pub fn parse_expr_str(text: &str) -> Result<Expr, Diagnostic>;  // calls parse_expr (Task 8), then requires Newline
```

  In this task, `parse_expr_str` calls `parse_arith`. Task 8 switches it to `parse_expr`.

- [ ] **Step 1: Failing tests** in `expr.rs`:

```rust
#[cfg(test)]
mod tests {
    use crate::ast::sexpr::sexpr;
    use crate::parser::parse_expr_str;
    fn sx(t: &str) -> String { sexpr(&parse_expr_str(t).unwrap()) }
    fn perr(t: &str) -> String { parse_expr_str(t).unwrap_err().problem }

    #[test]
    fn precedence_and_facts() {
        assert_eq!(sx("1 + 2 * 3"), "(+ 1 (* 2 3))");
        assert_eq!(sx("a - b - c"), "(- (- a b) c)");
        assert_eq!(sx("count of prices + 1"), "(+ (count prices) 1)");
        assert_eq!(sx("name of item 2 of students"), "(of name (item 2 students))");
        assert_eq!(sx("item (i + 1) of prices"), "(item (paren (+ i 1)) prices)");
        assert_eq!(sx("item i of prices"), "(item i prices)");
        assert_eq!(sx("-amount of x"), "(- (of amount x))");
        assert_eq!(sx("-7.5"), "(- 7.5)");
        assert_eq!(sx("cost as number"), "(as-number cost)");
        assert_eq!(sx("total as text with 2 places"), "(as-text-2 total)");
        assert_eq!(sx("remainder of n by 2"), "(remainder n 2)");
        assert_eq!(sx("2 * whole of x"), "(* 2 (whole x))");
        assert_eq!(sx("(whole of a) / b"), "(/ (paren (whole a)) b)");
        assert_eq!(sx("whole of (a / b)"), "(whole (paren (/ a b)))");
        assert_eq!(sx("empty list of Student"), "(empty-list Student)");
        assert_eq!(sx("shown"), "shown");
        assert_eq!(sx("no"), "no");
    }

    #[test]
    fn ambiguity_and_errors() {
        assert_eq!(perr("whole of a / b"), "whole of a / b can be read two ways. Write (whole of a) / b or whole of (a / b)");
        assert_eq!(perr("remainder of a by b + 1"), "remainder of a by b + 1 can be read two ways. Write (remainder of a by b) + 1 or remainder of a by (b + 1)");
        assert_eq!(perr("2 * whole of x + 1"), "whole of x + 1 can be read two ways. Write (whole of x) + 1 or whole of (x + 1)");
        assert_eq!(perr("item i + 1 of prices"), "item needs of after its position; a position that is worked out goes in parentheses, as in item (i + 1) of prices");
        assert_eq!(perr("total as text with 5 places"), "The number of places is a digit from 0 to 4");
        assert_eq!(perr("count prices"), "count needs of, as in count of prices");
        assert_eq!(perr("1 +"), "+ needs a value after it");
        assert_eq!(perr("(1 + 2"), "This ( has no matching )");
        assert_eq!(perr("1 2"), "Unexpected 2 after the expression");
    }
}
```

- [ ] **Step 2: Run**; it must fail.
- [ ] **Step 3: Implement** one function per grammar level in Appendix A: `primary`, `fact`, `signed`, `converted`, `product`, `arith`.
  - **`fact`:**
    - `Name` followed by `Kw::Of` is `FieldOf`, with the rest parsed recursively by `fact`.
    - `Kw::Count | First | Last | Whole` followed by `of` is `Fact`.
    - `item` reads its position (a number, a name, or `(expr)`), then `of`, then `fact`. Anything else after the position gives the `item needs of` error. Build the example from the actual tokens: `item (i + 1) of prices`.
    - `remainder of fact by fact`.
  - **Ambiguity check.** Before consuming an operator in `product` or `arith`, test `ends_with_number_fact(&left)`:
    - It is true for `Fact{Whole}` and `Remainder`, and recursively through `Negate(x)` and the `right` of `Binary`.
    - When true, parse the right operand, then build the message with `print::expr`.
    - The fact node is the innermost one found on the right edge.
    - Whole: `"{F} {op} {R} can be read two ways. Write ({F}) {op} {R} or whole of ({X} {op} {R})"`, where `X` is the fact's operand.
    - Remainder: `... or remainder of {A} by ({B} {op} {R})`.
  - **`primary`:**
    - `RecordName` alone is an error: `{Name} is a record; build one with {Name} with ...`.
    - `Name` or `RecordName` followed by `Kw::With` is an error: `{name} with ... must be in parentheses here, because it is inside arithmetic or a comma list`.
- [ ] **Step 4: Run** the verify gate. **Step 5: Commit** with the message `Parse arithmetic, facts and conversions`.

**How this could pass vacuously:** precedence could be right for `+`/`*` but wrong for `-` associativity. `a - b - c` pins left associativity.

**Review model:** sonnet.

**Risks:** building the ambiguity message from the AST must print the user's own spelling. `print::expr` does that, because the AST keeps `Paren`.

---

## Task 8: Parser, part 2: open forms, comparisons, logic  (est. 130 lines / 1 file)

**Files:**
- Modify: `src/parser/expr.rs`

**Interfaces:**
- Produces:
  - `Parser::parse_expr(&mut self) -> Result<Expr, Diagnostic>`
  - `Parser::parse_value(&mut self) -> Result<Expr, Diagnostic>`
  - `parse_expr_str` now uses `parse_expr`.

- [ ] **Step 1: Failing tests**, appended to the tests module:

```rust
    #[test]
    fn open_forms_and_comparisons() {
        assert_eq!(sx(r#"category_total with "food" is 420"#), r#"(is (call category_total "food") 420)"#);
        assert_eq!(sx("f with a, b"), "(call f a b)");
        assert_eq!(sx("f with a + 1"), "(call f (+ a 1))");
        assert_eq!(sx("f with (x is 3)"), "(call f (paren (is x 3)))");
        assert_eq!(sx(r#"Expense with name "L", amount 120"#), r#"(record Expense (name "L") (amount 120))"#);
        assert_eq!(sx("list of 4, 9, 2"), "(list 4 9 2)");
        assert_eq!(sx(r#"list of (S with n "A", m 1), (S with n "B", m 2)"#),
            r#"(list (paren (record S (n "A") (m 1))) (paren (record S (n "B") (m 2))))"#);
        assert_eq!(sx("cost is not number"), "(is-not-number cost)");
        assert_eq!(sx("cost is number"), "(is-number cost)");
        assert_eq!(sx("a is not b"), "(is-not a b)");
        assert_eq!(sx("a is greater than b"), "(greater a b)");
        assert_eq!(sx("a is less than b"), "(less a b)");
        assert_eq!(sx("a is at least 3"), "(at-least a 3)");
        assert_eq!(sx("a is at most 3"), "(at-most a 3)");
        assert_eq!(sx("remainder of n by 2 is 0"), "(is (remainder n 2) 0)");
        assert_eq!(sx("not a and b"), "(and (not a) b)");
        assert_eq!(sx("a or b or c"), "(or a b c)");
        assert_eq!(sx("(a and b) or c"), "(or (paren (and a b)) c)");
        assert_eq!(sx("is_even with n"), "(call is_even n)");
    }

    #[test]
    fn logic_errors() {
        assert_eq!(perr("a is b is c"), "Comparisons cannot be chained; join two comparisons with and");
        assert_eq!(perr("a and b or c"), "and and or cannot be mixed without parentheses. Write (a and b) or c, or a and (b or c)");
        assert_eq!(perr("a is greater b"), "is greater needs than, as in a is greater than b");
        assert_eq!(perr("a is at 3"), "is at needs least or most");
        assert_eq!(perr("1 + f with a"), "f with ... must be in parentheses here, because it is inside arithmetic or a comma list");
        assert_eq!(perr("list of f with a"), "f with ... must be in parentheses here, because it is inside arithmetic or a comma list");
        assert_eq!(perr("Expense with 3"), "Expense with needs a field name, as in Expense with name ...");
    }
```

- [ ] **Step 2: Run**; it must fail.
- [ ] **Step 3: Implement.**
  - **`parse_value`.** If the next tokens are `Name, Kw::With`, parse a call: inputs are `parse_arith` separated by commas. `RecordName, With` builds a record from `Name parse_arith` pairs. `Kw::List, Kw::Of` (but not `empty list`) builds a list of `parse_arith` items. Otherwise it calls `parse_arith`.
  - **`comparison`.** `left = parse_value()`. On `Kw::Is`:
    - `not` followed by `number` is `IsNumber{negated: true}`; `not` alone is `IsNot`.
    - `number` is `IsNumber`.
    - `greater than` and `less than`.
    - `at least` and `at most`.
    - Otherwise it is `Is`.

    Then the right side is `parse_value()`. If another `is` follows a comparison, that is the chaining error.
  - **`negation`.** An optional `not`, then `comparison`.
  - **`parse_expr`.** Read negations joined by `and` or `or`. Seeing the other word is the mixing error, built with `print::expr` from the parts read so far. Parse the remaining part before building the message: `c` in the test.
- [ ] **Step 4: Run** the verify gate. **Step 5: Commit** with the message `Parse open forms, comparisons and logic`.

**How this could pass vacuously:** `not` could bind looser than `and`. `not a and b` pins it to `(and (not a) b)`.

**Review model:** sonnet.

---

## Task 9: Statement AST and simple statements  (est. 140 lines / 4 files)

**Files:**
- Create: `src/ast/stmt.rs`, `src/parser/stmt.rs`
- Modify: `src/ast/mod.rs`, `src/parser/mod.rs`

**Interfaces:**
- Produces:

```rust
#[derive(Clone, Debug, PartialEq)] pub enum Note { Line(String), Block(Vec<String>) }
#[derive(Clone, Debug, PartialEq)]
pub enum Target { Var(String), Field { field: String, of: Box<Target> }, Item { position: Box<Expr>, of: Box<Target> } }
#[derive(Clone, Debug, PartialEq)] pub struct Stmt { pub line: Line, pub blank_before: bool, pub kind: StmtKind }
#[derive(Clone, Debug, PartialEq)]
pub enum StmtKind {
    Note(Note),
    Set { target: Target, value: Expr },
    Add { value: Expr, target: Target },
    Subtract { value: Expr, target: Target },
    Multiply { target: Target, by: Expr },
    Divide { target: Target, by: Expr },
    Append { value: Expr, list: String },
    Show(Vec<Expr>),
    Ask { question: Expr, into: String },
    If { branches: Vec<(Expr, Vec<Stmt>)>, otherwise: Option<Vec<Stmt>> },
    ForEachIn { var: String, list: Expr, body: Vec<Stmt> },
    ForEachRange { var: String, from: Expr, to: Expr, body: Vec<Stmt> },
    RepeatWhile { cond: Expr, body: Vec<Stmt> },
    Skip, Stop,
    Give(Expr),
    Run { feature: String, inputs: Vec<Expr> },
    Load(String), Save(String), Clear(String),
    Expect(Expr),
    Input(Vec<Expr>),
}
```

  - `Target::root(&self) -> &str`, the variable or table name the target starts from
  - `ast::sexpr::stmt(&Stmt) -> String`
  - `Parser::parse_stmt(&mut self) -> Result<Stmt, Diagnostic>`
  - `parser::parse_stmts_str(text: &str) -> Result<Vec<Stmt>, Diagnostic>`, which parses statements until the end of the file, at level 0

- [ ] **Step 1: Failing tests** in `parser/stmt.rs`:

```rust
#[cfg(test)]
mod tests {
    use crate::ast::sexpr::stmt;
    use crate::parser::parse_stmts_str;
    fn ss(t: &str) -> Vec<String> { parse_stmts_str(t).unwrap().iter().map(stmt).collect() }
    fn perr(t: &str) -> String { let d = parse_stmts_str(t).unwrap_err(); d.render() }

    #[test]
    fn simple_statements() {
        assert_eq!(ss("set total to 0\nadd price to total\nsubtract d from total\nmultiply total by 2\ndivide total by people"),
            ["(set total 0)", "(add price total)", "(subtract d total)", "(multiply total 2)", "(divide total people)"]);
        assert_eq!(ss("set amount of item 3 of expenses to 150"), ["(set (of amount (item 3 expenses)) 150)"]);
        assert_eq!(ss(r#"append Expense with name "L", amount 1 to expenses"#), [r#"(append (record Expense (name "L") (amount 1)) expenses)"#]);
        assert_eq!(ss(r#"show "Total: ", (f with kind), total"#), [r#"(show "Total: " (paren (call f kind)) total)"#]);
        assert_eq!(ss(r#"ask "How much?" into cost"#), [r#"(ask "How much?" cost)"#]);
        assert_eq!(ss("skip\nstop\ngive total\nrun report with a, b\nrun start"),
            ["skip", "stop", "(give total)", "(run report a b)", "(run start)"]);
        assert_eq!(ss("load expenses\nsave expenses\nclear expenses"), ["(load expenses)", "(save expenses)", "(clear expenses)"]);
        assert_eq!(ss(r#"expect total is 420
input "Lunch", "120"
note hi there"#), ["(expect (is total 420))", r#"(input "Lunch" "120")"#, r#"(note "hi there")"#]);
    }

    #[test]
    fn statement_errors() {
        assert_eq!(perr("set total as 0"), "line 1: set total as 0. set needs to. Did you mean set total to 0?");
        assert_eq!(perr("total = 0"), "line 1: total = 0. = is not part of Gloss. Comparisons are written in words, such as is.");
        assert_eq!(perr("total to 0"), "line 1: total to 0. Every statement starts with a keyword, such as set or show; total is not one.");
        assert_eq!(perr("add 1 into total"), "line 1: add 1 into total. add needs to, as in add 1 to total.");
        assert_eq!(perr(r#"show "a", f with b"#), "line 1: show \"a\", f with b. f with ... must be in parentheses here, because it is inside arithmetic or a comma list.");
        assert_eq!(perr("run f with"), "line 1: run f with. with needs at least one input after it.");
        assert_eq!(perr("set Total to 1"), "line 1: set Total to 1. Variable names start with a lowercase letter. Did you mean set total to 1?");
    }
}
```

- [ ] **Step 2: Run**; it must fail.
- [ ] **Step 3: Implement.**
  - Dispatch on the first token: `Kw::Set`, `Kw::Add` and so on. `Note` and `NoteBlock` tokens become `StmtKind::Note`.
  - Each statement must end with `Newline`. Anything else gives `Unexpected {tok} after the statement`.
  - **`set` with `as`.** If the token after the target is `Kw::As`, the problem is `set needs to`, with the suggestion `src.line(l).trim()` with the first ` as ` replaced by ` to `.
  - **Other missing words** give `{verb} needs {word}, as in {example}`. The example is the source line with the wrong word replaced, or the word inserted after the value.
  - **`stmt` sexpr** follows the test's shapes. Blocks come in Task 10.
- [ ] **Step 4: Run** the verify gate. **Step 5: Commit** with the message `Parse simple statements`.

**How this could pass vacuously:** `parse_stmts_str` could stop after the first statement. Every test parses several statements.

**Review model:** sonnet.

---

## Task 10: Block statements  (est. 120 lines / 2 files)

**Files:**
- Modify: `src/parser/stmt.rs`, `src/ast/sexpr.rs`

**Interfaces:**
- Produces: `Parser::parse_block(&mut self, owner: &str) -> Result<Vec<Stmt>, Diagnostic>`. It expects `Newline`, then `Indent`, then statements up to `Dedent`. The block sexpr shapes are:
  - `(if C [S ...] C2 [S ...] else [S ...])`
  - `(for-each x L [S ...])`
  - `(for-range i A B [S ...])`
  - `(repeat C [S ...])`

  Statements inside `[...]` are separated by single spaces.

- [ ] **Step 1: Failing tests:**

```rust
    #[test]
    fn blocks() {
        let p = "if a is 1\n  show 1\notherwise if a is 2\n  show 2\notherwise\n  show 3\n  stop";
        assert_eq!(ss(p), ["(if (is a 1) [(show 1)] (is a 2) [(show 2)] else [(show 3) stop])"]);
        assert_eq!(ss("for each price in prices\n  add price to total"), ["(for-each price prices [(add price total)])"]);
        assert_eq!(ss("for each i from 1 to count of xs\n  skip"), ["(for-range i 1 (count xs) [skip])"]);
        assert_eq!(ss("repeat while cost is not number\n  ask \"Again\" into cost"), [r#"(repeat (is-not-number cost) [(ask "Again" cost)])"#]);
        assert_eq!(ss("if a\n  if b\n    stop\nshow 1"), ["(if a [(if b [stop])])", "(show 1)"]);
    }

    #[test]
    fn block_errors() {
        assert_eq!(perr("if a\nshow 1"), "line 1: if a. if needs an indented block under it.");
        assert_eq!(perr("otherwise\n  show 1"), "line 1: otherwise. otherwise must follow an if block.");
        assert_eq!(perr("for each x of xs\n  stop"), "line 1: for each x of xs. for each needs in or from, as in for each x in xs.");
        assert_eq!(perr("repeat until a\n  stop"), "line 1: repeat until a. repeat needs while, as in repeat while a.");
    }
```

- [ ] **Step 2: Run**; it must fail. **Step 3: Implement.** `otherwise` branches are read only directly after an `if` block, at the same level. A stray `otherwise` gives the error. **Step 4: Run** the verify gate. **Step 5: Commit** with the message `Parse block statements`.

**How this could pass vacuously:** nested blocks could swallow the following statement. The last `blocks` case checks that `show 1` stays outside.

**Review model:** sonnet.

---

## Task 11: Top-level items and `parse`  (est. 140 lines / 4 files)

**Files:**
- Create: `src/ast/program.rs`, `src/parser/top.rs`
- Modify: `src/ast/mod.rs`, `src/lib.rs`

**Interfaces:**
- Produces:

```rust
#[derive(Clone, Debug, PartialEq)] pub struct Program { pub items: Vec<Item> }
#[derive(Clone, Debug, PartialEq)]
// `attached` is true when no blank line separates the note from the next item, so Format keeps them together.
pub enum Item { Note { line: Line, note: Note, attached: bool }, Record(RecordDecl), Table(TableDecl), Feature(FeatureDecl), Test(TestDecl) }
#[derive(Clone, Debug, PartialEq)] pub enum RecordEntry { Field(FieldDecl), Note(Note) }
#[derive(Clone, Debug, PartialEq)] pub struct RecordDecl { pub line: Line, pub name: String, pub entries: Vec<RecordEntry> }
#[derive(Clone, Debug, PartialEq)] pub struct FieldDecl { pub line: Line, pub name: String, pub ty: PlainType }
#[derive(Clone, Debug, PartialEq)] pub struct TableDecl { pub line: Line, pub name: String, pub record: String, pub path: String }
#[derive(Clone, Debug, PartialEq)] pub struct InputDecl { pub line: Line, pub name: String, pub ty: Type }
#[derive(Clone, Debug, PartialEq)] pub struct FeatureDecl { pub line: Line, pub name: String, pub needs: Vec<InputDecl>, pub body: Vec<Stmt> }
#[derive(Clone, Debug, PartialEq)] pub struct TestDecl { pub line: Line, pub title: String, pub body: Vec<Stmt> }
impl RecordDecl { pub fn fields(&self) -> impl Iterator<Item = &FieldDecl>; }
impl Program {
    pub fn records(&self) -> impl Iterator<Item = &RecordDecl>;
    pub fn tables(&self) -> impl Iterator<Item = &TableDecl>;
    pub fn features(&self) -> impl Iterator<Item = &FeatureDecl>;
    pub fn tests(&self) -> impl Iterator<Item = &TestDecl>;
}
```

  - `parser::parse(toks: &[Token], src: &Source) -> Result<Program, Diagnostic>`
  - `gloss::parse_source(src: &Source) -> Result<Program, Diagnostic>`, in `lib.rs`
  - Design spec Q3 default: `note` lines are allowed in record bodies. Notes are also allowed before `needs`.

- [ ] **Step 1: Failing tests** in `top.rs`. `EXPENSES` is the complete example in §13 of the language spec, copied verbatim into the test file as a `const &str`:

```rust
#[cfg(test)]
mod tests {
    use crate::ast::*;
    use crate::source::Source;
    const EXPENSES: &str = "..."; // paste §13 of spec/v0.md verbatim
    fn parse(t: &str) -> Result<Program, crate::diag::Diagnostic> { crate::parse_source(&Source::new(t)) }

    #[test]
    fn parses_complete_example() {
        let p = parse(EXPENSES).unwrap();
        assert_eq!(p.items.len(), 11);
        let r = p.records().next().unwrap();
        assert_eq!(r.fields().map(|f| f.name.as_str()).collect::<Vec<_>>(), ["name", "amount", "category"]);
        let t = p.tables().next().unwrap();
        assert_eq!((t.name.as_str(), t.record.as_str(), t.path.as_str()), ("expenses", "Expense", "expenses.csv"));
        let ct = p.features().find(|f| f.name == "category_total").unwrap();
        assert_eq!(ct.needs, vec![InputDecl { line: 11, name: "wanted".into(), ty: Type::Item(ItemType::Plain(PlainType::Text)) }]);
        assert_eq!(p.tests().count(), 4);
        assert_eq!(p.features().find(|f| f.name == "start").unwrap().body.len(), 9);
    }

    #[test]
    fn list_input_types_and_record_notes() {
        let p = parse("record S\n  note the name\n  name as text\nfeature f\n  note why\n  needs xs as list of S, n as number\n  give n\n").unwrap();
        let f = p.features().next().unwrap();
        assert_eq!(f.needs[0].ty, Type::List(ItemType::Record("S".into())));
        assert_eq!(f.body.len(), 2);
    }

    #[test]
    fn top_level_errors() {
        let e = |t: &str| parse(t).unwrap_err().render();
        assert_eq!(e("show 1\n"), "line 1: show 1. Only record, table, feature, test and note can start a line at the top level.");
        assert_eq!(e("record expense\n  a as text\n"), "line 1: record expense. Record names start with a capital letter. Did you mean record Expense?");
        assert_eq!(e("feature F\n  stop\n"), "line 1: feature F. Feature names start with a lowercase letter. Did you mean feature f?");
        assert_eq!(e("feature f\n  show 1\n  needs a as number\n"), "line 3: needs a as number. needs must be the first line of a feature.");
        assert_eq!(e("record E\n  a as list of number\n"), "line 2: a as list of number. A record field is a number, text or truth.");
        assert_eq!(e("table t of E in data.csv\n"), "line 1: table t of E in data.csv. The file name is written in quotes. Did you mean table t of E in \"data.csv\"?");
        assert_eq!(e("test category total\n  stop\n"), "line 1: test category total. A test title is a sentence in quotes. Did you mean test \"category total\"?");
    }
}
```

- [ ] **Step 2: Run**; it must fail. **Step 3: Implement** with `parse_block` for bodies. `needs` after a non-note statement gives the error. **Step 4: Run** the verify gate. **Step 5: Commit** with the message `Parse top-level items`.

**How this could pass vacuously:** the item count alone would pass a parser that dropped bodies. The test also asserts `start` has 9 statements and checks the `needs` line number.

**Review model:** sonnet.

---

## Task 12: Checker scaffold and program-level names (security boundary)  (est. 140 lines / 4 files)

**Files:**
- Create: `src/check/mod.rs`, `src/check/names.rs`, `src/check/similar.rs`
- Modify: `src/lib.rs`

**Interfaces:**
- Produces:

```rust
pub struct CheckResult { pub diagnostics: Vec<Diagnostic>, pub types: Option<TypeInfo> }
#[derive(Default)] pub struct TypeInfo { pub expr_types: Vec<Option<Ty>>, pub features: std::collections::BTreeMap<String, FeatureInfo> }
pub struct FeatureInfo { pub inputs: Vec<(String, Ty)>, pub result: Ty, pub vars: Vec<(String, Ty, u32)> }
#[derive(Clone, Debug, PartialEq, Eq)] pub enum Ty { Number, Text, Truth, Record(String), List(Box<Ty>), Table(String), Nothing, Error }
pub fn check(program: &Program, src: &Source) -> CheckResult;
pub struct Ctx<'a> { pub program: &'a Program, pub src: &'a Source, pub diags: Vec<Diagnostic> }
```

  - `similar::nearest<'a>(name: &str, candidates: impl Iterator<Item = &'a str>) -> Option<&'a str>`. It returns the closest candidate within edit distance 2, preferring the smallest distance, then source order. It returns `None` when `name.len() < 3`.
  - `similar::distance(a: &str, b: &str) -> usize`, the Levenshtein distance
  - `gloss::check_source(text: &str) -> Vec<Diagnostic>`, in `lib.rs`. It parses, checks, sorts, and returns either the parse error alone or the check diagnostics.
  - Test helper used from now on: `fn diags(src: &str) -> Vec<String>`, which is `check_source(src)` rendered.

- [ ] **Step 1: Failing tests** in `names.rs`:

```rust
#[cfg(test)]
mod tests {
    fn diags(s: &str) -> Vec<String> { crate::check_source(s).iter().map(|d| d.render()).collect() }
    const START: &str = "feature start\n  show 1\n";

    #[test]
    fn start_rules() {
        assert_eq!(diags("feature main\n  show 1\n"), ["This program has no feature start, so it has nowhere to begin."]);
        assert_eq!(diags("feature start\n  needs a as number\n  show 1\n"),
            ["line 2: needs a as number. start cannot have a needs line; it is where the program begins."]);
    }

    #[test]
    fn duplicates_and_case() {
        let p = format!("{START}feature report\n  show 1\nfeature report\n  show 1\n");
        assert_eq!(diags(&p), ["line 5: feature report. report is already used on line 3 for a feature."]);
        let p = format!("{START}feature myTotal\n  show 1\nfeature mytotal\n  show 1\n");
        assert_eq!(diags(&p), ["line 5: feature mytotal. mytotal and myTotal differ only in case; rename one."]);
        let p = format!("{START}record E\n  a as text\n  a as number\n");
        assert_eq!(diags(&p), ["line 5: a as number. E already has a field named a."]);
        let p = format!("{START}test \"x\"\n  show 1\ntest \"x\"\n  show 1\n");
        assert_eq!(diags(&p), ["line 5: test \"x\". Another test already has this title (line 3)."]);
    }

    #[test]
    fn table_paths_are_a_security_boundary() {
        let t = |path: &str| diags(&format!("{START}record E\n  a as text\ntable t of E in \"{path}\"\n"));
        assert_eq!(t("/etc/passwd"), [r#"line 5: table t of E in "/etc/passwd". The file name must not start with /; table files are found from the folder the program runs in."#]);
        assert_eq!(t("../x.csv"), [r#"line 5: table t of E in "../x.csv". The file name must not use ..; table files stay inside the folder the program runs in."#]);
        assert_eq!(t("data/../../x.csv"), [r#"line 5: table t of E in "data/../../x.csv". The file name must not use ..; table files stay inside the folder the program runs in."#]);
        assert_eq!(t("a//b.csv"), [r#"line 5: table t of E in "a//b.csv". The file name has an empty part; write folders as data/expenses.csv."#]);
        assert_eq!(t(""), [r#"line 5: table t of E in "". The file name has an empty part; write folders as data/expenses.csv."#]);
        assert_eq!(t("data/"), [r#"line 5: table t of E in "data/". The file name has an empty part; write folders as data/expenses.csv."#]);
        assert_eq!(t(r"a\b.csv"), [r#"line 5: table t of E in "a\b.csv". Use / between folders, not \."#]);
        assert_eq!(t("./x.csv"), [r#"line 5: table t of E in "./x.csv". The file name must not use . as a folder; write x.csv."#]);
        assert!(t("data/expenses.csv").is_empty());
        let two = format!("{START}record E\n  a as text\ntable t of E in \"x.csv\"\ntable u of E in \"x.csv\"\n");
        assert_eq!(diags(&two), [r#"line 6: table u of E in "x.csv". x.csv is already the file of table t (line 5); one file belongs to one table."#]);
    }

    #[test]
    fn unknown_records_suggest() {
        let p = format!("{START}record Expense\n  a as text\ntable t of Expens in \"x.csv\"\n");
        assert_eq!(diags(&p), [r#"line 5: table t of Expens in "x.csv". There is no record named Expens. Did you mean Expense?"#]);
    }
}
```

  Note on `./x.csv`: a `.` part is rejected too, so that every file has exactly one spelling (rule 2) and paths can be compared. This makes explicit a rule the language spec implies but does not state. Record it as ruling R1 in the ledger and in the PR description.

- [ ] **Step 2: Run**; it must fail.
- [ ] **Step 3: Implement** `names::check_program(&mut Ctx)`:
  - Collect program-level names in source order into a `Vec<(name, kind, line)>`.
  - Report repeated names at the second line.
  - Report pairs that are equal when lowercased, unless one is a record and the other differs only in the first letter.
  - Check fields, test titles, table paths and duplicate files.
  - For paths, split on `/` and test, in this order:
    1. leading `/`
    2. a `\` anywhere
    3. any `..` part
    4. any `.` part
    5. any empty part
  - `check` runs `names` and then, from later tasks, the other passes. It sorts the diagnostics. `types` is `Some` only when no errors were found.
- [ ] **Step 4: Run** the verify gate. **Step 5: Commit** with the message `Check program-level names and table paths`.

**How this could pass vacuously:** a path check using `contains("..")` would wrongly reject `a..b.csv`, a legal name. Add `assert!(t("a..b.csv").is_empty());`. A check using `starts_with("..")` would miss `data/../../x.csv`, which is covered.

**Review model:** **opus for the first review** (security boundary), sonnet for follow-ups.

**Risks:** Unicode lookalikes in paths are accepted as ordinary characters. The run-time `openat2` check in M3 is the second line of defence.

---

## Task 13: Expression typing, part 1  (est. 140 lines / 3 files)

**Files:**
- Create: `src/check/types.rs`, `src/check/expr.rs`
- Modify: `src/check/mod.rs`

**Interfaces:**
- Consumes: `Ctx`, `Ty`, `nearest`.
- Produces:
  - `Ty::describe(&self) -> String`. It gives `number`, `text`, `truth`, the record name, `list of X`, `table expenses`, and `nothing` for `Nothing`.
  - `Ty::item(&self) -> Option<Ty>`, the item type of a `List` or `Table`
  - `pub struct Env<'a> { pub vars: &'a BTreeMap<String, Ty>, pub results: &'a BTreeMap<String, Ty> }`. `results` maps each feature to its result type. A result that is still `Ty::Error` is being worked out, and it produces no diagnostics.
  - `expr::type_of(ctx: &mut Ctx, env: &Env, e: &Expr, types: &mut Vec<Option<Ty>>) -> Ty`. It records `types[e.id] = Some(ty)`, growing the vector as needed. It returns `Ty::Error` after reporting, and never reports twice for the same cause: an `Error` operand stays silent.
  - In this task it covers: literals, `Name`, `Shown`, `EmptyList`, `Paren`, `FieldOf`, `Fact`, `ItemOf`, `Remainder`, `Negate`, `Convert` and `Binary`.
  - Names resolve in this order:
    1. variables in `env.vars`
    2. tables, which give `Ty::Table`
    3. features with no inputs, which give `env.results[name]`
    4. otherwise an error

- [ ] **Step 1: Failing tests** in `expr.rs`. They drive `type_of` through a tiny program whose `start` body is checked by a temporary walker in `check/mod.rs`: for each `Show` item, call `type_of` with the variables collected so far from `Set` statements. Task 14 replaces this walker.

```rust
#[cfg(test)]
mod tests {
    fn diags(s: &str) -> Vec<String> { crate::check_source(s).iter().map(|d| d.render()).collect() }
    fn body(lines: &str) -> Vec<String> {
        diags(&format!("record E\n  amount as number\n  name as text\ntable es of E in \"e.csv\"\nfeature start\n{lines}"))
    }

    #[test]
    fn values_and_facts() {
        assert!(body("  set n to 1\n  show n + 2, count of es, amount of first of es, whole of n, -n\n").is_empty());
        assert_eq!(body("  show totl\n"), ["line 6: show totl. totl has no value."]);
        assert_eq!(body("  set total to 1\n  show total, totl\n"), ["line 7: show total, totl. totl has no value. Did you mean total?"]);
        assert_eq!(body("  set e to first of es\n  show amout of e\n"), ["line 7: show amout of e. E has no field amout. Did you mean amount?"]);
        assert_eq!(body("  set n to 1\n  show amount of n\n"), ["line 7: show amount of n. n is a number, so it has no field amount."]);
        assert_eq!(body("  set n to 1\n  show count of n\n"), ["line 7: show count of n. count of works on lists and tables; n is a number."]);
        assert_eq!(body("  show whole of \"a\"\n"), ["line 6: show whole of \"a\". whole of works on numbers; \"a\" is text."]);
        assert_eq!(body("  show item \"a\" of es\n"), ["line 6: show item \"a\" of es. The position in item must be a number; \"a\" is text."]);
    }

    #[test]
    fn arithmetic_and_conversion() {
        assert_eq!(body("  show 1 + \"a\"\n"), ["line 6: show 1 + \"a\". + needs numbers on both sides; \"a\" is text."]);
        assert_eq!(body("  show 1 as number\n"), ["line 6: show 1 as number. as number converts text; 1 is already a number."]);
        assert_eq!(body("  show \"a\" as text\n"), ["line 6: show \"a\" as text. \"a\" is already text."]);
        assert_eq!(body("  show \"a\" as text with 2 places\n"), ["line 6: show \"a\" as text with 2 places. with places works on numbers; \"a\" is text."]);
        assert_eq!(body("  show first of es as text\n"), ["line 6: show first of es as text. Only numbers and truths can be converted to text; first of es is E."]);
        assert!(body("  show yes as text, 1 as text with 2 places, \"3\" as number\n").is_empty());
        assert_eq!(body("  show 1 + (2 + \"a\")\n"), ["line 6: show 1 + (2 + \"a\"). + needs numbers on both sides; \"a\" is text."]);
    }
}
```

  The last case asserts a single diagnostic: the outer `+` sees an `Error` operand and stays silent.

- [ ] **Step 2: Run**; it must fail. **Step 3: Implement.** Messages quote operands with `print::expr`. **Step 4: Run** the verify gate. **Step 5: Commit** with the message `Type values, facts, arithmetic and conversions`.

**How this could pass vacuously:** cascade suppression could hide real errors. `show totl` and the field typo assert that the first error is reported.

**Review model:** sonnet.

---

## Task 14: Expression typing, part 2  (est. 140 lines / 1 file)

**Files:**
- Modify: `src/check/expr.rs`

**Interfaces:**
- Produces: `type_of` now also covers `Compare`, `IsNumber`, `Not`, `And`, `Or`, `Call`, `BuildRecord` and `BuildList`.
  - Comparisons give `Truth`.
  - A call gives `env.results[feature]`.
  - A table may be passed where `list of R` is needed.

- [ ] **Step 1: Failing tests**, appended. They use the same `body` helper, plus `feature f` with `needs n as number, t as text` that gives `n`.

```rust
    fn with_f(lines: &str) -> Vec<String> {
        diags(&format!("record E\n  amount as number\n  name as text\ntable es of E in \"e.csv\"\nfeature f\n  needs n as number, t as text\n  give n\nfeature g\n  needs xs as list of E\n  give 1\nfeature start\n{lines}"))
    }

    #[test]
    fn comparisons_and_logic() {
        assert!(with_f("  show (1 is 2) and not (\"a\" is not \"b\"), (\"3\" is number)\n").is_empty());
        assert_eq!(with_f("  show 1 is \"a\"\n"), ["line 12: show 1 is \"a\". is compares two values of the same type; 1 is a number and \"a\" is text."]);
        assert_eq!(with_f("  show \"a\" is greater than \"b\"\n"), ["line 12: show \"a\" is greater than \"b\". is greater than compares numbers; \"a\" is text."]);
        assert_eq!(with_f("  show 1 is number\n"), ["line 12: show 1 is number. is number checks text; 1 is already a number."]);
        assert_eq!(with_f("  show 1 and yes\n"), ["line 12: show 1 and yes. and joins conditions; 1 is a number."]);
        assert_eq!(with_f("  show not 1\n"), ["line 12: show not 1. not works on a condition; 1 is a number."]);
    }

    #[test]
    fn calls_and_building() {
        assert!(with_f("  show (f with 1, \"a\"), (g with es), (g with list of (E with amount 1, name \"x\"))\n").is_empty());
        assert_eq!(with_f("  show (f with 1)\n"), ["line 12: show (f with 1). f needs 2 inputs: n and t; this call gives 1."]);
        assert_eq!(with_f("  show (f with \"a\", \"b\")\n"), ["line 12: show (f with \"a\", \"b\"). The input n of f is a number; \"a\" is text."]);
        assert_eq!(with_f("  show (h with 1)\n"), ["line 12: show (h with 1). There is no feature named h."]);
        assert_eq!(with_f("  set r to E with amount 1\n  show r\n"), ["line 12: set r to E with amount 1. Every field of E must be given; name is missing."]);
        assert_eq!(with_f("  show (E with amount 1, amount 2, name \"x\")\n"), ["line 12: show (E with amount 1, amount 2, name \"x\"). amount is given twice."]);
        assert_eq!(with_f("  show (E with amount \"1\", name \"x\")\n"), ["line 12: show (E with amount \"1\", name \"x\"). The field amount of E is a number; \"1\" is text."]);
        assert_eq!(with_f("  show (list of 1, \"a\")\n"), ["line 12: show (list of 1, \"a\"). Every item of a list has the same type; 1 is a number and \"a\" is text."]);
        assert_eq!(with_f("  show (list of (list of 1))\n"), ["line 12: show (list of (list of 1)). Lists of lists are not part of v0."]);
        assert_eq!(with_f("  show empty list of F\n"), ["line 12: show empty list of F. There is no record named F."]);
        assert_eq!(with_f("  show f\n"), ["line 12: show f. f needs 2 inputs: n and t, as in f with ...."]);
    }
```

- [ ] **Step 2: Run**; it must fail. **Step 3: Implement.** **Step 4: Run** the verify gate. **Step 5: Commit** with the message `Type comparisons, logic, calls and building`.

**How this could pass vacuously:** arity could be checked but input types not. The `"a"`-for-number case covers it.

**Review model:** sonnet.

---

## Task 15: Statements and variables  (est. 140 lines / 2 files)

**Files:**
- Create: `src/check/stmt.rs`
- Modify: `src/check/mod.rs` (replace the temporary walker)

**Interfaces:**
- Produces:
  - `stmt::check_body(ctx, results, inputs: &[(String, Ty)], body: &[Stmt], in_test: bool, types) -> FeatureInfo`
  - **Pre-pass.** Collect each variable's type at its first `set` or `ask`, in source order, into `vars`. A loop variable lives in a scope stack and only while its body is walked.
  - **Then type every statement.** At this stage `results` comes from a single source-order pass, where a feature's result is the type of its first `give` (or `Nothing`). Task 16 replaces this with a fixpoint.

- [ ] **Step 1: Failing tests** in `stmt.rs`. They use `diags` with full programs.

```rust
    const HEAD: &str = "record E\n  amount as number\ntable es of E in \"e.csv\"\n";
    fn start(lines: &str) -> Vec<String> { diags(&format!("{HEAD}feature start\n{lines}")) }

    // Every variable below is also read, so the unused-variable warning from Task 17 never adds noise.
    #[test]
    fn variables_have_fixed_types() {
        assert_eq!(start("  set total to 0\n  set total to \"a\"\n  show total\n"),
            ["line 6: set total to \"a\". total is a number, so it cannot be set to text (line 5 made it a number)."]);
        assert_eq!(start("  ask \"q\" into c\n  set c to 1\n  show c\n"),
            ["line 6: set c to 1. c is text, so it cannot be set to a number (line 5 made it text)."]);
        assert_eq!(start("  set es to 1\n"), ["line 5: set es to 1. es is a table; a variable cannot reuse its name."]);
    }

    #[test]
    fn verbs_match_types() {
        assert_eq!(start("  set xs to list of 1\n  add 5 to xs\n  show count of xs\n"),
            ["line 6: add 5 to xs. add works on numbers; to put an item at the end of xs, use append. Did you mean append 5 to xs?"]);
        assert_eq!(start("  set n to 1\n  append 5 to n\n  show n\n"), ["line 6: append 5 to n. append works on lists and tables; n is a number."]);
        assert_eq!(start("  append 5 to es\n"), ["line 5: append 5 to es. es holds E; 5 is a number."]);
        assert_eq!(start("  set n to 1\n  load n\n  show n\n"), ["line 6: load n. load works on tables; n is a number."]);
        assert_eq!(start("  if 1\n    show 1\n"), ["line 5: if 1. The condition of if must be yes or no; 1 is a number."]);
        assert_eq!(start("  for each x in 3\n    show x\n"), ["line 5: for each x in 3. for each … in walks a list or table; 3 is a number."]);
        assert_eq!(start("  ask 3 into c\n  show c\n"), ["line 5: ask 3 into c. The question must be text; 3 is a number."]);
    }

    #[test]
    fn placement_rules() {
        assert_eq!(start("  skip\n"), ["line 5: skip. skip can only be used inside a loop."]);
        assert_eq!(start("  expect 1 is 1\n"), ["line 5: expect 1 is 1. expect can only be used inside a test."]);
        assert_eq!(start("  input \"a\"\n"), ["line 5: input \"a\". input can only be used inside a test."]);
        assert_eq!(start("  show count of shown\n"), ["line 5: show count of shown. shown can only be used inside a test."]);
        assert_eq!(start("  set x to 1\n  show x\n  for each x in es\n    show 1\n"),
            ["line 7: for each x in es. x is already a variable in this feature; pick another name for the loop."]);
        assert!(start("  for each x in es\n    show amount of x\n  for each x from 1 to 3\n    show x\n").is_empty());
        assert!(diags(&format!("{HEAD}feature start\n  show 1\ntest \"t\"\n  input \"a\"\n  expect count of shown is 0\n")).is_empty());
    }
```

- [ ] **Step 2: Run**; it must fail. **Step 3: Implement.** **Step 4: Run** the verify gate. **Step 5: Commit** with the message `Type statements and variables`.

**How this could pass vacuously:** the loop-variable scope could leak, so a later reuse is wrongly accepted or rejected. The reuse case requires acceptance and the clash case requires rejection.

**Review model:** sonnet.

---

## Task 16: Feature results, every-path `give`, recursion, `run`  (est. 130 lines / 2 files)

**Files:**
- Create: `src/check/features.rs`
- Modify: `src/check/mod.rs`

**Interfaces:**
- Produces: `features::infer_results(ctx, program) -> BTreeMap<String, Ty>`.
  - Start with every result set to `Ty::Error`, meaning "being worked out".
  - Repeat until nothing changes, at most `features + 1` times: type each body silently, with diagnostics discarded. A feature's result is its first `give` whose type is not `Error`, or `Nothing` if it has no `give`.
  - Then do one final pass with diagnostics on.
  - Also check:
    - every `give` in a feature agrees
    - every path gives
    - the base case of recursion
    - `start` gives nothing
    - `run` targets a feature that gives nothing
    - a feature that gives nothing is not used as a value
  - `always_gives(body)` is true when the last non-note statement is `Give`, or an `If` with `otherwise` whose every branch `always_gives`.

- [ ] **Step 1: Failing tests:**

```rust
    #[test]
    fn results() {
        let p = "feature start\n  show (twice with 2)\nfeature twice\n  needs n as number\n  give n * 2\n";
        assert!(diags(p).is_empty());
        let rec = "feature start\n  show (down with 3)\nfeature down\n  needs n as number\n  if n is 0\n    give 0\n  give down with n - 1\n";
        assert!(diags(rec).is_empty());
        let mutual = "feature start\n  show (a with 1)\nfeature a\n  needs n as number\n  give b with n\nfeature b\n  needs n as number\n  give a with n\n";
        assert_eq!(diags(mutual), [
            "line 3: feature a. cannot work out what a gives; add a give that does not call a.",
            "line 6: feature b. cannot work out what b gives; add a give that does not call b.",
        ]);
    }

    #[test]
    fn give_rules() {
        let mixed = "feature start\n  show (f with 1)\nfeature f\n  needs n as number\n  if n is 1\n    give \"one\"\n  give n\n";
        assert_eq!(diags(mixed), ["line 7: give n. Every give in f must give the same type: line 6 gives text, this line gives a number."]);
        let partial = "feature start\n  show (f with 1)\nfeature f\n  needs n as number\n  if n is 1\n    give 1\n";
        assert_eq!(diags(partial), ["line 3: feature f. f can reach its end without giving a value."]);
        assert_eq!(diags("feature start\n  give 1\n"), ["line 2: give 1. start cannot give a value; it is where the program begins."]);
    }

    #[test]
    fn run_versus_value() {
        let p = "feature start\n  run total with \"food\"\nfeature total\n  needs k as text\n  give 1\n";
        assert_eq!(diags(p), ["line 2: run total with \"food\". total gives a number; use it, as in set result to total with \"food\"."]);
        let q = "feature start\n  show report\nfeature report\n  show 1\n";
        assert_eq!(diags(q), ["line 2: show report. report gives nothing, so it cannot be used as a value; use run report."]);
        let r = "feature start\n  run nothing_here\n";
        assert_eq!(diags(r), ["line 2: run nothing_here. There is no feature named nothing_here."]);
    }
```

- [ ] **Step 2: Run**; it must fail. **Step 3: Implement.** **Step 4: Run** the verify gate. **Step 5: Commit** with the message `Infer feature results and check give and run`.

**How this could pass vacuously:** single recursion might be inferred only by luck of source order. `down` is declared after `start`, which calls it.

**Review model:** sonnet.

---

## Task 17: Flow checks: definite values, unused, typos  (est. 140 lines / 2 files)

**Files:**
- Create: `src/check/flow.rs`
- Modify: `src/check/mod.rs`

**Interfaces:**
- Produces: `flow::check_flow(ctx, inputs, body)`.
  - Walk statements with the set of variables that definitely have a value.
    - An `if` without `otherwise` adds nothing after it.
    - An `if` with `otherwise` adds the intersection of its branches.
    - Loops add nothing after them.
  - Reading a variable that exists in the feature but may lack a value is the "may not have a value" error. It names where the variable is first set: `inside the if on line N`, or `inside the loop on line N`.
  - Then check unused variables, typos and case collisions.

- [ ] **Step 1: Failing tests:**

```rust
    #[test]
    fn definite_values() {
        let p = "feature start\n  if 1 is 1\n    set a to 1\n  show a\n";
        assert_eq!(diags(p), ["line 4: show a. a may not have a value here; it is first set inside the if on line 2. Set it before the if."]);
        let q = "feature start\n  for each i from 1 to 3\n    set last_i to i\n  show last_i\n";
        assert_eq!(diags(q), ["line 4: show last_i. last_i may not have a value here; it is first set inside the loop on line 2. Set it before the loop."]);
        let ok = "feature start\n  if 1 is 1\n    set a to 1\n  otherwise\n    set a to 2\n  show a\n";
        assert!(diags(ok).is_empty());
    }

    #[test]
    fn warnings() {
        assert_eq!(diags("feature start\n  set totl to 5\n"), ["line 2: warning: set totl to 5. totl is set but never used."]);
        let typo = "feature start\n  set total to 1\n  set totl to 2\n  show total, totl\n";
        assert_eq!(diags(typo), ["line 3: warning: set totl to 2. new name totl; did you mean total?"]);
        let plural = "feature start\n  set prices to list of 1, 2\n  for each price in prices\n    show price\n";
        assert!(diags(plural).is_empty());
        let short = "feature start\n  set ab to 1\n  set ac to 2\n  show ab, ac\n";
        assert!(diags(short).is_empty());
        let case = "feature start\n  set myTotal to 1\n  set mytotal to 2\n  show myTotal, mytotal\n";
        assert_eq!(diags(case), ["line 3: set mytotal to 2. mytotal and myTotal differ only in case; rename one."]);
    }
```

- [ ] **Step 2: Run**; it must fail. **Step 3: Implement.**
  - **Typo rule** (language spec §5): the new name has 4 or more characters, and some existing name in scope has distance 1. Existing names include variables created earlier, inputs, tables and features. Pairs equal except for one trailing `s` are exempt. Pairs that differ only in case are also exempt, because they are already the case-collision error.
- [ ] **Step 4: Run** the verify gate. **Step 5: Commit** with the message `Check definite values, unused variables and typos`.

**How this could pass vacuously:** the plural exemption could suppress all warnings. The `totl`/`total` case must still warn.

**Review model:** sonnet.

---

## Task 18: Mutation rules and rule-2 rejections  (est. 140 lines / 3 files)

**Files:**
- Create: `src/check/mutation.rs`, `src/check/rules.rs`
- Modify: `src/check/mod.rs`

**Interfaces:**
- Produces:
  - `rules::set_rewrite(stmt: &StmtKind) -> Option<StmtKind>`. It returns the `Add`, `Subtract`, `Multiply` or `Divide` form for the four patterns in language spec §7. Comparisons with the target are structural, ignoring `ExprId` and `line`.
  - `rules::truth_rewrite(e: &Expr) -> Option<ExprKind>`:
    - `X is yes` → `X`
    - `X is no` → `Not(X)`
    - `X is not yes` → `Not(X)`
    - `X is not no` → `X`
    - and the same with `yes` or `no` on the left
  - `mutation::check_mutation(ctx, program, results)` covers:
    - loop variables cannot be set
    - inputs are read-only
    - a list cannot change while a loop walks it, through the call graph for tables
    - a table that is saved but never loaded or cleared anywhere
    - `load` after `append` in the same block (a warning)
  - Format reuses both rewrite functions in Task 20.

- [ ] **Step 1: Failing tests:**

```rust
    const HEAD: &str = "record E\n  amount as number\ntable es of E in \"e.csv\"\n";

    #[test]
    fn rule_two() {
        assert_eq!(diags("feature start\n  set t to 1\n  set t to t + 2\n  show t\n"),
            ["line 3: set t to t + 2. set with + on its own target is written with add. Did you mean add 2 to t?"]);
        assert_eq!(diags("feature start\n  set t to 1\n  set t to 3 * t\n  show t\n"),
            ["line 3: set t to 3 * t. set with * on its own target is written with multiply. Did you mean multiply t by 3?"]);
        assert!(diags("feature start\n  set t to 1\n  set t to 3 - t\n  set t to t * 2 + 1\n  show t\n").is_empty());
        assert_eq!(diags("feature start\n  set d to yes\n  if d is yes\n    show 1\n"),
            ["line 3: if d is yes. A truth is used directly: write d instead of d is yes."]);
        assert_eq!(diags("feature start\n  set d to yes\n  if d is not yes\n    show 1\n"),
            ["line 3: if d is not yes. A truth is used directly: write not d instead of d is not yes."]);
    }

    #[test]
    fn loop_and_input_mutation() {
        assert_eq!(diags("feature start\n  set ns to list of 1\n  for each n in ns\n    add 1 to n\n"),
            ["line 4: add 1 to n. n stands for an item of ns and cannot be set. To change the item, loop with for each i from 1 to count of ns and set item i of ns. To work on a copy, set m to n first."]);
        assert_eq!(diags("feature start\n  for each i from 1 to 3\n    set i to 2\n"),
            ["line 3: set i to 2. i counts the loop and cannot be changed."]);
        assert!(diags(&format!("{HEAD}feature start\n  load es\n  for each e in es\n    set amount of e to 0\n  save es\n")).is_empty());
        assert_eq!(diags("feature start\n  run f with (list of 1)\nfeature f\n  needs ns as list of number\n  append 2 to ns\n"),
            ["line 5: append 2 to ns. ns is an input, so it cannot be changed. To work on a copy, set mine to ns first."]);
    }

    #[test]
    fn walking_and_saving() {
        let p = format!("{HEAD}feature start\n  load es\n  for each e in es\n    run add_one\n  save es\nfeature add_one\n  run put\nfeature put\n  append (E with amount 1) to es\n");
        assert_eq!(diags(&p), ["line 7: run add_one. es cannot change while the loop on line 6 walks it: add_one changes it (through put)."]);
        assert_eq!(diags(&format!("{HEAD}feature start\n  save es\n")),
            ["line 5: save es. es is saved here but nothing in this program loads it. Add load es before the first change, or clear es if you mean to start the file again."]);
        assert_eq!(diags(&format!("{HEAD}feature start\n  append (E with amount 1) to es\n  load es\n  save es\n")),
            ["line 6: warning: load es. load es replaces the rows appended on line 5."]);
    }
```

- [ ] **Step 2: Run**; it must fail. **Step 3: Implement.** **Step 4: Run** the verify gate. **Step 5: Commit** with the message `Check mutation rules and rule-2 forms`.

**How this could pass vacuously:**
- The call-graph check could look only one call deep. The test goes two deep: `add_one` → `put`.
- The save check could accept any `load` of any table. Add `assert_eq!(..)` for a program that loads table `fs` but saves `es`, expecting the save error.

**Review model:** sonnet.

---

## Task 19: Format: canonical printing  (est. 140 lines / 3 files)

**Files:**
- Create: `src/format/mod.rs`
- Modify: `src/lib.rs`, `src/parser/stmt.rs` (set `Stmt.blank_before` from the first token)

**Interfaces:**
- Produces:
  - `format::format_program(&Program) -> String`
    - Top-level items are separated by exactly one blank line, except that a top-level note with `attached == true` is printed directly above the next item, with no blank line between.
    - A statement with `blank_before` gets one blank line before it.
    - Indentation is two spaces per level.
    - Expressions print with `print::expr`.
  - `gloss::format_source(text: &str) -> Result<String, Diagnostic>`

- [ ] **Step 1: Failing tests** in `format/mod.rs`:

```rust
    fn fmt(s: &str) -> String { crate::format_source(s).unwrap() }

    #[test]
    fn canonical_layout() {
        let messy = "note top\nfeature start\n  set  total  to 1+2\n\n\n\n  show total ,\"x\"\nrecord E\n  a as text\n";
        assert_eq!(fmt(messy), "note top\nfeature start\n  set total to 1 + 2\n\n  show total, \"x\"\n\nrecord E\n  a as text\n");
    }

    #[test]
    fn idempotent_on_complete_example() {
        const EXPENSES: &str = "..."; // the same verbatim copy of §13 as in Task 11
        assert_eq!(fmt(EXPENSES), EXPENSES);
        assert_eq!(fmt(&fmt(EXPENSES)), fmt(EXPENSES));
    }

    #[test]
    fn note_blocks_round_trip() {
        let s = "note\n  Walks the list.\n\n    Indented more.\nfeature start\n  show 1\n";
        assert_eq!(fmt(s), s);
    }
```

- [ ] **Step 2: Run**; it must fail. **Step 3: Implement.** **Step 4: Run** the verify gate. **Step 5: Commit** with the message `Format: canonical printing`.

**How this could pass vacuously:** `format_source` could return its input unchanged. The `messy` case requires changes.

**Review model:** sonnet.

---

## Task 20: Format: rewrites and line wrapping  (est. 120 lines / 2 files)

**Files:**
- Create: `src/format/wrap.rs`
- Modify: `src/format/mod.rs`

**Interfaces:**
- Consumes: `rules::set_rewrite`, `rules::truth_rewrite`.
- Produces:
  - Rewrites are applied before printing.
  - A printed logical line longer than 100 characters is broken after commas. Each continuation line holds as many items as fit within 100 characters and is indented one level deeper. Commas inside text never count as break points.

- [ ] **Step 1: Failing tests:**

```rust
    #[test]
    fn rewrites() {
        assert_eq!(fmt("feature start\n  set t to 1\n  set t to t + 2\n  set t to 3 * t\n  show t\n"),
            "feature start\n  set t to 1\n  add 2 to t\n  multiply t by 3\n  show t\n");
        assert_eq!(fmt("feature start\n  set d to yes\n  if d is no\n    show 1\n"),
            "feature start\n  set d to yes\n  if not d\n    show 1\n");
    }

    #[test]
    fn wraps_long_lines_after_commas() {
        let long = format!("feature start\n  show {}\n", (1..=30).map(|i| format!("\"item{i:02}\"")).collect::<Vec<_>>().join(", "));
        let out = fmt(&long);
        assert!(out.lines().all(|l| l.len() <= 100), "{out}");
        assert!(out.lines().nth(2).unwrap().starts_with("    \"item"));
        assert_eq!(crate::parse_source(&crate::source::Source::new(&out)).unwrap(), crate::parse_source(&crate::source::Source::new(&long)).unwrap());
        assert_eq!(fmt(&out), out);
    }
```

  `Program` equality compares `ExprId`s and line numbers. Compare `format_program` output instead if the line numbers differ: `assert_eq!(fmt(&out), fmt(&long))`. Use that form for the final assertion.

- [ ] **Step 2: Run**; it must fail. **Step 3: Implement.** The printer marks break-eligible commas with `\u{1}`, and `wrap` splits on them. **Step 4: Run** the verify gate. **Step 5: Commit** with the message `Format: rewrites and line wrapping`.

**How this could pass vacuously:** the wrapper could break at a comma inside text. Add a case with `"a, b"` text items and assert they stay whole.

**Review model:** sonnet.

---

## Task 21: CLI: `gloss build` (check only) and `gloss format`  (est. 130 lines / 3 files)

**Files:**
- Create: `src/cli.rs`, `tests/cli.rs`
- Modify: `src/main.rs`, `src/lib.rs`

**Interfaces:**
- Produces:
  - `cli::run(args: &[String], out: &mut dyn Write, err: &mut dyn Write) -> i32`
  - `main` calls `std::process::exit(cli::run(&args[1..], &mut stdout, &mut stderr))`.
  - The usage text is printed by `gloss` alone, `gloss help` and unknown commands:

```
usage: gloss <command> <file.gloss>
commands: build, test, format, trace, explain, flowchart
```

- [ ] **Step 1: Failing tests** in `tests/cli.rs`. Each test writes into a unique folder under `std::env::temp_dir()`:

```rust
use std::process::Command;
fn gloss(args: &[&str]) -> (i32, String, String) {
    let o = Command::new(env!("CARGO_BIN_EXE_gloss")).args(args).output().unwrap();
    (o.status.code().unwrap(), String::from_utf8(o.stdout).unwrap(), String::from_utf8(o.stderr).unwrap())
}
fn temp_file(name: &str, text: &str) -> std::path::PathBuf {
    let dir = std::env::temp_dir().join(format!("gloss-cli-{}-{}", name, std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let p = dir.join(format!("{name}.gloss"));
    std::fs::write(&p, text).unwrap();
    p
}

#[test]
fn build_reports_errors_with_status_3() {
    let p = temp_file("bad", "feature start\n  set total as 0\n");
    let (code, out, err) = gloss(&["build", p.to_str().unwrap()]);
    assert_eq!((code, out.as_str()), (3, ""));
    assert_eq!(err, "line 2: set total as 0. set needs to. Did you mean set total to 0?\n");
}

#[test]
fn build_checks_clean_program() {
    let p = temp_file("good", "feature start\n  show \"hello\"\n");
    let (code, out, err) = gloss(&["build", p.to_str().unwrap()]);
    assert_eq!(code, 0, "{err}");
    assert_eq!(out, "Checked good.gloss: no errors (code generation arrives in milestone 2)\n");
}

#[test]
fn format_rewrites_in_place() {
    let p = temp_file("fmt", "feature start\n  set  t to 1\n  set t to t + 1\n  show t\n");
    assert_eq!(gloss(&["format", p.to_str().unwrap()]).0, 0);
    assert_eq!(std::fs::read_to_string(&p).unwrap(), "feature start\n  set t to 1\n  add 1 to t\n  show t\n");
    let (code, out, _) = gloss(&["format", p.to_str().unwrap()]);
    assert_eq!((code, out.as_str()), (0, "fmt.gloss is already formatted\n"));
}

#[test]
fn usage_and_unbuilt_commands_exit_3() {
    assert_eq!(gloss(&[]).0, 3);
    assert_eq!(gloss(&["frobnicate", "x.gloss"]).0, 3);
    let (code, _, err) = gloss(&["test", "x.gloss"]);
    assert_eq!((code, err.as_str()), (3, "gloss test is not built yet; it arrives in milestone 4\n"));
    let (code, _, err) = gloss(&["build", "/nonexistent/x.gloss"]);
    assert_eq!((code, err.as_str()), (3, "cannot read /nonexistent/x.gloss\n"));
}
```

- [ ] **Step 2: Run**; it must fail.
- [ ] **Step 3: Implement.**
  - **Warnings** go to standard error, and `build` still exits 0.
  - **`format`** prints `Formatted {file}` when it changes the file. It uses the file name only, not the full path, in both messages.
  - **Unbuilt commands:** `trace` → milestone 5, `explain` → milestone 6, `flowchart` → milestone 7.
- [ ] **Step 4: Run** the verify gate. **Step 5: Commit** with the message `Add CLI for build (check only) and format`.

**How this could pass vacuously:** a status code could be right with the wrong stream. Each test pins both the status and the exact output stream.

**Review model:** sonnet.

---

## Task 22: Spec-example guard  (est. 110 lines / 1 file)

**Files:**
- Create: `tests/spec_examples.rs`

**Interfaces:**
- Consumes: `parse_source`, `check_source`, `format_source`, `parser::parse_stmts_str`.

- [ ] **Step 1: Write the guard.**
  - Extract every block fenced with ` ```gloss ` from `spec/v0.md`. Read it with `include_str!("../spec/v0.md")`.
  - Classify each block:
    - **`future`:** it contains `use "` (the v0.1 sketch). It is skipped, but counted.
    - **`program`:** it contains `feature start`. It must check with zero errors (warnings allowed, printed), and `format_source(block) == block`.
    - **`top`:** its first line starts with `record `, `table `, `feature `, `test ` or `note`, and it has no `feature start`. It must parse with `parse_source`, and `format_source(block) == block`.
    - **`snippet`:** anything else. It must parse with `parse_stmts_str`.
  - Assert the exact counts per class, filled in by running it once and reading the actual numbers. For example: `assert_eq!(counts, [("program", P), ("top", T), ("snippet", S), ("future", 1)])`. Adding or removing a spec example then forces this test to be updated deliberately.
  - Assert the total is at least 10. That is the sweep guard.
- [ ] **Step 2: Run** `cargo test --test spec_examples`.
  - Any failure here is a real disagreement between the spec and the compiler.
  - Fix the compiler if the spec is right. If the spec is wrong, stop and report it to the controller, who decides and records a ruling; do not edit the spec yourself.
- [ ] **Step 3: Run** the verify gate. **Step 4: Commit** with the message `Guard: every spec example compiles`.

**How this could pass vacuously:** extraction could find zero blocks if the fence string changes. The exact counts and the minimum of 10 catch that.

**Review model:** sonnet.

**Risks:** some spec snippets may legitimately depend on context, such as a table declared elsewhere. Those are classified `top` (parse only) or `snippet` by construction; only `program` blocks are fully checked.

---

## Self-review (done while writing)

- **Spec coverage.** Language spec sections and the tasks that cover them:
  - §3 (structure, names, reserved words, text): Tasks 3, 4, 11, 12, 17
  - §4 (types, copies, numbers, truth, conversions): Tasks 2, 13, 14, 18
  - §5 (variables and checks): Tasks 15, 17
  - §6 (expressions and precedence): Tasks 7, 8, 13, 14
  - §7 (statements and their rules): Tasks 9, 10, 15, 16, 18
  - §8 (compile-time parts of records and tables, paths, save check): Tasks 11, 12, 18
  - §9 (features and test placement): Tasks 15, 16
  - §10 (error format, Format): Tasks 1, 19, 20
  - §11 (exit statuses for build and format): Task 21
  - Run-time behaviour (§8 files, §9 fixtures, §10 trace, explain, flowchart, §12) belongs to M2 to M7.
- **Placeholders.** Two are deliberate: the `EXPENSES` constant, which is a verbatim copy of §13, and the class counts in Task 22, which are measured. Both are filled by copying or measuring, not by design.
- **Type consistency.** These names were checked across tasks: `Ty`, `Env`, `type_of`, `check_body`, `FeatureInfo`, `set_rewrite`, `truth_rewrite`, `format_source`, `check_source`, `parse_source`, `parse_stmts_str` and `parse_expr_str`.

## Amendments

None yet.
