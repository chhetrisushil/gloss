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

### Task 12: The work list, and copies at any depth (`m8/t12-copy-deep`) - risky

**Files:** `src/runtime/work.rs` (new), `src/runtime/copy.rs`, `src/runtime/mod.rs`, `src/runtime/heap_tests.rs` (or a new `copy_tests.rs`).

**Interfaces:**
- The work list is an ordinary list block (`list_new`, `list_push`), used as a stack of words. The new routine is `work_pop(list) -> word`: it lowers the count by one and gives the last word. The caller checks the count before popping. These routines are not reachable from v0 programs unless the program copies something.
- `copy_value(value, desc, line)` keeps its signature. It copies at any depth:
  1. Push `(source, desc, address of the destination word)` for the root.
  2. Loop until the list is empty:
     - Pop a triple.
     - A `Maybe` holding 0 stores 0. Any other `Maybe` continues with its inner descriptor.
     - A record gets a new block, and pushes one triple for each field.
     - A list gets a new header with room for at least 4 items, plus an items block, and pushes one triple for each item.
     - Number, text and truth are stored as they are. Text and boxes are never changed, so sharing them is safe.
  3. The result is the root's destination word.
- `copy_record` and `copy_list` either go, or become helpers that never call back into `copy_value`.
- **Guard for every later task:** a unit test `no_runtime_routine_recurses` builds the call graph of `runtime::routines()` (each `Inst::Call` target) and asserts it has no cycle.

- [ ] **Step 1: failing tests:**
  - `no_runtime_routine_recurses`
  - `a_copied_list_of_lists_is_its_own` (copy `[[1,2],[3]]`, poke the copy's inner list, and the original reads the same)
  - `a_record_with_a_list_field_is_copied_deep`
  - `a_chain_of_cells_is_copied` (10,000 cells deep: it finishes and the copy's last cell is a new block)
  - `nothing_copies_as_nothing`
- [ ] **Step 2-5.** Commit `Copy values at any depth with a work list`.

**Fail-proof:**
- A copy that shares inner lists passes a flat test. `a_copied_list_of_lists_is_its_own` pokes the inner list to catch that.
- A recursive copy passes every value test. `no_runtime_routine_recurses` catches it, and the 10,000-deep chain would also fault the stack.

**Review:** opus first, then sonnet. **Estimate:** ~190 lines (×1.4 = 266). Split it in two: 12a covers the work list, the guard and records; 12b covers lists and `Maybe`.

### Task 13: `is` at any depth (`m8/t13-equal-deep`) - risky

**Files:** `src/runtime/equal.rs`, its tests.

**Interfaces:**
- `equal_values(a, b, desc)` keeps its signature. It compares at any depth with a work list of `(a, b, desc)` triples:
  - A `Maybe`: both 0 are equal, only one 0 is unequal, otherwise compare the inner values.
  - Records compare field by field; lists compare counts, then items.
  - Text compares with `text_equal`, numbers and truths by word.
  - The first difference ends the walk with 0. An empty list ends it with 1.

- [ ] **Step 1: failing tests:**
  - `nested_lists_compare_deep` (equal, and unequal in the last inner item)
  - `records_with_list_fields_compare`
  - `nothing_equals_only_nothing`
  - `boxed_numbers_compare_by_value` (two boxes holding 5 are equal)
- [ ] **Step 2-5.** Commit `Compare values at any depth`.

**Fail-proof:**
- A compare that checks only counts passes the equal case. The unequal-last-item case catches it.
- A compare that compares box addresses passes when both sides share one box. The test builds two separate boxes.

**Review:** opus first, then sonnet. **Estimate:** ~150 lines.

### Task 14: `show` at any depth (`m8/t14-show-deep`) - risky

**Files:** `src/runtime/values.rs`, `src/runtime/mod.rs` (the text `nothing` in `TEXTS`), their tests.

**Interfaces:**
- `show_value(value, desc)` keeps its signature. The work list holds two kinds of entries, told apart by a tag word: *show `(value, desc, nested)`* and *write fixed text n*. Entries are pushed in reverse order so they come out in order.
- Forms:
  - Text at the top level is unquoted; nested text is quoted.
  - A nested record, or a nested non-empty list, is in parentheses. An empty list is never in parentheses.
  - `nothing` is written `nothing`, and a boxed number or truth shows its value.

  So `Cell with value 1, next (Cell with value 2, next nothing)` and `list of (list of 1, 2), (list of 3, 4)`.
- v0 forms are unchanged: a record in a list is in parentheses, as today.

- [ ] **Step 1: failing tests** (exact output):
  - `cells_show_their_source_form` (the spec string)
  - `a_grid_shows_in_parentheses` (`list of (list of 1, 9, 3, 0), (list of 4, 5, 6, 7, 0)`)
  - `an_empty_inner_list` (`list of (list of 1), empty list of number`)
  - `text_is_quoted_inside`
  - `a_lone_nothing`
- [ ] **Step 2-5.** Commit `Show values at any depth in their source form`.

**Fail-proof:**
- Pushing entries in order instead of in reverse prints items backwards. Every test has at least two items to catch that.
- Putting parentheses around empty lists passes the grid test. `an_empty_inner_list` catches it.

**Review:** opus first, then sonnet. **Estimate:** ~180 lines (×1.4 = 252). Split it in two: 14a covers the tagged work list and records; 14b covers lists, `nothing` and boxes.

### Task 15: Paths: the address of any slot (`m8/t15-paths`) - risky

**Files:** `src/lower/path.rs` (new), `src/lower/{stmt,list,record}.rs`.

**Interfaces:**
- ```rust
  enum Place { Slot(u32), Table(String), Memory } // Memory: the slot's address is in `a`
  fn place(&mut self, target: &Target) -> Result<Place, Diagnostic>
  ```
  - A `Var` is its frame slot, or a table.
  - A longer path loads the root, then follows each step but the last. A field step loads at its offset. An item step goes through `item_address` (failures 2 and 3, with the list named as `print::expr` writes it) and then loads.
  - The last step leaves the slot's address in `a`. A checked step needs no test here: the checker has proved it is not nothing.
- `set`, `add`, `subtract`, `multiply` and `divide` on any target go through `place`. `set` stores `store_value(value)`, so a list or record that already has an owner is copied, at every depth.
- The new value is computed before the address (D44 order), so a call inside the value runs first.
- `item_target`, `item_field` and the old `place` in record.rs (F4) are replaced. A v0 program behaves the same, though its instructions may differ.

- [ ] **Step 1: failing tests** (through `irsim::run_source`):
  - `set_deep_in_a_grid` (`set item 2 of item 1 of grid to 9`, then `show grid` prints the spec line without the appends)
  - `add_deep_in_records` (`add 1 to marks of item 2 of students of class`)
  - `a_deep_position_fails_with_its_list` (failure 2 names `item 1 of grid`)
  - `set_copies_at_depth` (`set b to a`, `set item 1 of item 1 of b to 0`, `show a` is unchanged)
  - every v0 run case passes, under `irsim` and as executables
- [ ] **Step 2-5.** Commit `Change values through paths of any depth`.

**Fail-proof:**
- A path that loads the last step instead of taking its address changes a copy. `set_deep_in_a_grid` shows the original.
- Computing the address before the value lets a call in the value see a half-done change. An IR-order test pins the order.

**Review:** opus first, then sonnet. **Estimate:** ~190 lines (×1.4 = 266). Split it in two: 15a covers `place` and `set`; 15b covers the four verbs and removing the old helpers.

### Task 16: Appends at any depth, loop variables, and building with nested values (`m8/t16-append-loops`) - risky

**Files:** `src/lower/{list,record,stmt,expr,copy}.rs`, `tests/explain_cases/*.out` (re-blessed where loops change).

**Interfaces:**
- `append value to target`: `place(target)`, load the list header, then `list_push(header, store_value(value))`. A bare name and a table behave as today.
- A loop variable that walks a list holds the **address of the current item's slot** (D71). Reading it loads through that address. Changes through it (`append 0 to row`, `set qty of line to 2`) use the address as their place. The walked list must not move during the loop, and the checker's walking rule (Task 10) guarantees that.
- A record build stores each field through `store_value`, so a list variable put into a record is copied.
- `give` and call inputs stay as they are: inputs cannot change, and the receiving `set` copies.
- Explain goldens with a `for each` change only in how the loop variable is loaded. They are re-blessed with `GLOSS_BLESS=1`, and the reviewer reads the diff.

- [ ] **Step 1: failing tests** (`irsim::run_source`):
  - `the_grid_program` (spec §17's grid: `list of (list of 1, 9, 3, 0), (list of 4, 5, 6, 7, 0)`)
  - `append_deep` (`append line to lines of order`)
  - `a_built_record_owns_its_list` (`set r to Order with lines ls`, `append x to ls`, `show r` unchanged)
  - `loop_variable_changes_reach_the_item` (`for each line in lines` / `add 1 to qty of line`, then `show lines`)
- [ ] **Step 2-5.** Commit `Append at any depth and let loop variables change their items`.

**Fail-proof:**
- A loop variable that copies the item passes every read test. `loop_variable_changes_reach_the_item` catches it.
- A record build that does not copy passes the grid test. `a_built_record_owns_its_list` catches it.

**Review:** opus first, then sonnet. **Estimate:** ~170 lines plus re-blessed goldens (×1.4 = 238). Split it in two: 16a covers append and builds; 16b covers loop variables and the goldens.

### Task 17: `nothing`, boxes and checked reads (`m8/t17-nothing-lowering`) - risky

**Files:** `src/lower/{expr,cond,stmt,copy,list}.rs`, `src/runtime/core.rs` (or `values.rs`) for `box_value`.

**Interfaces:**
- `nothing` and `nothing as T` lower to `Const 0`.
- `box_value(v) -> address`: a one-word block holding `v`. Every expression in `TypeInfo.boxed` is boxed right after it is computed.
- Every expression in `TypeInfo.narrowed` whose recorded type is `Number` or `Truth` is unboxed after it is read (one load).
- `x is nothing` and `x is not nothing` compare with 0, with no routine call. `is` between two `Maybe` values, after boxing, calls `equal_values` with the `Maybe` descriptor.
- `show` of a `Maybe` calls `show_value` with its descriptor. Text at the top level stays unquoted.
- A `set` of a `Maybe` variable, field or item is accepted by the statement guards (stmt.rs:47-51) and copies through `copy_value` with the `Maybe` descriptor.

- [ ] **Step 1: failing tests** (`irsim::run_source`, the spec programs with their printed output):
  - `the_linked_list` (1, 2, 3)
  - `find_gives_nothing_or_a_student` (67)
  - `the_search_tree` (1, 2, 5, 8, 9)
  - `number_or_nothing_boxes` (field `score as number or nothing`: shows `R with score 5`, then `R with score nothing`; under a check, `score of r + 1` prints 6)
  - `two_nothings_are_equal`
- [ ] **Step 2-5.** Commit `Build nothing, boxes and checked reads`.

**Fail-proof:**
- Unboxing every number passes the plain tests but reads garbage from a plain `score`. `number_or_nothing_boxes` also shows a plain number field.
- A `Maybe` compared by address passes `two_nothings_are_equal`. The test also compares two separately boxed 5s.

**Review:** opus first, then sonnet. **Estimate:** ~170 lines (×1.4 = 238). Split it in two: 17a covers `nothing`, `is nothing` and `show`; 17b covers boxes.

### Task 18: Build and test nested programs (`m8/t18-lift-build`)

**Files:** `src/lower/gate.rs` (the gate now applies only to trace mode), `tests/spec_examples.rs`, `tests/run/*`, `tests/run_sweep.inc`, `tests/test_cases/*`, `tests/test_sweep.inc`.

**Interfaces:**
- The gate refuses only `lower_trace` programs, with the same message.
- The spec guard drops `v0_only` and `section_17_is_left_out_of_the_v0_checks`. Block classes become `[1, 8, 4, 1]`, the `"line ` count becomes 15, and the four §17 quoted messages join `QUOTED_MESSAGES`. The four §17 programs join `PRINTS`: `1\n2\n3\n`, `67\n`, `1\n2\n5\n8\n9\n`, and the grid line.
- Run cases:
  - `nested_linked_list`, `nested_find`, `nested_tree`, `nested_grid`
  - `nested_copies` (deep copy independence, for lists and records)
  - `nested_maybe_number`
  - `nested_failure_deep` (failure 2 at depth, with `.status` and `.err`)
  - `nested_append_paths`
- Test cases: `nested_expect_pass` (expect on nested lists and records, and `is nothing`) and `nested_expect_fail` (a failing expect on a grid, with its report).
- Floors: `MIN_CASES` goes to 161 and `MIN_TEST_CASES` to 20.

- [ ] **Step 1-4.** Add the cases and the guard change, run the sweeps under `irsim` and on Linux, then run the verify gate.
- [ ] **Step 5.** Commit `Build and test nested programs`.

**Fail-proof:** a sweep that skips a case passes with fewer cases. The raised floors catch that. Each case's `.out` is written by hand from the spec, not blessed from the compiler.
**Review:** sonnet. **Estimate:** ~240 lines across about 22 files. Split it in three: 18a covers the gate, the spec guard and the linked-list and find cases; 18b covers the tree, grid, copies, maybe-number, failure and append cases; 18c covers the test cases.

### Task 19: Trace events for nested values (`m8/t19-trace-events`)

**Files:** `src/runtime/json.rs`, `src/runtime/mod.rs` (JSON pieces), `src/lower/trace.rs`, `src/runtime/trace_backend_tests.rs`.

**Interfaces:**
- `json_value`:
  - A `Maybe` holding 0 writes `{"nothing":"<name>"}`, using the descriptor's name text (Task 11).
  - Any other `Maybe` writes its inner value, unboxed for number and truth.
  - Nested lists and records are written inside one another, using a work list (D70). `json_value` must not call itself; `no_runtime_routine_recurses` holds.
- A `set` whose target is one of the v0 shapes (`x`, `field of x`, `item p of x`, `field of item p of x`) writes the same keys as today. Any other target writes `"name":"<root>","target":"<path as print writes it>"` in place of `item` and `field`, followed by `from` and `to` as usual. Example: `{"line":3,"event":"set","name":"grid","target":"item 2 of item 1 of grid","from":2,"to":9}`.
- An append to a path that is not a bare name writes `"name"`, `"target"` (the list's path), `"item"` (the new count) and `"to"`.
- The `check` event leaves out of `values` every read whose expression holds an expression in `TypeInfo.narrowed`, and every field or item read of a `Maybe`. This prevents a load through 0 (F6).

- Until Task 20 these tests lower with `lower_ungated(…, Mode::Trace)`.

- [ ] **Step 1: failing tests:**
  - `a_deep_set_names_its_target` (the exact line above)
  - `nothing_is_written_with_its_type` (`{"nothing":"Cell"}`)
  - `nested_values_are_written_whole`
  - `a_check_does_not_read_through_nothing` (`if c is not nothing and value of c is 3` with `c` nothing: the run finishes, and `values` holds `c` only)
- [ ] **Step 2-5.** Commit `Write trace events for nested values`.

**Fail-proof:** a trace that writes `target` for v0 shapes too passes the nested tests. The existing 47 trace cases catch it.
**Review:** sonnet. **Estimate:** ~180 lines (×1.4 = 252). Split it in two: 19a covers `json_value`; 19b covers set, append and check.

### Task 20: Reading and narrating nested traces (`m8/t20-trace-narrate`)

**Files:** `src/trace/{event,narrate,value}.rs`, `src/lower/gate.rs` (deleted), `src/lower/mod.rs`, `tests/trace_cases/*`, `tests/trace_sweep.inc`.

**Interfaces:**
- Decoding:
  - `Value::Nothing(String)` decodes `{"nothing":"Cell"}`.
  - Records hold any value. The error `field "{field}" of a record must be a number, text or truth` is removed, together with its test.
  - `Event::Set` gets `target: Option<String>`.
- Narration (spec §17):
  - `line 6: item 2 of item 1 of grid becomes 9`
  - an append to a path: `{v} is appended to {target} (now N item[s])`
  - `nothing` is said as `nothing`
  - nested values are said as `show` writes them, with D47's three-item cut applied only at the top level
  - the first set of a variable to nothing: `best is nothing`
- The gate is deleted (`gate.rs`, `first_nested`, `lower_ungated`). `irsim::run_source` now calls `lower`.
- Trace cases: `nested_grid`, `nested_linked_list` and `nested_find`, each with `.trace` and `.out`. `MIN_TRACE_CASES` goes to 50.

- [ ] **Step 1: failing tests:**
  - `deep_sets_are_narrated`
  - `nothing_is_said`
  - `nested_values_are_said_in_source_form`
  - `appends_to_paths_are_narrated`
  - the three cases
- [ ] **Step 2-5.** Commit `Narrate nested values and lift the last refusal`.

**Fail-proof:** a narrator that drops `target` and falls back to `name` says `grid becomes 9`. The exact sentence catches it.
**Review:** sonnet. **Estimate:** ~200 lines across about 12 files. Split it in two: 20a covers decoding and narration; 20b removes the gate and adds the cases.

### Task 21: Explain and flowchart cases (`m8/t21-explain-flowchart`)

**Files:** `tests/explain_cases/nested_linked_list.{gloss,out}`, `tests/explain_sweep.inc`, `tests/flowchart_cases/nested_linked_list.{gloss,start.svg}`, `tests/flowchart_sweep.inc`.

**Interfaces:**
- The explain case lists `head: Cell or nothing` among the variables, and the new routines (`work_pop`, `box_value` if reached) in its runtime part.
- The flowchart case draws `repeat while c is not nothing` as an ordinary decision.
- `MIN_EXPLAIN_CASES` goes to 14, and `MIN_FLOWCHART_CASES` to 7.

- [ ] **Steps:** bless the goldens with `GLOSS_BLESS=1`, read them, then run the verify gate. Commit `Add explain and flowchart cases for nested values`.

**Fail-proof:** a blessed golden proves only that the output stays stable. The reviewer checks the variable line and the decision by eye against this task's text.
**Review:** sonnet. **Estimate:** ~120 lines.

---

## After the last task

- **Wave.** The deferred minors are fixed in one dispatch.
- **Final review:** `opus`, over the whole milestone diff against D66 to D72 and §17. One fix dispatch, then a scoped `sonnet` re-review.
- **As built.** The controller writes the design spec's "Milestone 8a" As-built entry and moves `spec/v0.md` §17's opening sentence into the present tense. Both are in their own PR.
- **Then** comes the M8b plan (D73 to D75), whose guard is every case above.
