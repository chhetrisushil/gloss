# Gloss compiler M8a: nested values, the language - implementation plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task by task. Steps use checkbox (`- [ ]`) syntax for tracking. Ledger: `.superpowers/sdd/2026-11-28-compiler-m8a-nested-values/progress.md`.

**Goal:** Gloss builds every program of `spec/v0.md` §17: lists of lists, records with list and record fields, the value `nothing` with compile-time checks over names and paths, changes at any depth, and nested values in `show`, `is`, `expect` and trace. Copies stay eager and memory is never freed; M8b changes that with no visible change.

**Architecture:**
- The AST's types become recursive with an `or nothing` flag (D67). The checker gains `Ty::Maybe`, one `fits` rule, result inference with `nothing`, and `check/narrow.rs` for checks over paths (D68).
- Lowering gets one routine that turns any path into the address of its slot; every change goes through it (D71). `nothing` is 0; a plain value that may be nothing is boxed (D69).
- The runtime's `copy_value`, `equal_values` and `show_value` walk an explicit work list, never recursing (D70). Descriptors gain `Maybe`, and records that hold themselves get their descriptor index before their fields are resolved.
- Until the back end is ready, lowering refuses nested programs (the **gate**, Task 2). Task 18 lifts it for `build` and `test`, and Task 20 for `trace`.

**Tech stack:** Rust 2024, stable 1.87 or later, no crates.

**Spec:** `docs/specs/2026-11-22-compiler-design.md` is binding: D66 to D72, D76 and §4.12. The language is `spec/v0.md` §17. Conflicts about the language resolve against the language spec; conflicts about the implementation resolve against the design spec. Survey of hook points at `main` @ d4be04fb: `~/.gloss/m8/findings.md` (cited below as `F§n`).

**How to read the tasks.**
- **Binding:** every exact string (messages, JSON keys, sentences), the tests named in each task with their assertions, and the types and signatures in each **Interfaces** block.
- **Guidance:** hook points (`file:line` at d4be04fb; find the named function when lines drift) and sketches. The implementer writes the rest.
- A test may change only if it contradicts the language spec or the design spec. Record that as a ruling in the ledger.

## Global constraints

- [D1] Rust 2024, `rust-version = "1.87"`. [D3] No dependencies. [D4] One crate.
- [D18, D19] Lowering, the IR and the runtime never name an x86 instruction, register or system-call number.
- [D70] Runtime routines never call themselves, directly or through another routine. Depth is handled by a work list on the heap.
- [D69] Every value stays 8 bytes. `nothing` is 0. `number or nothing` and `truth or nothing` hold the address of a one-value block.
- [D66] No reference counts, no freeing and no sharing in M8a: a list or record that already has an owner is copied when it is stored, at every depth. Text and boxes never change, so they may be shared.
- v0 behaviour is unchanged: every existing case passes byte for byte. Tasks 1 to 11 leave every v0 executable byte-for-byte the same (the controller compares sha256 of the `examples/*` builds after Tasks 2 and 11). From Task 12 on, v0 executables may change, and only their instructions may differ.
- Problem strings never end with a period. Diagnostics read `line N: <source line>. <problem>.` (D7).
- Spec §17's quoted messages are binding, word for word.

## Rules for every PR

- **One PR at a time.** When the owner asks for a chain, each task's branch is cut from the previous task's branch, recorded on the `dispatched` line.
- **Size.** At most 200 changed lines (tests and expected files included) and 10 files. Each task gives an estimate; plan estimates run about 40% low, so a task whose estimate × 1.4 passes 200 is split into `m8/tNNa-…`, `m8/tNNb-…` with a ruling. Never trim comments or tests to fit; no `#[rustfmt::skip]`, no golfed code.
- **Every intermediate branch passes the verify gate on its own**, clippy included (M7-R32).
- **Branches:** `m8/tNN-<slug>`. **Pull requests:** draft, merged with `gh pr merge <n> --rebase` on the owner's word.
- **Commits:** one per branch, ending with
  ```
  Co-Authored-By: Claude Sonnet 5.5 <noreply@anthropic.com>
  Claude-Session: https://claude.ai/code/session_011uGukQR6EwQQdiSVNu4ERC
  ```
  PR bodies end with `🤖 Generated with [Claude Code](https://claude.com/claude-code)`, a blank line, and the session link.
- **Verify gate:** `CARGO_BUILD_JOBS=4 cargo fmt --check && cargo clippy --all-targets -- -D warnings && cargo test`. Run cases need the Linux machine (`GLOSS_RUNNER`); without it they are skipped with a warning and `irsim` still runs them.
- **Reviews:** implementers and reviewers use `sonnet` (Task 4: `haiku` implementer). **Risky** tasks (first review on `opus`): 7, 8, 12, 13, 14, 15, 16, 17. Every re-review uses `sonnet`. The final review is `opus`.
- **Branch hygiene:** never delete a branch before `git log --oneline main..<branch>` prints nothing.

## Resource limits

- At most 2 agents at once, reviews included. One `cargo` at a time, `CARGO_BUILD_JOBS=4`, in the foreground.
- No extra cargo target directories; scratch scripts get unique names; never kill a process you did not start.
- A denied command is reported, never worked around.

## Findings that shape this plan (at d4be04fb)

| # | Finding | Consequence |
|---|---|---|
| F1 | `Ty::Nothing` means "the feature gives no value" in the checker, lowering and trace (F§4: stmt.rs:65, features.rs:180, lower/trace.rs:396) | Task 4 renames it `Ty::NoValue` before `nothing` gets a type |
| F2 | There is no single compatibility rule; strict `==` at stmt.rs:214, 419, 446, combine.rs:14, 96, 173, 228 and features.rs:160 | Task 5 adds `fits` and uses it at every site |
| F3 | `record_desc` and `type_desc` recurse eagerly, and `Module::desc` interns by value (ir/mod.rs:382) | A self-holding record would loop forever: Task 11 reserves the index first |
| F4 | Lowering handles only depth-2 targets (`item_target`, `item_field`, `place`); a loop variable holds the item's value, not its slot's address (list.rs:379) | Tasks 15 and 16 |
| F5 | `copy_list` copies only record items; `copy_record`, `equal_fields` and `show_fields` treat every field as a scalar | Tasks 12 to 14 |
| F6 | The trace `check` event re-reads every field read in the condition, eagerly (lower/trace.rs:318-352) | Under `if c is not nothing and value of c is 3`, the re-read would load through 0: Task 19 leaves such reads out |
| F7 | The trace decoder refuses nested record fields (trace/event.rs:69-78) and has no `nothing` | Task 20 |
| F8 | `irsim` runs IR only and shares `ir::rodata` with the executable; it runs the `tests/run`, `tests/test_cases` and `tests/trace_cases` sweeps in-crate (irsim/cases.rs) | Runtime and lowering tasks test end to end through `irsim` before the gate lifts |
| F9 | `tests/spec_examples.rs` cuts §17 (`v0_only`, :17-26) and pins block classes `[1, 4, 4, 1]` and 11 quoted `"line ` messages | Task 18 drops the cut: `[1, 8, 4, 1]`, 15 messages, 4 new `QUOTED_MESSAGES` |
| F10 | `Kw::ALL` has 63 words, and `token.rs:134` checks them against the spec's keyword line (v0.md:44) | Task 1 adds `nothing` to both |
| F11 | The parser already refuses an open form inside a comma list without parentheses (expr.rs:231), and the formatter keeps `Paren` nodes | Format needs no new parenthesis rule; Task 3 tests that nested forms round-trip |
| F12 | Sweep floors: run 153, test 18, trace 47, explain 13, flowchart 6 | Each task that adds cases raises its floor |

## File map

| File | Responsibility | Tasks |
|---|---|---|
| `src/lexer/token.rs`, `spec/v0.md:44` | Keyword `nothing` | 1 |
| `src/ast/{mod,program,stmt,print,sexpr,stmt_sexpr,walk}.rs` | Recursive types, `Nothing`, `append` to a target | 1, 3 |
| `src/parser/{top,expr,stmt,value}.rs` | Types, `nothing`, `append … to target` | 1, 3 |
| `src/format/{mod,print,rewrite}.rs` | Printing the new forms | 1, 3 |
| `src/check/{mod,types,expr,combine,stmt,features}.rs` | `Ty::Maybe`, `fits`, results | 2, 4, 5, 6 |
| `src/check/narrow.rs` (new) | Checks over names and paths | 7, 8 |
| `src/check/{names,mutation,walking,saving,flow}.rs` | Structural rules, changes at depth | 9, 10 |
| `src/lower/gate.rs` (new, removed in 20) | Refusing nested programs until built | 2, 18, 20 |
| `src/ir/{mod,rodata}.rs`, `src/lower/{list,record}.rs` | Descriptors | 11 |
| `src/runtime/{work,copy,equal,values}.rs` | Work list; copy, equal, show at depth | 12, 13, 14 |
| `src/lower/{path,stmt,list,record,expr,cond,copy}.rs` | Paths, changes, loops, boxing | 15, 16, 17 |
| `src/lower/trace.rs`, `src/runtime/json.rs` | Trace events | 19 |
| `src/trace/{event,narrate,value}.rs` | Decoding and narrating | 20 |
| `tests/run/*`, `tests/test_cases/*`, `tests/trace_cases/*`, `tests/explain_cases/*`, `tests/flowchart_cases/*`, `tests/spec_examples.rs` | Cases | 18, 20, 21 |

---

### Task 1: Recursive type syntax (`m8/t01-type-syntax`)

**Files:** `src/lexer/token.rs`, `spec/v0.md` (line 44 only), `src/ast/mod.rs`, `src/ast/program.rs`, `src/parser/top.rs`, `src/parser/expr.rs`, `src/format/mod.rs`, `src/check/expr.rs` (and the other `ast -> Ty` sites, F§4).

**Interfaces:**
- Produces:
  ```rust
  pub enum ItemType { Plain(PlainType), Record(String), List(Box<ItemType>) }
  pub struct Type { pub item: ItemType, pub or_nothing: bool }
  // FieldDecl.ty: Type (was PlainType); InputDecl.ty: Type; ExprKind::EmptyList(ItemType) unchanged
  impl Type { pub fn is_v0(&self) -> bool } // no `or nothing`; Plain or Record, or a List of Plain or Record
  pub fn type_text(&Type) -> String   // "list of list of number", "Cell or nothing"
  pub fn item_type_text(&ItemType) -> String
  pub(crate) fn ty_of(&Type) -> Ty     // in check/expr.rs; the one ast -> Ty conversion
  ```
  `Kw::Nothing` (`"nothing"`) after `Note` in the keyword list; `Kw::ALL` has 64 words. `spec/v0.md` line 44 gains `nothing` in the same alphabetical place.
- Grammar (spec §17): `item_type = plain | RECORD_NAME | "list" "of" item_type`; `type = item_type ["or" "nothing"]`. Record fields, `needs` inputs and `empty list of` all use it (`empty list of` takes an `item_type`, with no `or nothing`). `parse_item_type` keeps the `nested()` depth guard.
- The parser no longer says "A record field is a number, text or truth"; a bad field type gets the item-type error `Expected number, text, truth or a record name` (expr.rs:272).
- In this task the checker still accepts only v0 types: `ty_of` is called on every declared type, and a type that is not `is_v0()` (a nested list, a non-plain field, or `or nothing`) is reported on its line as `nested values arrive in milestone 8a`. Task 2 replaces that refusal.

- [ ] **Step 1: failing tests.**
  - Parser: `nested_types_parse` (`grid as list of list of number` as a `needs` input, `next as Cell or nothing` and `lines as list of Line` as fields, `empty list of list of number`), each giving the expected `Type`; `or_nothing_covers_the_whole_type` (`list of Cell or nothing` is `{ item: List(Record("Cell")), or_nothing: true }`); `a_bad_field_type_is_an_item_type_error` (`a as 5` gives `Expected number, text, truth or a record name`).
  - Lexer: `keyword_list_matches_language_spec` passes with 64 words.
  - Format: `nested_types_round_trip`: formatting a program with each new type form gives the same text.
  - Checker: `nested_types_are_refused_until_built`: `line 2: next as Cell or nothing. nested values arrive in milestone 8a.`
- [ ] **Step 2:** run them; they fail.
- [ ] **Step 3:** implement. Update the existing parser tests that pin `Type::Item`/`Type::List` (top.rs:335, :355) and top.rs:415's message (it now reads as the item-type error).
- [ ] **Step 4:** verify gate. **Step 5:** commit `Parse nested types and or nothing`.

**Fail-proof:** a parser that reads one `list of` level passes `list of number`; `list of list of number` is in the tests. `or nothing` bound to the inner type instead of the whole passes `Cell or nothing`; the `list of Cell or nothing` test pins it.
**Review:** sonnet. **Estimate:** ~150 lines (×1.4 = 210: split 1a lexer, AST and parser; 1b format and checker refusal).

### Task 2: `Ty::Maybe`, full type conversion, and the gate (`m8/t02-maybe-gate`)

**Files:** `src/check/mod.rs`, `src/check/types.rs`, `src/check/expr.rs`, `src/lower/gate.rs` (new), `src/lower/mod.rs`.

**Interfaces:**
- Produces `Ty::Maybe(Box<Ty>)`. `describe`: `"Cell or nothing"`, `"list of list of number"` (a `Maybe` is described without an article by `described`: `"next of c is Cell or nothing"`). `item()` of a `Maybe` is `None`. `norm()` leaves `Maybe` as it is, normalising inside.
- `ty_of` converts every type fully: `List` nests, `or_nothing` wraps in `Maybe`. The Task 1 refusal is removed from the checker.
- `lower::gate::first_nested(program: &Program, types: &TypeInfo) -> Option<Line>`: the first line that declares or uses a nested value: a type that is not `is_v0()`, an `ExprKind::Nothing` (Task 3), or an expression whose recorded type holds a `Maybe` or a list inside a list.
- `lower_program` refuses such a program with `line N: <source>. nested values arrive in milestone 8a.`, before lowering anything, in every mode. `#[cfg(test)] pub(crate) fn lower_ungated(program, types, src, mode) -> Result<Module, Diagnostic>` skips the gate, for in-crate tests until the gate lifts.
- `#[cfg(test)] pub(crate) fn irsim::run_source(text: &str, stdin: &[u8]) -> Outcome`: parse, check, `lower_ungated` (real mode), run under `irsim`. Later tasks test end to end with it.

- [ ] **Step 1: failing tests.** `types.rs`: `maybe_describes_and_has_no_item`. `gate.rs`: `the_gate_names_the_first_nested_line` (a program with `next as Cell or nothing` on line 3 gives `Some(3)`), `v0_programs_pass_the_gate` (every file in `examples/` gives `None`). `check`: `nested_types_check` (a record with `lines as list of Line` and a feature taking `grid as list of list of number` check with no errors).
- [ ] **Step 2-4:** fail, implement, verify gate (the example executables must not change: the controller compares sha256).
- [ ] **Step 5:** commit `Add the maybe type and refuse nested programs until they are built`.

**Fail-proof:** a gate that only reads declarations misses `list of (list of 1, 2)` built in a body: `the_gate_names_the_first_nested_line` gets a second program with only that line.
**Review:** sonnet. **Estimate:** ~130 lines.

### Task 3: `nothing`, `nothing as T`, `is nothing`, and `append … to target` (`m8/t03-nothing-syntax`)

**Files:** `src/ast/{mod,stmt,print,sexpr,stmt_sexpr,walk}.rs`, `src/parser/{expr,stmt}.rs`, `src/format/{print,rewrite}.rs`, `src/check/{stmt,flow,saving,mutation,walking}.rs` (append's new shape only).

**Interfaces:**
- `ExprKind::Nothing(Option<ItemType>)`: `nothing` or `nothing as T`, parsed in `primary` (expr.rs:207). `nothing as number` is a type, not a conversion (it is read by `primary` before `converted`).
- `is nothing` and `is not nothing` are ordinary `Compare { op: Is | IsNot, right: Nothing(None) }`; no new expression kind.
- `StmtKind::Append { value: Expr, target: Target }` (was `list: String`), parsed with `parse_target` (stmt.rs:141). Every consumer that read `list` reads `target.root()` and behaves as before for a bare name; the walkers visit the target's positions (`walk.rs:19`, `:101`).
- Printers: `nothing`, `nothing as Cell`, `append 0 to item 2 of grid`. `rewrite.rs:43` treats `Nothing` as bare.
- `lower::gate::first_nested` counts an `ExprKind::Nothing`.
- Checker in this task: `Nothing` is typed `Ty::Error` with no diagnostic (Task 5 types it); an append whose target is not a bare name gets `nested values arrive in milestone 8a` from the checker (Task 10 lifts it). Everything else is unchanged.

- [ ] **Step 1: failing tests.** Parser: `nothing_and_nothing_as_parse`; `is_not_nothing_is_a_comparison`; `append_takes_a_path` (`append 7 to item 2 of grid`). Format: `nothing_forms_round_trip` (including `set head to Cell with value 3, next head` and `list of (list of 1, 2), (list of 3, 4)`, which keep their parentheses). S-expression printers: one test each for `nothing as Cell` and the new append.
- [ ] **Step 2-4.** Every existing `append` test passes unchanged.
- [ ] **Step 5:** commit `Parse nothing and append to a path`.

**Fail-proof:** an append that keeps `list: String` and stores only the root passes the existing tests; `append_takes_a_path` checks the whole target.
**Review:** sonnet. **Estimate:** ~150 lines (×1.4 = 210: split 3a `nothing`, 3b `append`).

### Task 4: Rename `Ty::Nothing` to `Ty::NoValue` (`m8/t04-no-value`)

**Files:** every use (F1): `src/check/{mod,types,stmt,features,combine}.rs`, `src/lower/trace.rs`, `src/explain/*` if it matches on it.

**Interfaces:** `Ty::NoValue`: a feature that gives no value. `describe` still says `"nothing"` for it, so no message changes. Pure rename; no behaviour change.

- [ ] **Step 1-4:** rename, verify gate (no test changes except the enum name).
- [ ] **Step 5:** commit `Name the type of a feature that gives no value NoValue`.

**Fail-proof:** a missed site fails to compile. **Review:** sonnet (implementer: haiku). **Estimate:** ~30 lines.

