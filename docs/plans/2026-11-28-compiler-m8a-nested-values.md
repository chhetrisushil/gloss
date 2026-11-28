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

### Task 5: `fits`, the type of `nothing`, and boxing marks (`m8/t05-fits`)

**Files:** `src/check/{mod,combine,stmt,expr,features}.rs`, `src/check/stmt/prepass.rs`.

**Interfaces:**
- `Ty::NothingLit`: the type of a bare `nothing`. `describe`: `"nothing"`. `nothing as T` has type `Maybe(ty_of(T))`.
- `pub(crate) fn fits(got: &Ty, want: &Ty) -> bool`: equal after `norm`; or `want` is `Maybe(T)` and `got` is `T` or `NothingLit`; a `Table` fits `List(Record)` as today. Used at every site of F2: `set` of an existing variable, field or item; a call's inputs; a record build's fields; `append`; `give` agreement (Task 6 changes inference); `is` and `is not` (either side may fit the other).
- `TypeInfo.boxed: BTreeSet<ExprId>`: each expression of type `Number` or `Truth` whose value is stored, passed, given or compared where a `Maybe` is wanted. Lowering (Task 17) boxes exactly these.
- A new variable whose first `set` is a bare `nothing`: `nothing needs a type here, as in set {name} to nothing as {R}`, where `{R}` is the first record declared in the program, or `number` when there is none. A bare `nothing` anywhere else no type is known (`show nothing`, `nothing + 1`, a list item): the same message with `set x to nothing as {R}`.
- `x is nothing` where `x` is not a `Maybe`: `{x} is {described}, so it is never nothing`.
- Arithmetic, conditions, `for each`, field and item reads on a `Maybe` keep their existing messages in this task (Task 7 replaces them with the check message).

- [ ] **Step 1: failing tests** (exact strings): `a_value_fits_its_maybe` (`set best to nothing as Student` then `set best to s` checks; the reverse, a `Maybe` into a `Student` variable, gives `best is Student, so it cannot be set to Student or nothing`); `nothing_needs_a_type` (spec: `line 4: set best to nothing. nothing needs a type here, as in set best to nothing as Student.`); `inputs_take_nothing` (`run show_in_order with nothing` checks when the input is `Node or nothing`); `boxed_marks_plain_values_into_maybes` (`set m to 5` into a `number or nothing` field marks the `5`); `only_maybes_are_nothing`.
- [ ] **Step 2-5.** Commit `Let a value fit where it may be nothing`.

**Fail-proof:** `fits` that ignores direction passes the forward test; the reverse test fails it. A `boxed` set filled for every number passes the marking test; it also asserts a plain `set n to 5` is not marked.
**Review:** sonnet. **Estimate:** ~180 lines (×1.4 = 252: split 5a `fits` and its sites, 5b `nothing` typing, messages and `boxed`).

### Task 6: Results that may be nothing, and nested lists (`m8/t06-results-lists`)

**Files:** `src/check/{stmt,features,combine,expr}.rs`.

**Interfaces:**
- Result inference (stmt.rs:62-67, features.rs:30-79): the `give` types of a feature combine as: all `T` → `T`; `T` and `NothingLit` (in any order) → `Maybe(T)`; `T` and `Maybe(T)` → `Maybe(T)`; only `NothingLit` → error `cannot work out what {f} gives; add a give with a value`, on the first `give nothing` line. The `infer_results` loop must stay monotone (a type only grows from `T` to `Maybe(T)`), so its `debug_assert!(converged)` holds.
- Lists: any type may be an item except `NothingLit` and `Maybe`: `A list holds numbers, text, truths, lists or records; {x} is {described}`. `Lists of lists are not part of v0` (combine.rs:221) is removed. `empty list of list of number` and nested `list of` builds type as `List(List(Number))`.
- Record builds accept fields of any declared type through `fits`. `record_type` (combine.rs:172), `field_type` (expr.rs:291) and `target_ty` (stmt.rs:118) all read field types through `ty_of`.

- [ ] **Step 1: failing tests:** `find_gives_student_or_nothing` (the spec's `find` program checks, and the call's type is `Student or nothing`); `give_nothing_alone` (spec: `line 6: give nothing. cannot work out what find gives; add a give with a value.`); `recursive_results_converge` (the spec's `insert`, which calls itself, checks); `lists_of_lists_check` (the spec's grid program checks); `lists_never_hold_nothing` (`list of 1, nothing` gives the list message with `nothing is nothing`).
- [ ] **Step 2-5.** Commit `Infer results that may be nothing and allow lists of lists`.

**Fail-proof:** inference that takes the first give's type passes `find` written value-first; the test also has a version with `give nothing` first.
**Review:** sonnet. **Estimate:** ~150 lines (×1.4 = 210: split 6a results, 6b lists and fields).

### Task 7: Checks over names and paths: where a check holds (`m8/t07-narrow`) - risky

**Files:** `src/check/narrow.rs` (new), `src/check/{mod,stmt,expr,combine}.rs`.

**Interfaces:**
- ```rust
  pub(crate) enum Step { Field(String), Item(String) } // an item step keeps its position as `print::expr` writes it
  pub(crate) struct PathKey { pub root: String, pub steps: Vec<Step> }
  pub(crate) fn key_of(e: &Expr, types: &…) -> Option<PathKey> // Name, FieldOf, ItemOf, Paren chains rooted at a variable, input or loop variable; None for anything else (a call, a table, a built value)
  pub(crate) struct Facts { pub when_true: Vec<PathKey>, pub when_false: Vec<PathKey> }
  pub(crate) fn facts(cond: &Expr) -> Facts
  // TypeInfo.narrowed: BTreeSet<ExprId>: reads of a Maybe(T) path inside a check, recorded as type T
  ```
- `facts`: `x is not nothing` → true `[x]`; `x is nothing` → false `[x]`; `not (c)` swaps; `a and b and …` → true: the union of the parts' true facts, false: none; anything else → none.
- The checker's walk carries a set of checked keys (`Walk`, stmt.rs:14). Where a check holds (spec §17):
  - the body of `if`/`otherwise if` branch i: its condition's true facts, plus the false facts of every earlier condition in the chain;
  - the `otherwise` body: the false facts of every condition;
  - the body of `repeat while`: its condition's true facts;
  - inside an `and`, each part is typed with the true facts of the parts before it (expr.rs:128-137).
- Typing a read whose key is checked and whose type is `Maybe(T)` records `T` and adds its id to `narrowed`.
- Using a `Maybe` where a value is needed, outside a check, gives, on the innermost culprit: `{e} may be nothing here. Check it with if {e} is not nothing first`. "Where a value is needed": `field of e`, `item p of e`, `count of`/`first of`/`last of`, `for each … in e`, arithmetic and `whole`/`remainder`, conversions, a condition (`if e`, `and`, `or`, `not`), an input whose type is not a `Maybe`, and a `set`, field or item whose type is not a `Maybe`. `show`, `is`, `is not`, storing into a `Maybe` and `give` from a feature whose result is a `Maybe` stay allowed.
- In this task a check lasts to the end of the block it covers; Task 8 ends it earlier.

- [ ] **Step 1: failing tests** (exact strings):
  - `reading_through_nothing_needs_a_check` (spec: `line 9: show value of next of c. next of c may be nothing here. Check it with if next of c is not nothing first.`)
  - `the_linked_list_checks` (spec §17's first program, with no errors)
  - `a_check_covers_a_path` (spec: `if next of c is not nothing` then `set value of next of c to 9` and `add 1 to value of next of c`)
  - `otherwise_after_is_nothing` (`if hit is nothing` / `show 0` / `otherwise` / `show marks of hit`)
  - `the_rest_of_an_and` (`if n is not nothing and value of n is 3`); `not_before_and_is_not_a_check` (`if value of n is 3 and n is not nothing` gives the error on the first part)
  - `narrowed_reads_are_recorded` (the `c` in `value of c` inside the loop is in `narrowed` and typed `Cell`)
- [ ] **Step 2-5.** Commit `Check that a value may be nothing before it is read`.

**Fail-proof:**
- Facts that ignore paths and key on the root only would pass `the_linked_list_checks`. `a_check_covers_a_path` also has `show value of left of c` under a check of `right of c`, and that must be an error.
- An `and` that types every part with every part's facts passes `the_rest_of_an_and`; `not_before_and_is_not_a_check` fails it.

**Review:** opus first, then sonnet. **Estimate:** ~200 lines. Split it in two: 7a covers the keys, facts and blocks; 7b covers `and`, the consumer errors and `narrowed`.

### Task 8: Checks over names and paths: how long a check lasts (`m8/t08-narrow-ends`) - risky

**Files:** `src/check/narrow.rs`, `src/check/stmt.rs`, `src/check/uses.rs` (read only).

**Interfaces:**
- `fn drop_overlapping(checked: &mut BTreeSet<PathKey>, changed: &Target)`. A key is dropped when:
  - the changed path is a prefix of it, or it is a prefix of the changed path (`set c to …` and `set next of c to …` both end a check of `next of c`);
  - the changed root is a name used in one of its item positions.

  Every change form calls it: `set`, the four arithmetic verbs, `append`, `clear`, `ask … into`, `load`, and the `for each` loop variable on each turn. Calls drop nothing, because a feature cannot change its inputs or the caller's variables.
- **Loops.** On entering a loop body (`for each` or `repeat while`), every key that overlaps any change made anywhere in the body is dropped first, because a later turn starts after the body's changes. A `repeat while` condition then adds its own true facts again, so they hold at the start of every turn.
- **After a block that always leaves.** An `if` chain whose only branch's body always leaves (`uses::diverges(body, Exits::Any)`) and has no `otherwise` adds its condition's false facts for the rest of the enclosing block.

- [ ] **Step 1: failing tests:**
  - `setting_ends_a_check` (`if c is not nothing` / `set c to next of c` / `show value of c`: the error on the `show` line)
  - `a_deep_set_ends_a_deeper_check`
  - `a_position_change_ends_a_check` (`if next of item i of cs is not nothing` / `set i to 2` / `show value of next of item i of cs`: error)
  - `a_loop_body_change_ends_an_outer_check` (`if c is not nothing` / `repeat while k is less than 3` / `show value of c` / `set c to next of c`: error on the `show`)
  - `after_a_give_the_check_holds` (`if hit is nothing` / `give 0`, then `give marks of hit`)
  - the spec's `insert` and `show_in_order` check with no errors
- [ ] **Step 2-5.** Commit `End a check where the checked value changes`.

**Fail-proof:**
- Dropping keys only on an exact match passes `setting_ends_a_check`, but `a_deep_set_ends_a_deeper_check` catches it.
- Skipping the loop-entry drop passes every straight-line test, but `a_loop_body_change_ends_an_outer_check` catches it.

**Review:** opus first, then sonnet. **Estimate:** ~150 lines (×1.4 = 210). Split it in two: 8a covers changes and the always-leaves rule; 8b covers loops.

### Task 9: Records that hold themselves, and flat tables (`m8/t09-record-rules`)

**Files:** `src/check/names.rs`.

**Interfaces:**
- **Unknown records.** `unknown_records` (names.rs:231) checks record names wherever they appear: in every field type, input type and `empty list of` / `nothing as` type, at any depth.
- **A record that holds itself.** Build a graph with an edge R → S for each field of R whose type is exactly `S`, not `or nothing` and not inside a list. Report each cycle once, on the field that closes it: the first record of the cycle in source order, at its field that leads into the cycle. The message is `{field} as {S} would make every {R} hold another {R} forever. Write {field} as {S} or nothing, or children as list of {S}`. The spec's example is `line 3: next as Cell would make every Cell hold another Cell forever. Write next as Cell or nothing, or children as list of Cell.`
- **Flat tables.** A table's record may only have fields of plain type with no `or nothing`. The error is reported on the table's line, naming the first bad field: `{table} cannot be a table: {field} of {R} is {what}, and a table file holds only numbers, text and truth. Keep {table} in a list instead`. Here `{what}` is `a list`, `a record`, or `something that may be nothing`. The spec's example is `line 5: orders cannot be a table: lines of Order is a list, and a table file holds only numbers, text and truth. Keep orders in a list instead.`

- [ ] **Step 1: failing tests:**
  - `a_record_cannot_hold_itself` (spec message)
  - `two_records_cannot_hold_each_other` (A has `b as B` and B has `a as A`: one error, on A's field)
  - `or_nothing_and_lists_break_the_cycle` (Cell, Node and `children as list of Node` all check)
  - `tables_stay_flat` (spec message; also the record and maybe forms)
  - `unknown_records_inside_types` (`items as list of list of Itme` gives `There is no record named Itme` with the existing suggestion)
- [ ] **Step 2-5.** Commit `Refuse records that hold themselves and nested table records`.

**Fail-proof:** a cycle check that only looks at direct self-reference passes the spec test, but `two_records_cannot_hold_each_other` catches it.
**Review:** sonnet. **Estimate:** ~140 lines.

### Task 10: Changes at any depth: inputs, loops and saving (`m8/t10-change-rules`)

**Files:** `src/check/{mutation,walking,saving,flow}.rs`, `src/check/stmt.rs` (lift Task 3's append refusal).

**Interfaces:**
- `mutation.rs`:
  - `changed()` returns the whole target path.
  - A change whose root is an input gives the existing message `{name} is an input, so it cannot be changed. To work on a copy, set mine to {name} first`. That includes `append x to children of tree`.
  - A loop variable that stands for an item can now be appended to, and its fields and items can be changed (spec: "Inside `for each row in grid`, `append 0 to row` changes that item"). The v0 refusal `… so nothing can be appended to it` (mutation.rs:118) is removed.
  - Setting the loop variable itself keeps its message (:121). Changing a loop variable that walks an input keeps its message (:113).
- `walking.rs`: a loop walks a path W (`lines of order`, `item 1 of grid`). A change conflicts with it in any of these cases:
  - its target is a prefix of W or equal to W (set, append, clear, load);
  - it sets `item p of W` itself.

  Changes further inside an item, or through the loop variable, stay allowed, as in v0. The error is the existing `{list} cannot change while the loop on line {line} walks it{cause}`, with W printed. The call-chain part concerns tables only, as today (features cannot change their inputs).
- `saving.rs` and `flow.rs` read `target.root()` for `append`. The Task 3 checker refusal of path appends is removed.

- [ ] **Step 1: failing tests:**
  - `deep_input_changes_are_refused` (`append x to children of tree`)
  - `appending_to_a_row_is_allowed` (the spec's grid program checks)
  - `setting_a_row_is_refused` (existing message)
  - `a_walked_path_cannot_change` (`for each line in lines of order` / `set order to o2`: error naming `lines of order`; `append l to lines of order`: error)
  - `changes_inside_an_item_are_allowed` (`for each line in lines of order` / `add 1 to qty of line`: no error)
- [ ] **Step 2-5.** Commit `Apply the input and loop rules to paths of any depth`.

**Fail-proof:** comparing roots only would refuse `set note of order to "x"` while `lines of order` is walked; `a_walked_path_cannot_change` asserts that this change, which is not a prefix of the walked path, is allowed.
**Review:** sonnet. **Estimate:** ~170 lines (×1.4 = 238). Split it in two: 10a covers mutation; 10b covers walking and saving.

### Task 11: Descriptors for nested values (`m8/t11-descriptors`)

**Files:** `src/ir/mod.rs`, `src/ir/rodata.rs`, `src/lower/list.rs`, `src/lower/record.rs`.

**Interfaces:**
- `Desc::Maybe(u32)` (the inner descriptor), kind word `DESC_MAYBE = 5`. Its rodata is three words: `[5, inner descriptor address, name text address]`. The name text is the inner type as Gloss writes it (`Cell`, `number`, `list of number`), so trace can write `{"nothing":"Cell"}`.
- `Module::reserve_desc(&mut self) -> u32` and `Module::set_desc(&mut self, i: u32, d: Desc)`. `record_desc(name)` looks the name up in a per-module map. If the name is missing, it reserves an index, records it, resolves the fields (which may name this same index), then sets the descriptor. All other descriptors are interned by value, as today.
- `type_desc` handles `Maybe` and lists at any depth. A record's field descriptors come from `ty_of(field.ty)`.

- [ ] **Step 1: failing tests:**
  - `a_record_that_holds_itself_has_one_descriptor` (Cell: `Record { fields: [("value", number), ("next", m)] }` with `descs[m] == Maybe(cell)`)
  - `maybe_descriptors_are_laid_out` (rodata words and name text)
  - `v0_descriptors_are_unchanged` (the IR text of an existing lowering test is the same)
- [ ] **Step 2-5.** Commit `Describe nested types and records that hold themselves`.

**Fail-proof:** without the reservation, the Cell test overflows the stack instead of failing cleanly. The test therefore runs the build in a thread with a small stack and asserts that it finishes.
**Review:** sonnet. **Estimate:** ~120 lines.

