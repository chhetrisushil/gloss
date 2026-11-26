# Gloss compiler M7: `gloss flowchart` - implementation plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task by task. Steps use checkbox (`- [ ]`) syntax for tracking. Ledger: `.superpowers/sdd/2026-11-27-compiler-m7-flowchart/progress.md`.

**Goal:** `gloss flowchart x.gloss` writes one SVG chart per feature beside the source (`x.<feature>.svg`). Each chart has a start box, one node per statement labelled with its source lines, yes/no branches, loops with back arrows, `skip`/`stop`/`give` arrows and an end box, laid out top to bottom with one node per row.

**Architecture:**
- `src/flowchart/graph.rs` turns a `FeatureDecl` and the source into nodes (in source order) and arrows (D60, D61).
- `src/flowchart/layout.rs` gives every node a row, a column and a box, and every arrow a route through gaps and lanes (D62, D63).
- `src/flowchart/svg.rs` writes the SVG text (D64).
- `Checked::flowcharts` returns `(file name, svg)` pairs, and the CLI writes them (D59).

**Tech stack:** Rust 2024, stable 1.87 or later, no crates. Nothing in M7 runs an executable, so every M7 test runs on the Mac as well as on Linux.

**Spec:** `docs/specs/2026-11-22-compiler-design.md` is binding. For this milestone that means D59 to D65 and §4.11. The language is defined by `spec/v0.md` §15 (Flowchart), §10 and §11 (commands). Conflicts about the language resolve against the language spec; conflicts about the implementation resolve against the design spec.

**How to read the tasks.**
- **Binding:** every exact string (messages, file names, arrow labels, SVG element forms), the tests named in each task with their assertions, the sizes in §4.11, and the types and signatures in each **Interfaces** block.
- **Guidance:** implementation steps give the hook point (file:line at `main` @ d479f81) and the key code; the implementer writes the rest. Line numbers drift as tasks land, so find the named function.
- A test may change only if it contradicts the language spec or the design spec. Record that change as a ruling in the ledger.

## Global constraints

- [D1] Rust 2024 edition, `rust-version = "1.87"`. [D3] No dependencies. [D4] One crate.
- [D18] `src/flowchart/` knows nothing about targets: it reads the AST, the source and nothing else.
- Lowering, the IR and the target are unchanged: every example executable is byte-for-byte the same after M7 (sha256 checked by the controller after the last task).
- [D62] One node per row, in source order. [D63] Every segment is horizontal or vertical; no segment passes through a box.
- [D64] Whole-pixel coordinates; the same program always gives the same bytes. No colours.
- [D59] `gloss flowchart` asks for no approval, runs nothing and works on any host.
- Problem strings never end with a period (as in M1 to M6). Messages to the user are exact.

## Rules for every PR

- **One PR at a time** (owner directive). When the owner asks for a chain, each task's branch is cut from the previous task's branch, and that base is recorded on the `dispatched` line.
- **Size.** At most 200 changed lines (insertions plus deletions; tests and expected files included) and at most 10 files. Estimates run about 40% low: each task states its estimate and that figure times 1.4. A task that still grows past the cap is split into sequential branches (`m7/tNNa-…`, `m7/tNNb-…`), recorded as a ruling. Never trim comments or tests to fit, and never use `#[rustfmt::skip]` or golfed code.
- **Branches:** `m7/tNN-<slug>`.
- **Pull requests:** draft (`gh pr create --draft`), merged with `gh pr merge <n> --rebase` on the owner's word.
- **Commits:** one per branch, ending with
  ```
  Co-Authored-By: Claude Sonnet 5.5 <noreply@anthropic.com>
  Claude-Session: https://claude.ai/code/session_011uGukQR6EwQQdiSVNu4ERC
  ```
  PR bodies end with `🤖 Generated with [Claude Code](https://claude.com/claude-code)`, a blank line, and the session link.
- **Verify gate** at every commit: `CARGO_BUILD_JOBS=4 cargo fmt --check && cargo clippy --all-targets -- -D warnings && cargo test`.
- **Reviews:** implementers and reviewers use `sonnet`. Tasks 3 and 4 (the layout and routing) are **risky**: their first review uses `opus`. Every re-review uses `sonnet`.
- **Branch hygiene:** never delete a branch before `git log --oneline main..<branch>` prints nothing (or `git cherry` shows every commit as `-`).

## Resource limits

- At most 2 agents at once, reviews included. One `cargo` run at a time, with `CARGO_BUILD_JOBS=4`.
- Agents never create extra cargo target directories, give any scratch script a unique name, and never kill a process they did not start.
- An agent whose command is denied stops and reports it. It never works around the denial.

## Findings that shape this plan (verified at `main` @ d479f81)

| # | Finding | Consequence |
|---|---|---|
| F1 | `StmtKind` (`src/ast/stmt.rs:40`) has `If { branches: Vec<(Expr, Vec<Stmt>)>, otherwise }`, `ForEachIn`, `ForEachRange`, `RepeatWhile`, `Skip`, `Stop`, `Give`, `Run`, `Note` and the plain statements; `Stmt::line` is the statement's first line | Task 1 maps each kind to a shape; an `otherwise if` takes its line from its condition `Expr::line` |
| F2 | `otherwise` has no line in the AST | It is not a node (D61), so nothing needs one |
| F3 | A long line is broken after a comma, and each continuation line is one level deeper (spec §3; `format/wrap.rs`). A statement never ends in a comma | A node's label is its first line plus each following line while the previous line ends with `,` (after trimming trailing spaces) |
| F4 | `ast::walk::for_each_expr_of(kind, f)` (`src/ast/walk.rs:187`) visits a statement's own expressions, and `for_each_expr` (`:179`) recurses into one; `ExprKind::Call { feature, inputs }` is a call | Task 1's call shape: any `Call` among a non-block statement's own expressions, or `Run` |
| F5 | `FeatureDecl { line, name, needs: Vec<InputDecl>, notes_before_needs, body }`; `InputDecl::line` | The start label is the feature's line, then each `needs` line in order (deduplicated, as several inputs may share a line) |
| F6 | `lower::fail::spell_invisible` (`src/lower/fail.rs:27`) is `pub(crate)` | Labels use it (D60) |
| F7 | A statement after `give`, `skip` or `stop` in the same block is a warning, not an error (`check/flow.rs:240`) | Such a node is drawn with no incoming arrow (D63) |
| F8 | `cli::run` refuses `flowchart` as "not built yet" (`src/cli.rs:63`), and `tests/cli.rs:103` pins that; `USAGE` already lists `flowchart` | Task 7 changes both |
| F9 | `write_atomically(path, bytes, Some(0o644))` (`src/cli.rs:443`) writes a file that need not exist yet; `.allow` uses it | The SVG files use it (D59) |
| F10 | `tests/explain_cases.rs` and `tests/explain_sweep.inc` hold the bless convention (`GLOSS_BLESS=1` rewrites, creates missing files, then fails on purpose; dotfiles skipped) | Task 8 follows it |
| F11 | `Checked` (`src/lib.rs:59`) keeps `program`, `src` and `types` | Task 7 adds `Checked::flowcharts` |

## File map

| File | Responsibility | Task |
|---|---|---|
| `src/flowchart/mod.rs`, `src/lib.rs` | Module, `charts` entry point; `Checked::flowcharts` | 1, 7 |
| `src/flowchart/graph.rs` | Nodes, labels, shapes, arrows | 1, 2 |
| `src/flowchart/layout.rs` | Rows, columns, boxes, lanes | 3 |
| `src/flowchart/route.rs` | Arrow routes, gaps and tracks | 4 |
| `src/flowchart/check.rs` (cfg test) | Layout invariants and the XML well-formedness check | 5, 6 |
| `src/flowchart/svg.rs` | SVG text | 6 |
| `src/cli.rs`, `tests/cli.rs` | `gloss flowchart` | 7 |
| `tests/flowchart_cases.rs`, `tests/flowchart_sweep.inc`, `tests/flowchart_cases/*` | Golden cases | 8, 9 |
| `spec/v0.md`, `README.md`, design spec | Docs | 10, 11 |

---

### Task 1: Nodes (`m7/t01-graph-nodes`)

**Files:** Create `src/flowchart/mod.rs`, `src/flowchart/graph.rs`. Modify `src/lib.rs` (`pub(crate) mod flowchart`).

**Interfaces:**
- Produces:
  ```rust
  pub(crate) enum Shape { Start, End, Step, Call, Decision }
  pub(crate) struct Node { pub shape: Shape, pub label: Vec<String>, pub line: u32 }
  pub(crate) enum ArrowKind { Next, Yes, No, Each, Done, Skip, Stop, Give }
  pub(crate) struct Arrow { pub from: usize, pub to: usize, pub kind: ArrowKind }
  pub(crate) struct Graph { pub nodes: Vec<Node>, pub arrows: Vec<Arrow> }
  pub(crate) fn graph(feature: &FeatureDecl, src: &Source) -> Graph;
  impl ArrowKind { pub fn label(self) -> Option<&'static str> }
  impl Arrow { pub fn is_back(&self) -> bool { self.to <= self.from } }
  ```
  Derive `Clone, Copy, Debug, PartialEq, Eq` where it fits. `Next` is plain flow and has no label. `Yes` and `No` come out of `if`, `otherwise if` and `repeat while`. `Each` (label `next`) and `Done` (label `done`) come out of a `for each`. `Skip`, `Stop` and `Give` have no label (P2). The kind says how an arrow leaves its node; whether it goes back to a loop's decision is `is_back`. `label` is written in Task 2.
- Nodes in source order: `Start` (label: the feature's line, then each distinct `needs` line in order), one node per non-`note` statement in the order the statements are written (nested blocks in place), then `End` (label `["end"]`, line = the last line of the feature's body).
- Shapes: `if`, each `otherwise if`, `for each` (both forms) and `repeat while` are `Decision`; `run` and any statement with a `Call` among its own expressions (F4) is `Call`; every other statement is `Step`. An `if` chain is one node per condition: the `if` node has the `if` line, each `otherwise if` node its condition's line (F1). Their bodies' nodes come right after their own condition node.
- Labels (D60, F3): each source line trimmed of leading and trailing spaces and passed through `spell_invisible`; continuation lines follow.
- In this task `arrows` stays empty.

- [ ] **Step 1: failing tests** (in `graph.rs`, on programs checked with `crate::check_program`, taking the first feature):
  - `nodes_are_in_source_order`: a feature with `set`, `if`/`otherwise if`/`otherwise`, a `for each` and `show` gives shapes `[Start, Step, Decision, Step, Decision, Step, Step, Decision, Step, End]` and the matching first label lines.
  - `notes_are_not_nodes`: a `note` line and a note block in the body add no node.
  - `a_continued_line_is_one_node`: a `show` broken after a comma onto two lines gives one node with a two-line label, both trimmed.
  - `the_start_box_holds_the_needs_lines`: `feature total_of` plus `needs prices as list of number` gives the start label `["feature total_of", "needs prices as list of number"]`.
  - `calls_have_their_own_shape`: `run report`, `set t to total_of with xs` are `Call`; `if is_big with n` is `Decision`; `show n` is `Step`.
  - `invisible_characters_are_spelled`: a text literal holding U+200B gives a label with `\u{200B}`.
- [ ] **Step 2:** run `cargo test flowchart` and see the failures.
- [ ] **Step 3:** implement.
- [ ] **Step 4:** run the verify gate.
- [ ] **Step 5:** commit: `Turn each feature into flowchart nodes labelled with its source lines`.

**Fail-proof:** a builder that skips nested blocks passes a flat test; the first test has nodes inside an `if` body and a loop body. A label taken from the formatter instead of the source passes on formatted input; the continuation test uses the source's own break.
**Review:** sonnet. **Estimate:** ~140 lines (×1.4 = 196: split 1a types and plain statements, 1b blocks and labels, if it runs past 200).

### Task 2: Arrows (`m7/t02-graph-arrows`)

**Files:** Modify `src/flowchart/graph.rs`.

**Interfaces:**
- The builder walks each block with a list of **open exits** `(from, kind)`, the arrows that still need a target, and a stack of enclosing loops (each with its decision's index and its list of `Done`/`Stop` exits).
  - A plain node takes every open exit as an arrow to itself, and leaves `[(node, Next)]` open.
  - An `if` chain: the open exits go to the first condition. Condition `i`'s `Yes` goes to its body's first node (or, for an empty body, stays open as `(cond, Yes)`). Its `No` goes to the next condition, or to the `otherwise` body's first node, or, with no `otherwise`, stays open as `(last cond, No)`. After the chain, the open exits are every branch's open exits plus that `No`.
  - A loop: the open exits go to the decision. Its body starts from `[(decision, Each)]` for `for each` or `[(decision, Yes)]` for `repeat while`. The body's open exits become arrows back to the decision, each keeping its kind (a `No` left open by an `if` at the end of the body stays `No`, labelled `no`). After the loop, the open exits are `(decision, Done)` for `for each` or `(decision, No)` for `repeat while`, plus every `stop` exit of this loop.
  - `skip`: takes the open exits, then adds `Skip` from itself to the innermost loop's decision; nothing stays open. `stop`: takes the open exits, then its exit `(self, Stop)` is held by the innermost loop until the loop ends. `give`: takes the open exits, then adds `Give` from itself to the end node; nothing stays open.
  - At the end of the feature, the open exits go to the end node.
- Arrow order: in the order they are made. `ArrowKind::label`: `Yes` → `yes`, `No` → `no`, `Each` → `next`, `Done` → `done`, others `None`. A `No` out of `repeat while` reads `no`, out of `for each` the kind is `Done`.

- [ ] **Step 1: failing tests** (pairs `(from, to, kind)` listed exactly):
  - `straight_line_arrows`: start → 1 → 2 → end, all `Next`.
  - `an_if_chain_joins_below`: `if`/`otherwise if`/`otherwise` then `show`: both conditions' `Yes` into their bodies, `No` into the next condition and into the otherwise body, and every body's last node into the `show`.
  - `an_if_without_otherwise_falls_through_on_no`: the condition's `No` goes to the next node.
  - `a_loop_goes_back_and_out`: `for each` with a two-statement body: `Each` into the body, a `Next` back from the last body node to the decision (`is_back`), `Done` to the next node. `repeat while` gives `Yes` and `No` instead.
  - `jumps`: in a `for each` with `if … skip`, `if … stop` and `give`: `Skip` to the decision, `Stop` to the node after the loop, `Give` to the end node, and the node after `give` in the same block has no incoming arrow.
  - `nested_loops`: a `stop` in the inner loop goes to the node after the inner loop, which is inside the outer loop.
- [ ] Steps 2 to 5. Commit: `Connect flowchart nodes with branch, loop and jump arrows`.

**Fail-proof:** a builder that sends `stop` to the end of the outermost loop passes a single-loop test; `nested_loops` catches it.
**Review:** sonnet. **Estimate:** ~150 lines (×1.4 = 210: split 2a ifs and loops, 2b jumps and nesting, if needed).

### Task 3: Boxes (`m7/t03-layout-boxes`) - risky

**Files:** Create `src/flowchart/layout.rs`. Modify `src/flowchart/mod.rs`.

**Interfaces:**
- Produces:
  ```rust
  pub(crate) struct Rect { pub x: i32, pub y: i32, pub w: i32, pub h: i32 }
  pub(crate) struct Layout {
      pub width: i32, pub height: i32,
      pub boxes: Vec<Rect>,          // one per node, same index
      pub column: Vec<usize>,        // one per node
      pub loops: BTreeMap<usize, (i32, i32)>, // loop decision node -> x of its back and done lanes
      pub give_lane: i32,
      pub gaps: Vec<i32>,            // y of the top of the gap above each row (row 0: none)
      pub routes: Vec<Vec<(i32, i32)>>, // filled by Task 4; empty here
  }
  pub(crate) fn layout(g: &Graph) -> Layout;
  ```
- Sizes (§4.11, binding): text is 9px per character and 20px per line. A `Step`, `Call`, `Start` or `End` box is `9 × longest line + 24` wide and `20 × lines + 16` high. A `Decision` is the same text box plus half its height at each end. Each row's top is the previous row's bottom plus its gap; a gap is 24px plus 10px for each track beyond the first (Task 4 fills in the track counts; this task uses one track per gap). The chart has a 16px margin all round.
- Columns (D62): a block is laid out in a column `c`. In an `if` chain, condition `i` and its body are in column `c_i`; `c_0 = c`, and `c_{i+1}` is one past the last column used by branch `i`, its loops' lanes included. The `otherwise` body (or, with no `otherwise`, an empty column for the `no` path) is one past the last condition's branch. A loop's body is in the loop's column. A column's centre is its left edge plus half its widest node; each node is centred on its column.
- Lanes (D63): each loop owns a back lane 16px left of the leftmost of its span's boxes (its decision and every node of its body) and of its inner loops' back lanes, and a done lane 16px right of the rightmost of its boxes and its inner loops' done lanes. Columns are placed so that every back lane is at least the 16px margin from the left edge, and a column to the right of a loop's span starts 16px past its done lane. The give lane is 16px right of everything else.

- [ ] **Step 1: failing tests:**
  - `sizes`: a one-line `Step` with 10 characters is 114 × 36; a two-line one is 56 high; a `Decision` with 10 characters is 114 + 36 wide.
  - `one_node_per_row`: for the Task 2 programs, every node's `y` is greater than the previous node's `y + h`.
  - `an_if_chain_steps_right`: in an `if`/`otherwise if`/`otherwise`, the second condition's column is greater than the first's, the otherwise body's greater still, and the node after the chain is in the first column.
  - `boxes_never_overlap_in_x_across_columns`: for two nodes in different columns, the right edge of the left column's widest node is left of the right column's left edge.
  - `loop_lanes_bracket_the_loop`: the back lane is left of every box in the loop's span, the done lane is right of them, and an inner loop's lanes are between the outer's.
- [ ] Steps 2 to 5. Commit: `Lay out flowchart nodes one per row in branch columns`.

**Fail-proof:** a layout that puts every node in column 0 passes `sizes` and `one_node_per_row`; the column tests catch it.
**Review:** opus first, then sonnet. **Estimate:** ~170 lines (×1.4 = 238; pre-split: 3a sizes and rows, 3b columns and lanes).

### Task 4: Routes (`m7/t04-layout-routes`) - risky

**Files:** Create `src/flowchart/route.rs`. Modify `src/flowchart/layout.rs`.

**Interfaces:**
- `layout` fills `routes`, one polyline per arrow (start point first, last point on the target's edge), every segment horizontal or vertical:
  - `Next`, `Yes`, `Each` straight down in the same column: bottom centre to top centre.
  - Any forward arrow whose target is in another column, or not on the next row: down its own column from the source's bottom centre (from a decision's right point for `No`, `Done`, `Stop`: see below) to its track in the gap above the target's row, across to the target's column, down into its top centre.
  - `No` out of a condition: right from its right point to the next condition's column (or the `otherwise`/empty column), then down. `Done` out of a loop and every `Stop`: right to the loop's done lane (from the decision's right point, or a `stop` box's right edge), down the lane to its track in the gap above the target's row, across, down.
  - Back arrows (`is_back`) other than `Skip`: leave as a forward arrow of the same kind would (down from the bottom centre, or right from a decision's right point to its `no` column and down), go to their track in the gap below the loop's last row, left to the loop's back lane, up the lane to the decision's row, and right into the decision's left point.
  - `Skip`: left from the `skip` box's left edge to the back lane, then up and in as above.
  - `Give`: right from the `give` box's right edge to the give lane, down to its track in the gap above the end row, across to the end's column, down.
- Tracks: in each gap, one track per distinct target among the arrows that run across it, ordered by target row and then by arrow kind. The gap's height follows (Task 3's rule).
- Arrows with the same target may share segments; arrows with different targets never run along the same line for any length.

- [ ] **Step 1: failing tests:**
  - `every_segment_is_straight`: for the Task 2 programs, each pair of consecutive points shares an x or a y.
  - `routes_end_on_their_target`: the last point is on the target box's top edge, or at a decision's left point for a back arrow.
  - `a_no_arrow_leaves_from_the_right_point`, `a_back_arrow_uses_the_back_lane`, `stop_and_done_share_the_done_lane`, `give_uses_the_give_lane`: the exact x of the relevant segment equals the lane or point from Task 3.
  - `two_targets_two_tracks`: an `if` with no `otherwise` at the end of a loop body, followed after the loop by a node: the back arrows and the loop's `Done` arrow cross the gap below the loop on different tracks, so that gap is 34px.
- [ ] Steps 2 to 5. Commit: `Route flowchart arrows through gaps and loop lanes`.

**Fail-proof:** routes that go straight from point to point diagonally pass `routes_end_on_their_target`; `every_segment_is_straight` catches them. A router that gives every arrow the same track passes the lane tests; `two_targets_two_tracks` catches it.
**Review:** opus first, then sonnet. **Estimate:** ~180 lines (×1.4 = 252; pre-split: 4a forward arrows and tracks, 4b back, skip and give).

### Task 5: Layout invariants (`m7/t05-layout-invariants`)

**Files:** Create `src/flowchart/check.rs` (`#[cfg(test)]`). Modify `src/flowchart/mod.rs`.

**Interfaces:**
- `fn invariants(g: &Graph, l: &Layout) -> Result<(), String>`, the error naming the first broken rule and the nodes or arrows involved:
  1. No two boxes overlap (with the decision's pointed ends counted).
  2. No segment passes through a box's interior, except the first and last segment of a route, which may end on their own source or target edge.
  3. Two segments of arrows with different targets are never collinear and overlapping for more than a point.
  4. Every segment is horizontal or vertical.
  5. Every non-`note` statement of the feature is exactly one node (count by walking the AST), and the chart fits in `width` × `height`.
- Test `every_example_and_program_lays_out_cleanly`: every feature of every `examples/*.gloss` and of the Task 1 and 2 programs passes. It asserts that at least 12 example files were found.
- Negative tests: each rule fails on a hand-broken layout (two boxes moved together; a route pushed through a box; two different targets on one track; a diagonal; a node removed).

- [ ] Steps 1 to 5. Commit: `Check every flowchart layout for overlaps and crossings`.

**Fail-proof:** a checker that always passes is caught by the five negative tests.
**Review:** sonnet. **Estimate:** ~170 lines (×1.4 = 238; pre-split: 5a rules 1, 4, 5 with their negatives, 5b rules 2 and 3).

### Task 6: SVG (`m7/t06-svg`)

**Files:** Create `src/flowchart/svg.rs`. Modify `src/flowchart/check.rs` (the XML check) and `src/flowchart/mod.rs`.

**Interfaces:**
- `pub(crate) fn svg(title: &str, g: &Graph, l: &Layout) -> String`, with the elements of §4.11 in this order, one per line:
  1. `<svg xmlns="http://www.w3.org/2000/svg" width="W" height="H" viewBox="0 0 W H" font-family="monospace" font-size="15">`.
  2. `<title>TITLE</title>`, where TITLE is `x.gloss: feature NAME`.
  3. `<defs><marker id="arrow" viewBox="0 0 10 10" refX="10" refY="5" markerWidth="8" markerHeight="8" orient="auto"><path d="M0,0 L10,5 L0,10 z"/></marker></defs>`.
  4. `<rect width="100%" height="100%" fill="white"/>`.
  5. Each node: `<rect x y width height fill="white" stroke="black"/>` (start and end add `rx="12"`; a call adds two `<line>`s 6px inside each side) or `<polygon points="…" fill="white" stroke="black"/>` for a decision; then one `<text x y>` per label line, left-aligned 12px inside the text box (plus the decision's pointed end), baselines 20px apart starting 23px below the box's top.
  6. Each arrow: `<polyline points="x,y x,y …" fill="none" stroke="black" marker-end="url(#arrow)"/>`, and for a labelled arrow `<text x y font-size="12">LABEL</text>` 4px right of and 12px below its first point.
  7. `</svg>` and a newline.
- `fn escape(&str) -> String` replaces `&`, `<`, `>`, `"` with entities.
- `pub(crate) fn charts(program: &Program, src: &Source, name: &str) -> Vec<(String, String)>` in `mod.rs`: for each feature, `(<stem>.<feature>.svg, svg(...))`, where `<stem>` is `name` without `.gloss`.
- In `check.rs`: `fn well_formed(xml: &str) -> Result<(), String>`, a small checker that every tag opens and closes in order, attributes are quoted, and every `&` starts one of the four entities.

- [ ] **Step 1: failing tests:**
  - `a_one_statement_chart`: the full SVG of `feature start` + `show 1` equals a literal string in the test (read it before committing it).
  - `labels_are_escaped`: `show "a<b & \"c\""` gives `&lt;`, `&amp;` and `&quot;` in its text, and `well_formed` passes.
  - `well_formed_rejects`: an unclosed tag, a bare `&` and an unquoted attribute each fail.
  - `every_example_chart_is_well_formed` (in the Task 5 sweep).
- [ ] Steps 2 to 5. Commit: `Write flowcharts as SVG`.

**Review:** sonnet. **Estimate:** ~170 lines (×1.4 = 238; pre-split: 6a the writer, 6b the XML check and sweep).

### Task 7: `gloss flowchart` (`m7/t07-flowchart-command`)

**Files:** Modify `src/lib.rs`, `src/cli.rs` and `tests/cli.rs`.

**Interfaces:**
- `Checked::flowcharts(&self, name: &str) -> Vec<(String, String)>` calls `flowchart::charts`.
- In `cli::run`, `"flowchart" => 0`. `flowchart(path, name, text, out, err)` does the following:
  1. Prints the diagnostics as `explain` does. Any error gives status 3 and writes nothing.
  2. For each `(file, svg)`, writes `<source's folder>/<file>` with `write_atomically(…, Some(0o644))`. On success it prints `Wrote <file>` on `out`. On failure it prints `<file> cannot be written: <reason>` on `err` and returns 3 at once.
  3. Otherwise status 0.
- `--allow` with `flowchart` is a usage error (`ALLOW_COMMANDS` stays `["build", "trace"]`). The usage text is unchanged.

- [ ] **Step 1: failing tests** (in `tests/cli.rs`, in a temp folder):
  - Replace the "not built yet" test: no command is unbuilt now. Keep a test that an unknown command prints the usage and gives 3.
  - `flowchart_writes_one_svg_per_feature`: a program with `start` and `helper` gives status 0, output `Wrote x.start.svg\nWrote x.helper.svg\n`, and both files exist and start with `<svg `.
  - `flowchart_of_a_program_with_errors_writes_nothing`: status 3, the folder holds only `x.gloss`.
  - `flowchart_with_allow_is_a_usage_error`.
  - `flowchart_of_a_table_program_asks_nothing`: a program with a table charts with status 0 and a prompt that panics if asked.
  - `flowchart_reports_a_file_it_cannot_write`: a folder named `x.start.svg` in the way gives status 3 and `x.start.svg cannot be written: ` on standard error.
- [ ] Steps 2 to 5. Commit: `Add gloss flowchart`.

**Review:** sonnet. **Estimate:** ~140 lines (×1.4 = 196).

### Task 8: Flowchart cases (`m7/t08-flowchart-cases`)

**Files:** Create `tests/flowchart_cases.rs`, `tests/flowchart_sweep.inc`, `tests/flowchart_cases/{straight,if_chain}.gloss` and their `.svg` files.

- The harness checks each `<case>.gloss`, calls `flowcharts("<case>.gloss")`, and compares each SVG with `<case>.<feature>.svg` beside it, printing the first differing line. A missing or extra `.svg` is a failure.
- The sweep (as `explain_sweep.inc`, F10) accepts `<case>.gloss` and `<case>.<anything>.svg` whose `<case>.gloss` exists. `MIN_FLOWCHART_CASES = 2`.
- `GLOSS_BLESS=1` writes the SVG files (creating missing ones) and then fails on purpose.
- The layout invariants (Task 5) also run over every case: add the case folder to Task 5's sweep.
- Every SVG is looked at before it is committed: the controller renders each one to an image and checks it by eye.

**Review:** sonnet. **Estimate:** ~180 lines, mostly SVG (×1.4 = 252; pre-split: 8a harness plus `straight`, 8b `if_chain`).

### Task 9: More cases (`m7/t09-more-cases`)

**Files:** `tests/flowchart_cases/{loops,jumps,calls,continued}.gloss` and their SVGs; the sweep floor.
- `loops`: nested `for each` and `repeat while`. `jumps`: `skip`, `stop` and `give` inside an `if` inside a loop. `calls`: `run` and a call in a `set`, and a feature with `needs`. `continued`: a statement broken across two lines and a label with an invisible character. `MIN_FLOWCHART_CASES = 6`.

**Review:** sonnet. **Estimate:** one or two cases per branch (9a to 9d), each under 200 lines.

### Task 10: The language spec and README (`m7/t10-spec`)

**Files:** `spec/v0.md`, `README.md`.
- §15 Flowchart: add the start and end boxes, the shapes, the `next`/`done` labels, the one-row-per-node layout, `note` lines and tests not charted, and the D64 style. §11: `gloss flowchart` prints `Wrote …` per file; the write failure message and status 3; `--allow` is a usage error; it works on any host. The exit-status table's row 3 names the flowchart write failure. Add a row to the design-changes table if any of this changes what the spec said before.
- README: milestone 7 Done, a Flowchart section with the command and the file naming, and the command table row.

**Review:** sonnet. **Estimate:** ~100 lines.

### Task 11: As built (`m7/t11-as-built`)

**Files:** design spec §6 "As built - Milestone 7" (what shipped, rulings, deviations, tests, known limits such as wide characters).
**Review:** sonnet. **Estimate:** ~60 lines.

---

## Rulings made while planning

- P1: the chart is built from the AST, not the IR: the IR has lost `if` chains and loops as structure, and spec §15 asks for source lines as labels. Cost if wrong: none for v0; the step-through viewer (v0.1) also highlights source lines.
- P2: `ArrowKind` has `Each` (labelled `next`) as well as `Next` (plain flow, unlabelled), so only a `for each` shows `next`. There is no `Back` kind: an arrow to an earlier node is a back arrow and keeps the kind it left with, so a `no` that loops back is still labelled `no`. Cost if wrong: one label.
- P3: the end node's line is the last line of the feature's body, so every node has a line for the step-through viewer to highlight. Cost if wrong: one field.
- P4: SVG golden files are reviewed by rendering them, not by reading them; the invariants (Task 5) say what a correct layout is. Cost if wrong: a chart that passes the invariants but reads badly, caught at the controller's visual check.

## Self-review

- Spec coverage:
  - D59: Task 7.
  - D60: Task 1.
  - D61: Tasks 1, 2 and 6.
  - D62: Task 3.
  - D63: Tasks 2 and 4.
  - D64: Task 6.
  - D65: Tasks 5, 8 and 9.
  - Spec §15 bullets: one chart per feature (7), statements are boxes (1), yes/no and loop arrows (2, 4), `skip`/`stop`/`give` (2, 4), calls (1, 6), labels as source lines (1), SVG files named `source.feature.svg` (6, 7), top-to-bottom layout (3).
- Types are used the same way across tasks: `Graph`, `Node`, `Arrow`, `Shape` and `ArrowKind` (Tasks 1 and 2) in 3 to 6; `Layout` and `Rect` (Task 3) in 4 to 6; `charts` (Task 6) in 7; `Checked::flowcharts` (Task 7) in 8.
