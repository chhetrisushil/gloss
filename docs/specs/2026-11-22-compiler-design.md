# Gloss v0 compiler - design spec

- **Date:** 2026-11-22
- **Status:** built (milestones 1 to 4, section 6); milestones 5–7 not started
- **Context:** Implements the language defined in `spec/v0.md` (the *language spec*). This document decides *how* the compiler is built. Where the two disagree about the language, the language spec wins. Where they disagree about the implementation, this document wins.
- **Plans:** `docs/plans/2026-11-22-compiler-m1-front-end.md` (milestone 1). Milestone 2: `docs/plans/2026-11-23-compiler-m2-codegen.md`. Milestone 3: `docs/plans/2026-11-24-compiler-m3-tables-files.md`. Milestone 4: `docs/plans/2026-11-24-compiler-m4-tests.md`. Later milestones get their own plans.

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
| D18 | How is code generated? | Through a target-neutral **Gloss IR** (section 4.6). `lower` turns the AST into IR; a backend for each CPU turns IR into that CPU's instructions; an encoder and an object writer produce the file. | The compiler must later produce executables for other CPUs and OSes. Only the backend, encoder, object writer and OS primitives are per target. Generated code, the IR, `lower`, the runtime and `irsim` never name an instruction, register, system-call number or Linux flag value, and never decode a wait status; host code such as `testrun` may decode an `ExitStatus`. Owner, 2026-11-23. |
| D19 | Where is the runtime written? | Once, in Gloss IR. Only the OS primitives (write, read, get memory, exit, stack size, and in M3 open and rename) differ per target. | A new target must not mean rewriting number printing, 128-bit arithmetic, lists, copying and equality by hand |
| D20 | Which targets does v0 ship? | x86-64 Linux only, static ELF, direct system calls. The `Target` interface leaves room for arm64 and for macOS and Windows, which import their OS primitives from `libSystem` and `kernel32` as declared borrowed code. | The language spec's v0 target is unchanged |
| D21 | How is the runtime tested without Linux? | `irsim`, a test-only IR interpreter. It is never shipped and is not a second execution engine; the real executables stay the authority, and both must agree on every `tests/run` case. | Fast checks on any host, and a reference comparison against the Rust `number` module |
| D22 | What does M2's `gloss build` do with a program that declares a table? | It refuses: `tables arrive in milestone 3`, exit status 3, no executable written | No throwaway in-memory tables. Owner, 2026-11-23. |
| D23 | In what order is milestone 3 built? | One milestone, one plan: build-time approval first (permission list, `.allow`, `--allow`, the question, status 4, `--permissions`), then files at run time. The D22 refusal is lifted when tables first live in memory, after approval is in; until each statement's task lands, `load` and `save` are refused as statements this compiler cannot build yet. | `main` never builds an executable that touches files without approval. Owner, 2026-11-24. |
| D24 | How does the permission list show a file name? | A name of printable ASCII prints as it is. Any other name prints quoted, followed by the same name with every character outside printable ASCII as `\u{…}` in upper-case hex without padding, in parentheses: `"résumé.csv" (r\u{E9}sum\u{E9}.csv)`. The same form is used at build, in `.allow` and by `--permissions`. | Lookalike letters and non-normalised forms cannot pass as another name, and names in other languages stay readable. Owner, 2026-11-24. |
| D25 | What does a build that needs approval do when standard input is not a terminal? | It prints the list, then `Not approved: <files>. Build in a terminal to answer, or add --allow <file>.`, writes no executable and exits 4. It never reads an answer from a pipe. | An agent or script must approve with `--allow`, which shows in its command; piping `yes` in would weaken what an answer means. Owner, 2026-11-24. |
| D26 | Wording of run-time failures 6–8 | As in §4.7, one message per cause, naming the file and, for bad data, the file line where the row starts | Owner, 2026-11-24 |
| D27 | Which builds print the permission list? | Only programs that declare a table. The language spec says every build; a program with no tables would print only `nothing else`, so it prints `Built x` as in M2. `--permissions` works on every executable. | Avoids noise on every build. Deviation from spec §8, accepted by the owner, 2026-11-24. |
| D28 | What does `.allow` hold? | Exactly the current program's list after every successful build, written atomically and only when it changes. A file the program no longer uses, or no longer saves, loses its approval. | The safer direction: adding access back needs a new approval, and every change shows in a diff. Owner, 2026-11-24. |
| D29 | How are files tested? | Run cases carry a `<name>.files/` folder copied in before the run and a `<name>.after/` folder that must match the folder afterwards exactly; symbolic links come from `<name>.links`. `irsim` gets an in-memory file system seeded from the same folder, so file cases are differential too (D21). | One case format for real runs and `irsim`; links are not committed to git. |
| D30 | How is milestone 4 split? | Two parts, as D5 says. **M4a plain tests:** the `gloss test` CLI, the fork driver, `expect`, run-time failures inside tests, and tables that start empty (`append` works; no `load` or `save`). **M4b fixtures:** `input`, `shown` and pretend files. Fixtures are a second backend for the runtime's console and file routines, used only in test builds; the CSV reading and writing code is shared, and only where the bytes come from and go to changes. | Plain tests work first, as the language spec's build order says (§12 step 3); fixtures can slip to v0.1 without touching anything else. Owner, 2026-11-24. |
| D31 | What is the test executable? | `lower` gets a test mode (`lower_tests`) whose entry `_start` runs a **driver**, not `start`. For each test, in source order, the driver flushes output, forks and waits. The child runs the test and prints its own block, `pass  <title>` or `fail  <title>` followed by the details indented 6 spaces, then exits 0 for pass or 1 for fail. The parent counts the results and finally prints a blank line and `N passed, M failed`. A child that ends any other way gets `fail  <title>` and `      ended without a result (status N)` from the parent. Each child starts from the parent's untouched state, so tables start empty and counted as cleared. | A forked child needs no unwinding and no way to catch a failure: the failure routines already end the process. Isolation is free. Owner, 2026-11-24. |
| D32 | How do the driver and the child talk to the operating system? | Three new primitives, `Fork`, `Wait` and `DefaultChildSignal`. Their Linux numbers (`fork` 57, `wait4` 61, `rt_sigaction` 13 with `SIGCHLD` 17) and the decoding of the wait status live only in `src/target/` (D18). `DefaultChildSignal` gives `SIGCHLD` its default action: a test executable started by a parent that ignores `SIGCHLD` would otherwise have every `Wait` give -10 (the kernel reaps ignored children), so each test would pass in its child and be reported lost. The test `_start` calls it first; `irsim` does nothing for it, having no signals. `Wait(pid)` gives the child's exit status (0 to 255), or `256 + signal` when a signal ended it, or a negative error. `irsim` models `Fork` by cloning the machine and running the clone to its end inside the call (D21). | Everything else the driver does is runtime written in IR (D19). Owner, 2026-11-24. |
| D33 | What does `gloss test` print? | The owner's preview, exactly:<br>`pass  category total counts only the matching category`<br>`fail  largest expense is found`<br>`      line 23: expect name of largest_expense is "Dinner"`<br>`      shown:`<br>`        Largest so far: Bus at 40`<br>`pass  a cost that is not a number is asked for again`<br>(blank line)<br>`2 passed, 1 failed`<br>The `shown:` part appears only if the test printed something. Titles print as written; titles are checked unique already. | It reads like the language spec's promise (a non-programmer can see which promise broke). Owner, 2026-11-24. |
| D34 | What does a failure inside a test do? | A failed `expect` stops the test and reports `line N: expect <condition as gloss format writes it>`. A run-time failure prints its normal message (`line 5: failure 1: dividing by zero (people is 0)`) under the title, not on standard error, and fails only that test. `show` in a test is quiet: it appends to `shown`. `ask` prompts are not part of `shown`. | Matches the language spec: output appears only for a failing test, under its title. Owner, 2026-11-24. |
| D35 | What does `ask` do in a test? | It takes the next `input` answer. With none left the test fails with `the test gave 2 answers but line 31 asked a third question`. Standard input is never read. | The language spec's own wording (§9). Singular and plural forms are ruled in §4.8. |
| D36 | How do tables and files behave in a test? | Each table has an in-memory file that starts missing. `save` and `load` use the real CSV writer and reader (`csv_text`, the `table_read` parser), so failures 6 to 8 live in the same code. A test build contains no real file primitive: its IR has no `Open`, `Rename`, `Sync`, `Mode`, `SetMode`; a lowering test asserts it. | Tests need no approval and cannot touch a real file (§9). One CSV implementation means a test of `load` tests the real parser. Owner, 2026-11-24. |
| D37 | What does `gloss test x.gloss` do? | Checks the program, tests included. Builds the test executable into a private temporary folder (mode 0700), runs it, deletes it. Never writes beside the source, never reads or writes `.allow`. Exits 0 if all tests pass, 1 if any fails, 3 on a compile error. A program with no tests prints `No tests in x.gloss` and exits 0. | Language spec §11: `test` never asks. Owner, 2026-11-24. |
| D38 | `gloss test` on a host that cannot run x86-64 Linux programs? | It refuses with `gloss test runs tests on Linux only` and status 3. A ruling, recorded as a deviation risk: the language spec says `gloss test` "builds and runs" without naming a platform. | The compiler does not ship an interpreter (§1), so it can only run what the OS can run. Owner, 2026-11-24. |
| D39 | How is M4 tested? | New cases `tests/test_cases/<name>.gloss` with `.out` (standard output) and `.status`, run as real executables through `GLOSS_RUNNER` and under `irsim`, which must agree (D21). The CLI's build step is a library function (`build_test_executable`) so the harness can build a test executable on the Mac and run it through the runner. The language spec's §13 program becomes a case; the examples gain tests. Negative cases show isolation: a failing `expect`, failure 10 in one test and a missing `input`, while the other tests still pass. | One case format for both engines, as D29. Owner, 2026-11-24. |
| D40 | Accepted risk: a test that loops forever. | The run hangs. v0 has no time limit. | A limit needs a clock primitive and a policy; neither is worth it before someone hits it. Owner, 2026-11-24. |
| D41 | How is milestone 5 split? | Two parts. **M5a, the event stream:** `lower_trace` and the trace backend; the traced executable writes the events of spec §10 as JSON lines. **M5b, the narrator and the CLI:** `gloss trace` runs that executable, writes `x.trace` and prints the sentences. | M5a is testable on its own (real executable against `irsim`, like D13), and M5b's narrator is testable on the Mac from recorded event files. Owner, 2026-11-25. |
| D42 | Who turns events into sentences? | `gloss` does, in Rust (`src/trace/`). The traced executable writes only events; the narrator reads them, with the checked program at hand for each line's syntax tree. | One serializer in IR instead of two; the sentences are plain Rust and easy to test; the runtime that Explain prints stays smaller. Owner, 2026-11-25. |
| D43 | What does the traced executable write, and where? | One JSON event per line on standard output, through the normal output buffer, and nothing else. The buffer is flushed before `ask` reads, when the program fails and when it ends. What a normal build prints becomes events: each `show` line is a `show` event, `ask`'s question is a `show` event followed, once answered, by an `ask` event, and a run-time failure is a `fail` event, after which the program exits 2. | A single stream keeps the order exact with no framing: the narrator prints program lines and sentences in the order the events arrive. Owner, 2026-11-25. |
| D44 | When is a `set` event written, and what does it carry? | For `set`, `add`, `subtract`, `multiply` and `divide`, on a variable, a field (`amount of expense`) or an item (`item 2 of prices`): the new value is computed first (so any events from calls inside it come first), then the event is written while the old value is still in place, then the value is stored. A variable's first `set` has no `from`. `append` writes a `set` event with `"item": N` and only the new item, never the whole list. | Writing whole lists on every `append` would make building a table of `n` rows cost `n²`. Owner, 2026-11-25. |
| D45 | What does a `check` event carry? | The condition as `gloss format` writes it, and `values`: each variable and field read (`amount of expense`) named in it, read after the condition is evaluated. `item` reads are left out, since reading them can fail. `if`, `otherwise if` and `repeat while` write one; `otherwise` writes none. | The narrator rebuilds the sentence from the condition's syntax tree and these values. Owner, 2026-11-25. |
| D46 | How are calls narrated? | `enter` is written when a feature starts, with its inputs; `leave` when it gives a value or ends. Both carry the line of the call. The narrator indents each sentence 2 spaces per call level, so recursion shows as a staircase. | Owner, 2026-11-25 (indent chosen over a flat list). |
| D47 | How are values written in sentences? | As `show` writes them, with text in quotes. A list shows its first 3 items then `… (N items)`; text longer than 60 characters is cut with `…`. The `.trace` file always holds full values. | Owner, 2026-11-25. |
| D48 | What does `gloss trace x.gloss` do? | Checks the program (errors: status 3), asks for approval of its files exactly like `build` (`--allow` works; refusal: status 4), refuses on a host that cannot run x86-64 Linux programs (status 3, as D38), builds the traced executable into a private folder (as D37), and runs it with the current folder as its working directory and standard input inherited. It writes the events to `x.trace` beside the source (replaced each run; if it cannot be written, status 3) and narrates them. It exits 0 when the program ends normally, 2 after a `fail` event, and 2 with `the traced run ended without finishing (status N)` on standard error for any other end. | Owner, 2026-11-25. |
| D49 | Accepted risk: a run with millions of events. | Trace narrates every event; there is no limit. Ctrl-C stops it. | Nothing is hidden, and a cap would need a policy nobody has asked for yet. Owner, 2026-11-25. |
| D50 | How is M5 tested? | `tests/trace_cases/<name>.gloss` with the expected `<name>.trace`, run as a real executable and under `irsim` (the two must agree, as D13 and D39), with a sweep floor `MIN_TRACE_CASES`. The narrator has golden tests from recorded `.trace` files to `.out` sentences, which run on any host. `tests/cli.rs` covers `gloss trace` on Linux. | Owner, 2026-11-25. |
| D51 | What does Explain print, and from what? | The instructions of the real build (`lower`, `Backend::Real`), exactly as encoded. `target::x86_64::program` gets `assemble(&Module) -> Assembly` (each function's typed instructions, then the x86 helpers, in layout order); `executable` encodes that assembly and `explain` prints it. No cleaned-up or improved form. | What a reader sees is what runs (spec rule 6). The encoder's text already agrees with GNU objdump instruction by instruction (`tests/objdump.rs`), so printing the assembled list closes the chain. Owner, 2026-11-26 (real code over a cleaned-up view or peephole codegen). |
| D52 | In what order are instructions printed? | Machine order. Each `Line` marker starts a header `line N: <source line>`; when a line's code resumes after another line's, its header is repeated with ` (continued)`. A failure block (after a feature's `Return`) starts a header `failure C, line N`. As built (M6-R9, R10): the feature's entry code, its end guard and its implicit return are listed under the feature's line. | Jumps and labels then read correctly top to bottom. Owner, 2026-11-26 (machine order over source order). |
| D53 | What does Explain comment? | Only what the instruction does not show: the variable or input a frame slot holds (`; total`), the text a read-only address points at (`; "Total: "`, cut at 30 characters with `…`), the failure a jump leads to (`; failure 4: a number went out of range (price * 1000000)`: the failure's message as lowering knows it, with any part known only at run time written `…`), and the Linux call a `syscall` makes (`; write`). Every other instruction is bare. Comments start in one column per function. | The comments carry names a reader cannot recover; a gloss on every instruction would be noise. Owner, 2026-11-26. |
| D54 | Where are inferred types listed? | At the top of each feature, after its header `feature NAME (line N)`: each input (`rows: list of expense, input 1, at [rbp+24]`), then each variable (`total: number, created on line 13, at [rbp-8]`) and loop variable (`r: expense, loop variable on line 14, at [rbp-16]`), in the checker's order. | A reader knows what a slot holds before reading the code that uses it. Owner, 2026-11-26 (per feature over beside the line or one list). |
| D55 | How is the runtime printed? | After the program, under `runtime`, every routine the executable contains, in layout order, each under `routine NAME`: the IR runtime (with its system calls, inline as in the executable) and the x86 helpers. Labels inside a function print as `.L<n>`. A routine has no comments other than D53's text, syscall and failure ones. | Spec §10: nothing the executable runs is left out. |
| D56 | What does `gloss explain x.gloss` do? | Checks the program (errors: printed as `build` prints them, status 3; warnings printed, run continues), then prints the listing on standard output and exits 0. It asks for no approval, writes no file, runs nothing, and works on any host. Test blocks are left out, as in `build`. `--allow` is a usage error. A failed write to standard output is `cannot write the explanation` and status 3. | Explain only reads the program. Owner, 2026-11-26. |
| D57 | How is M6 tested? | `tests/explain_cases/<name>.gloss` with `<name>.out`: the listing up to and including the `runtime` line, then the names of its routines, one per line (the routine bodies are not golden). A sweep floor `MIN_EXPLAIN_CASES`. Invariant tests: `executable(m)` equals the ELF of `encode(assemble(m))`; the listing's instruction lines equal the text of every assembled instruction, in order (labels renamed by D55's rule); every function label in the encoded code has a `routine` or `feature` header. Spec §10's example is a case, pinned by `tests/spec_examples.rs`. | Golden routine bodies would rewrite every case on every runtime change; the invariants cover them. Owner, 2026-11-26. |
| D58 | Spec §10's example shows idealised code (`add [rbp-8], rax`). | Replace it with real output from the `spec_explain` case, and note the change in the spec's design-changes table. | D51. Owner, 2026-11-26. |
| D59 | What does `gloss flowchart x.gloss` do? | Checks the program (errors: printed as `build` prints them, status 3, no file written; warnings printed, run continues). Then, for each feature in source order, writes `x.<feature>.svg` beside the source through `write_atomically` with mode 0644 and prints `Wrote x.<feature>.svg`. Exits 0. A file that cannot be written prints `x.<feature>.svg cannot be written: <reason>` and exits 3 at once; files already written stay. It asks for no approval, runs nothing, and works on any host. `--allow` is a usage error. A failed write to standard output is ignored, as `build`'s messages are. | Spec §11 and §15. One line per file tells a person and a script what was made. Owner, 2026-11-27 (one line per file over silent). |
| D60 | What is charted, and what are the labels? | Every feature, in source order, including features nothing calls. Tests are not charted. `note` statements are not nodes. A node's label is its statement's source lines (the first line and any continuation lines), each with its indentation removed and each invisible character spelled `\u{XXXX}` (`lower::fail::spell_invisible`). | Spec §15: one chart per feature, and the labels are the source lines themselves. |
| D61 | What shapes and arrow labels? | A rounded **start** box whose label is the feature's line and its `needs` lines. A rounded **end** box labelled `end`, which receives the fall-off path and every `give`. A rectangle for each other statement, with double side bars when the statement calls a feature (`run`, or a `Call` in one of its own expressions). A **decision** (a box with pointed left and right ends) for `if`, `otherwise if`, `for each` and `repeat while`, labelled with that line. Arrows out of `if`, `otherwise if` and `repeat while` are labelled `yes` and `no`; out of `for each`, `next` and `done`. `otherwise` is not a node. `skip`, `stop` and `give` are rectangles whose one arrow goes to the loop's decision, past the loop, and to the end box. | Owner, 2026-11-27 (start and end boxes, so `give` has a target). |
| D62 | How is a chart laid out? | Top to bottom, **one node per row, in source order**, so no two nodes share a row. A block stays in one column. In an `if` chain, each body is in its condition's column, and the next condition is one column to the right of everything before it, on the row after the previous body: yes goes down, no goes right. An `if` with no `otherwise` gets an empty last column for its `no` path. The ends of all branches run down their own columns and join in the gap above the next node. | Owner, 2026-11-27 (yes down and no right; one row per node over compact columns). A horizontal arrow then never crosses a box, and the layout has invariants a test can check. |
| D63 | How do loops and jumps route? | Every arrow is made of horizontal and vertical segments. Horizontal segments run in the gap between two rows, on one track per target in that gap, or along a node's own row. Each loop has a **back lane** left of its span and a **done lane** right of it (As built, M7-R9: placed in the gaps between columns). The back arrow and every `skip` run up the back lane into the decision's left point. The `done` arrow and every `stop` run down the done lane to the gap above the node after the loop. The feature has one **give lane**, right of everything, which every `give` runs down to the end box (As built, M7 Task 3: right of the last done lane or column edge, not of everything). A node that nothing reaches (after a jump; the checker warns about it) is drawn with no incoming arrow. | Spec §15: every arrow goes forward in an enclosing block or back to an enclosing loop's condition, so lanes nest with the blocks. |
| D64 | How is the SVG written? | By `gloss` itself, with no crate (D3) and no drawing tool. Black lines on a white background, no colours. Monospace text at 15px, with each character assumed to be 9px wide (no font metrics). Every coordinate is a whole pixel, and the same program always gives the same bytes. The file has a `<title>` that names the source and the feature. | Spec §15's v0 limits: one fixed layout, no options. |
| D65 | How is M7 tested? | `tests/flowchart_cases/<case>.gloss` with the expected `<case>.<feature>.svg` files, a sweep floor `MIN_FLOWCHART_CASES`, and `GLOSS_BLESS=1` as in M6. Unit tests on the graph (nodes, labels, arrows, targets) and the layout. Invariant tests over every case and every example: no two boxes overlap, no segment passes through a box, two arrows share a segment only when they have the same target, every statement other than `note` is exactly one node, and every SVG is well-formed XML (a small checker in the tests). The controller renders the examples' charts and looks at them before the final review. | Golden SVG alone would pass a chart that is drawn wrong; the invariants say what "drawn right" means. |

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
| Run each test in-process and catch failures (unwinding, or a failure routine that returns) | Every failure routine ends the process today; making them return touches every call site, and one test's damage (heap, tables, globals) could leak into the next | Fork becomes unavailable on a target |
| Capture a child's standard error through a pipe (`Pipe` and `Dup2` primitives) | Two more primitives and more states to model in `irsim`, when the test backend can send failure text to a buffer by itself (D36) | The test backend and the real runtime drift apart |
| The traced executable writes the sentences itself, in IR | Every sentence form becomes runtime code checked under `irsim`, about twice the runtime work, and Explain would print it all (D42) | Trace must run without `gloss` installed |
| Events on a second descriptor (fd 3) beside the program's own output | Two pipes lose the order between a printed line and the events around it (D43) | n/a |
| A cap on the number of narrated events | Hides part of the run; no one has asked for it (D49) | Real programs make traces too long to use |
| Explain disassembles the built ELF | Needs a disassembler, and loses the line markers (D51) | n/a |
| Explain prints the IR | It is not the machine's instructions (spec §10, §12) | n/a |
| Peephole improvements so Explain matches the spec's first example | Optimisation, which v0 forbids, and a wider milestone (D51, D58) | v0.1 allows optimisation |
| Golden files for whole runtime routines | Every runtime change would rewrite every case (D57) | n/a |
| Compact columns: the bodies of an `if` chain side by side from the same row | A `skip`, `stop` or `give` in a right-hand branch then needs a router to avoid crossing other branches (D62) | Charts of real programs prove too tall |
| Emitting Graphviz DOT, or any drawing tool | Spec §15: the compiler writes the SVG itself | n/a |
| Charting test blocks | Spec §15 charts features | Owner asks |
| Merging straight-line statements into one box | Spec §15: statements are boxes | n/a |
| Real font metrics for text width | Needs font files or a crate (D3); a fixed monospace width is exact enough for monospace text (D64) | Wide characters (CJK) prove common in labels |

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
| `trace` | The narrator: JSON events to sentences (§4.9) | M5 |
| `explain` | The listing: features, lines, comments, runtime (§4.10) | M6 |
| `flowchart` | The flowchart: AST to a graph, a layout and SVG text (§4.11) | M7 |

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
- **Load.** Open with `RESOLVE_BENEATH | RESOLVE_NO_SYMLINKS`. A missing file prints `expenses.csv not found; starting with an empty table` to the error output and loads an empty table. `ELOOP` or `EXDEV` is failure 6 (symbolic link); any other error is failure 6 (cannot be read). Read the whole file into one growing block, check it is UTF-8, then parse RFC 4180: the header is matched to fields by name, quoted cells may hold doubled quotes, commas and line breaks, `\n` and `\r\n` both end a line, and one final empty line is ignored. One leading UTF-8 byte order mark (as Excel and Notepad write) is skipped before the header. Number cells use the runtime's `parse_number`; truth cells must be `yes` or `no`.
- **Save.** Open the table's folder with `O_RDONLY | O_DIRECTORY | O_CLOEXEC` and the same resolve flags (not `O_PATH`: a path-only descriptor cannot be synced, M3-R8); a missing folder is failure 7 (folder does not exist). A target that is a symbolic link is failure 7. Write `<name>.gloss-new` in that folder (`O_CREAT | O_TRUNC`; the mode is the target's permission bits, read with `fstat` on the probe descriptor, or 0644 when the table does not exist yet, M3-R26): the header in field order, then each row, numbers as `show` prints them, truths as `yes` and `no`, quotes only where needed, `\n` line endings. Then `fsync` the file, `renameat` it over the target and `fsync` the folder. Any write error is failure 7 (cannot be written). Output already shown is flushed before a failure, as in M2.
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
| 6 | A quote out of place: text after a closing quote, or a `"` inside a cell that does not start with one | `line N: failure 6: expenses.csv line 8: a quote is out of place` |
| 6 | Bytes that are not UTF-8 | `line N: failure 6: expenses.csv is not UTF-8 text (line 4)` |
| 6 | A symbolic link on the path | `line N: failure 6: expenses.csv is reached through a symbolic link` |
| 6 | Any other read error | `line N: failure 6: expenses.csv cannot be read` |
| 7 | The folder does not exist | `line N: failure 7: data/expenses.csv cannot be written (its folder data does not exist)` |
| 7 | Any other write error | `line N: failure 7: expenses.csv cannot be written` |
| 7 | A symbolic link on the path | `line N: failure 7: expenses.csv is reached through a symbolic link` |
| 8 | Saved before a load or clear | `line N: failure 8: expenses is saved here but was not loaded or cleared first` |

Cell text in a message follows failure 5's rules: at most 40 characters, invisible characters as `\u{XXXX}`, a `"` doubled. File and column names follow D24.

**Testing (D29).** Every message above has a run case with an exact `.err` and status 2. The unreadable-file and unwritable-folder cases skip visibly when run as root. Durability cannot be tested by pulling the power; a unit test on the generated IR checks the order `fsync`, `renameat`, `fsync` of the folder. `tests/cli.rs` covers: a first build answered yes and no, no terminal and no `--allow` (status 4, no executable), `--allow` writing `.allow`, a build covered by `.allow` (asks nothing, still prints the list), read only becoming read and replace, a dropped file shrinking `.allow`, and a broken `.allow` (status 3). Examples gain an expense tracker (`load`, `append`, `save`) and a read-only report.

**Reviews.** The security boundary gets its first review on the top tier: the approval logic and `.allow` parsing, the open and save paths, symbolic-link handling, safe writing, the new primitives in the encoder, and the CSV reader, which parses untrusted input.

### 4.8 Milestone 4: tests

**Scope.** `gloss test x.gloss` (D37) builds and runs every `test` block. M4a (D30): the CLI, the driver, `expect`, run-time failures inside tests, tables that start empty. M4b: `input`, `shown`, pretend files. A program with no tests prints `No tests in x.gloss` (status 0) and builds nothing.

**Pipeline.** check (tests included) → `lower_tests` (IR for the features, one function `t_<i>` per test in source order, the driver, and the reachable runtime with the test backend) → the same x86-64 backend and ELF writer as `build`. The entry `_start` of a test module is the driver's: `DefaultChildSignal; heap_init; stack_init; tables_init; test_main; flush; exit(status)`, where `test_main` gives 0 when every test passed and 1 otherwise. `--permissions` is not part of a test executable.

**The driver (D31, D32).** `test_main` is generated IR, one block per test:

```
flush            (the parent's output buffer is empty, so no child repeats it)
Fork
child (0):  set the title in the test state; call t_<i>; call test_pass   (never returns)
parent:     Wait(pid); 0 -> passed + 1;  1 -> failed + 1;
            anything else -> report_lost(title, status); failed + 1
```

After the last test the parent prints a blank line and `N passed, M failed`, with `N` and `M` written through `number_text`. The parent never runs test code, so every child starts from the same untouched heap, tables and globals.

**Reporting.** `test_pass` and `test_fail` write straight to descriptor 1 with `Prim::Write` through their own routine (`report_bytes`), not through `write_all` or the output buffer, because those are the test backend's (below). A block is written in a few whole writes after the test ended. `test_fail` prints `fail  <title>`, then each line of the failure text indented 6 spaces, then, if the test printed anything, `      shown:` and each shown line indented 8 spaces. It exits 1.

**The test state.** One new global, `Global::TestState` (2 KiB), holds, as 8-byte words: the title (a text), the `shown` bytes (pointer, length, room), the `input` answers (a list of text, and the index of the next one), a `failing` flag, and the failure text (a length word and a fixed byte area of 1,792 bytes; extra bytes are dropped). The failure text is a fixed area because failure routines must not allocate: out of memory (failure 10) is itself a failure that has to be reported (see `err_number`'s test in M2).

**The test backend (D30, D36).** `add_reachable` takes a `Backend { Real, Test }`. With `Test`, these routines are replaced, by name, with test versions; nothing else in the runtime changes:

| Routine | Real | Test |
|---|---|---|
| `write_all(fd, addr, len)` | writes to the descriptor | fd 1: appends to the `shown` bytes (dropped when `failing` is set and they are full, since growing them could fail again); fd 2: appends to the failure text |
| `fail_begin(line, code)` | flush, then `line N: failure C: ` to fd 2 | sets `failing`, flushes (so pending `show` output reaches `shown`), then writes the same text through `write_all` |
| `fail_end()` | newline, exit 2 | calls `test_fail` (no newline) |
| `ask(question, line)` | prompt, read a line | takes the next answer, or fails the test (D35); nothing is printed |
| `table_read`'s file half | `file_bytes` | the table's pretend file; a missing file gives an empty table and no notice |
| `table_save` | `Open`, `Rename`, `Sync` | `csv_text` into the table's pretend file |

For this to work, `err_int` must write through `write_all` (today it issues the `Write` primitive itself, `src/runtime/output.rs:173`), and `table_read` is split in two: `table_read` (bytes from `file_bytes`) and `table_rows` (the parser). In test mode, `load` is lowered to `pretend_read` (which hands `table_rows` the pretend file's text, or 0 for missing) and `save` to `pretend_save` (which stores the result of `csv_text`); both take the table's index. A table's entry in the tables block is three words in a test build: list, loaded flag (**1**: tests start with cleared tables), pretend file (0 for missing).

**Test statements and expressions.**
- `expect cond`: `condition(cond, fail)`; the failure block calls `test_expect(line, source)`, which writes `line N: expect <source>` into the failure text and calls `test_fail`. `<source>` is the condition as `gloss format` writes it (`ast::print::expr`), invisible characters as `\u{XXXX}`, as in every failure message (M2-R30).
- `input a, b, c`: each item must be text (a checker error otherwise, plan ruling P3); the answers are appended to the queue.
- `shown`: a fresh `list of text` made by `shown_list(line)`: flush, then split the shown bytes at each `\n` (a line is every byte up to a `\n`).
- `run start` and every other statement lower as in a feature; a test's variables get slots from `TestInfo`.
- Failures 6, 7 and 8 cannot happen in a test: every table counts as cleared, and the pretend file only ever holds text the CSV writer made. They live in shared code and are reachable only from a unit test that hands the parser bad bytes.

**Messages (binding).**
- `the test gave N answer(s) but line L asked a <ordinal> question`: `N` is the number of answers queued so far, `answer` when it is 1 and `answers` otherwise (`0 answers`); the ordinal is the word for the number of the question asked (`first` to `tenth`), and from the eleventh on `a <ordinal> question` becomes `question number K`. Examples: `the test gave 0 answers but line 7 asked a first question`; `the test gave 1 answer but line 7 asked a second question`; `the test gave 2 answers but line 31 asked a third question`; `the test gave 10 answers but line 4 asked question number 11`.
- `ended without a result (status N)`: `N` is `Wait`'s result (a signal shows as `256 + signal`).
- `No tests in x.gloss`, `gloss test runs tests on Linux only`.

**The CLI (D37, D38).** `run` handles `test` before the milestone table. Steps: read and check (the same diagnostics, sorted, and the `.gloss` suffix rule); on an error exit 3; if the program has no test print `No tests in <name>` and exit 0; if the host is not x86-64 Linux print `gloss test runs tests on Linux only` to standard error and exit 3 (after the compile errors, so a Mac user still sees them); build the module through `Checked::test_executable()`; create a private folder (`DirBuilder`, mode 0700, a fresh name under `std::env::temp_dir()`), write the executable there (mode 0700), run it with standard input null and standard output and error piped, each streamed to the terminal as bytes arrive (standard error on its own thread, standard output flushed after every chunk, so a hung test still shows every finished test's result), and delete the folder on every path, a panic included (a drop guard). The test run's exit status 0 or 1 is `gloss test`'s; anything else prints `the test run ended without finishing (status N)` to standard error and exits 1. `--allow` is a usage error for `test`.

The library function the harness calls is `build_test_executable(text) -> Result<Vec<u8>, Vec<Diagnostic>>`, the test-mode twin of `build_executable`; the CLI uses `Checked::test_executable`.

**Testing (D39).** `tests/test_cases/<name>.gloss` with `<name>.out`, optional `.err` (default empty) and `.status` (default 0); a sweep like D13's (`MIN_TEST_CASES`) shared by `tests/test_cases.rs` (real executables) and `src/irsim/test_cases.rs` (`irsim`). `tests/cli.rs` covers: no tests, a compile error (3), the Linux-only refusal (the deciding function is unit tested, since a Linux host cannot show it), and, on Linux, all pass and one fails. A lowering test asserts that a test module's IR contains no `Open`, `Rename`, `Sync`, `Mode` or `SetMode` (D36). Spec §13's complete program is `tests/test_cases/expense_tracker.gloss`; the examples gain tests, and `tests/examples.rs` also runs `gloss test` on each.

**Reviews.** `Fork` and `Wait` (target numbers, status decoding, the `irsim` clone) and the pretend-file backend (the swap, and the claim that no real file primitive remains) get their first review on the top tier.

### 4.9 Milestone 5: trace

**Scope.** `gloss trace x.gloss` (D48). M5a (D41): `lower_trace` and the trace backend, so the traced executable writes the events of spec §10. M5b: the narrator (`src/trace/`), the `gloss trace` command, the spec's sentence forms. The step-through viewer is v0.1 (spec §15) and not part of v0.

**Pipeline.** check → `lower_trace` (`Mode::Trace`: `start` and the features with event code, no tests, no `permissions_text`) → the reachable runtime with `Backend::Trace` → the same x86-64 backend and ELF writer. The trace `_start` is generated: `heap_init; stack_init; tables_init; call f_start; flush; exit 0` (start has no `enter` or `leave` event; as built, M5-R10). It does not call `args_check`: the traced executable is only ever run by `gloss trace`, with no arguments.

**The trace state.** One new global, `Global::TraceState` (32 bytes), laid out only when used (like `TestState`), so real and test builds keep their layout. Words: `ESCAPING` (text entering the output buffer is JSON-escaped), `FAILING` (writes to fd 2 are the failure message), `OPEN` (a `show` event is open: its text is being written) and `CARRY` (byte 24: an incomplete UTF-8 tail of at most 3 bytes held back between writes of the failure message; as built, M5-R4).

**The trace backend.** `out_bytes` is split: its body becomes `buf_bytes(addr, len)`, and the real `out_bytes` calls it. Real builds' code changes in three places in M5, all splits that leave behaviour as it was (as built, M5 final review): `out_bytes` into `buf_bytes`, `write_all` into `write_loop`, and `ask` into `ask_read`. With `Trace`, these routines are replaced by name:

| Routine | Real | Trace |
|---|---|---|
| `out_bytes(addr, len)` | `buf_bytes` | `ESCAPING` 0: `buf_bytes`; 1: `json_bytes` (escaped into the buffer through `buf_bytes`) |
| `write_all(fd, addr, len)` | writes | fd 2 while `FAILING`: `out_bytes` (so escaped into the `fail` event); fd 2 otherwise: flush, then the real write; fd 1: the real write |
| `fail_begin(line, code)` | flush, `line L: failure C: ` to fd 2 | if `OPEN`: close the `show` event (`"}` and a newline); then `{"line":L,"event":"fail","failure":C,"message":"`; `FAILING` and `ESCAPING` set |
| `fail_end()` | newline, exit 2 | `"}` and a newline; flush; exit 2 |
| `ask(question, line)` | shows the question, flushes, `ask_read(line)` | a `show` event of the question, flush, `ask_read(line)` |

The real `ask` is split the same way: its read loop becomes `ask_read(line)`.

`json_bytes` escapes `"` and `\` with a backslash, `\n`, `\r` and `\t` by their short forms, every other byte below 0x20 and 0x7F as `\u00XX` (lower-case hex), and copies valid UTF-8 sequences unchanged. Each byte that does not start a valid UTF-8 sequence becomes U+FFFD itself (the bytes EF BF BD), not a six-character escape (as built, M5-R6). Text from files is valid UTF-8 already (failure 6 otherwise); text typed at an `ask` may not be.

`json_value(value, desc)` writes a typed JSON value (spec §10), walking the same descriptors as `show_value`: `{"number":"<show form>"}`, `{"text":"<escaped>"}`, `{"truth":"yes"}`, `{"list":[v, …]}`, `{"record":"Expense","fields":{"name":v, …}}` with fields in declaration order. No spaces are written anywhere in an event.

**Events (binding).** Keys appear in exactly this order. `L` is the event's line; `V` a typed value.

| Event | Shape |
|---|---|
| set | `{"line":L,"event":"set","name":"total","from":V,"to":V}`; a field adds `"field":"amount"` after `name`; an item adds `"item":N` after `name` (and before `field` for a field of an item); a variable's first set has no `from`; `append` is `{"line":L,"event":"set","name":"prices","item":N,"to":V}` with `N` the new count |
| check | `{"line":L,"event":"check","condition":"<as gloss format writes it>","values":{"price":V,…},"result":"yes"}` |
| loop | `{"line":L,"event":"loop","name":"price","turn":N,"value":V}` |
| enter | `{"line":L,"event":"enter","feature":"gcd","inputs":{"a":V,…}}` (`"inputs":{}` when it has none) |
| leave | `{"line":L,"event":"leave","feature":"gcd","result":V}`; no `result` when the feature gives nothing |
| show | `{"line":L,"event":"show","text":"<the line, without its newline>"}` |
| ask | `{"line":L,"event":"ask","name":"item","question":"…","answer":"…"}` |
| file | `{"line":L,"event":"file","table":"expenses","file":"expenses.csv","action":"load","rows":N}`; `action` is `load`, `save` or `clear` |
| fail | `{"line":L,"event":"fail","failure":C,"message":"<the text after line L: failure C: >"}` |

Each event ends with one newline. `line`, `item`, `turn`, `rows` and `failure` are JSON integers.

**Where the events are written (lowering).**
- No event is open while user expressions run: `show` evaluates all its items into hidden slots first, then writes the event from the slots, so a failure part-way through a `show` writes no `show` event (as built, M5-R5). Every other event is written from values already computed into hidden slots, so events from calls inside an expression always come before the event of the statement that contains them.
- **set** (D44): the new value is computed (and copied, as today), the target's position checks done (failures 2 and 3), then the old value is read, the event written, the value stored. For `add`, `subtract`, `multiply` and `divide` the old value is saved before the arithmetic, so a failure 4 writes no event. Each variable made in a function has a hidden "made" flag slot in a trace build (frames start zeroed); a set with the flag 0 writes no `from` and sets it. `ask … into` makes its variable the same way but writes an `ask` event instead of a `set` event.
- **check** (D45): in `condition()`, after the condition's value is in `a` and before the branch. `values` holds each distinct variable, loop variable, input, and field read rooted at one of those (`amount of expense`, but not `amount of item 2 of expenses`), in order of first appearance, keyed by its printed form. As built (M5-R7, M5-R8): a name used only as an item position (`i` in `item i of xs`) is listed, and a list or table name is never listed.
- **loop**: after the loop variable is stored, before the body. A counting loop keeps a hidden turn counter; a list loop uses its index + 1.
- **enter / leave** (D46): at the call site, after the inputs are evaluated and after the call's `StackCheck` (so failure 10 writes no `enter`), and after the call returns. The line is the call's line.
- **file**: after `load`, `save` or `clear` succeeds; `rows` is the table's count afterwards.

**The narrator (`src/trace/`).** `json.rs` (a small JSON reader, std only), `event.rs` (typed events), `value.rs` (values as `show` writes them, shortened: D47), `narrate.rs` (sentences, indentation). `Narrator::new(&Program)` maps each line to its condition's syntax tree. For each event line it writes the line to `x.trace` and prints:

| Event | Printed |
|---|---|
| show | the text and a newline, never indented |
| fail | `line L: failure C: <message>` and a newline on standard error |
| set | `line L: total becomes 150`; first set: `line L: total is 0`; field: `amount of expense becomes 12`; item: `item 2 of prices becomes 5`; field of item: `amount of item 2 of expenses becomes 12`; append: `line L: 5 is appended to prices (now 4 items)` (`1 item`) |
| check | `if`/`otherwise if`: `line L: <c>, so the block runs` or `line L: <c> is no, so the block is skipped`; `repeat while`: `…, so the loop runs` or `… is no, so the loop ends`. `<c>` is the condition as written with each name in `values` replaced by its value; a condition that is a truth and not a comparison, `not`, `and`, `or` or number check (a name, a field read, an item, a call, `first of`) is written `<c> is yes` / `<c> is no` (as built, M5 final review) |
| loop | `line L: price is 150` |
| enter | `line L: calling gcd with a = 48, b = 36`, or `line L: calling report` |
| leave | `line L: gcd gives 12`, or `line L: report ends` |
| ask | `line L: the answer "coffee" goes into item` |
| file | `line L: loaded expenses from expenses.csv (12 rows)`, `line L: saved expenses to expenses.csv (12 rows)`, `line L: cleared expenses` (`1 row`) |

Every sentence is indented 2 spaces per open call below `start`; `start` has no `enter` or `leave` (as built, M5-R10). Values: numbers in their show form, truths `yes`/`no`, text in quotes with `"` doubled, lists and records as `show` writes them; a list shows its first 3 items and then `… (N items)`, text over 60 characters is cut to 60 and ends with `…` inside the quotes, as `…"` (D47; as built, M5-R13).

**The CLI (D48).** `trace` shares `build`'s first half (diagnostics, the `.suffix` rule, `check_flags`, the permission list and `approve`) through one helper. Then: the host check (`gloss trace runs programs on Linux only`, status 3); `Checked::trace_executable()`; create `x.trace` (failure: `x.trace cannot be written: <reason>`, status 3), written through a `BufWriter`, one flushed write per line, and flushed again when the run ends, before any closing message; write the executable into a `PrivateDir`; run it with the current folder as working directory, standard input inherited, standard output piped (read in whole lines), standard error streamed through. Status: 0 → 0; 2 after a `fail` event → 2; anything else, or a last line with no newline, → `the traced run ended without finishing (status N)` on standard error and 2. As built (M5-R15): an event line the narrator cannot read (a line that is not UTF-8 included) stops the run with `the traced run wrote an event gloss cannot read (event N: why)` and status 3.

**Testing (D50).** `tests/trace_cases/<name>.gloss` with `<name>.trace` (the exact event lines) and `<name>.out` (the narration), optional `<name>.in`, `<name>.status` and a `<name>.files/` seed folder; a sweep with `MIN_TRACE_CASES`. Three harnesses: `tests/trace_cases.rs` (real executable: stdout equals `.trace`), `src/irsim/trace_cases.rs` (`irsim` agrees), `tests/trace_narration.rs` (the narrator turns `.trace` into `.out`; any host). A test asserts that a real build and a test build contain no `trace_state` and no trace routine.

**Reviews.** The trace backend's swaps (`out_bytes`, `write_all`, `fail_begin`, `fail_end`, `ask`), the set-event ordering, and enter/leave at the call site get their first review on the top tier.

### 4.10 Milestone 6: explain

**Scope.** `gloss explain x.gloss` (D56) prints the real build's instructions beside the source (D51 to D55). Nothing new runs; the work is a printer over what `build` already makes, plus the names it needs.

**Pipeline.** check → `lower_explained` (the real lowering, which also returns `Notes`) → the reachable runtime with `Backend::Real` → `assemble` → the listing. `build` goes check → `lower` → runtime → `assemble` → `encode` → ELF, so both read one `Assembly`.

**`assemble` (D51).** `program::executable` is split: `assemble(m: &Module) -> Result<Assembly, String>` lowers every function in module order and appends `arith::helpers()`; `Assembly` keeps, for each function or helper, its name and its `Vec<X>`. `executable` is `assemble`, then the existing encode and ELF steps over the concatenated instructions. Nothing about the bytes changes: the same instructions are encoded in the same order, and every test that runs an executable covers the split.

**The listing types (`src/target/listing.rs`, target-neutral, D18).** `Target::listing(&Module) -> Result<Vec<Routine>, String>`. A `Routine` is `{ name: String, items: Vec<Item> }`; an `Item` is `Line(u32)` (a `Line` marker), `Label { text: String, local: Option<u32> }` (`local` is the IR label number for a function's own labels, printed `.L<n>`), or `Inst { text: String, refs: Vec<Ref> }`. A `Ref` is `Slot(u32)`, `Input(u32)`, `Text(u32)`, `Jump(u32)` (to the function's own label `n`) or `System(&'static str)` (the call a `syscall` makes). The x86-64 target builds these from its `Assembly`; `src/explain/` never sees an `X`.

**Notes.** IR `Function` stays as it is (168 places build one). `lower_explained(program, types, src) -> Result<(Module, Notes), Diagnostic>` runs `lower_program` in `Mode::Real` and keeps, per feature function: the name of each named slot, the inputs in order, and, per failure label, its code, line and message text (the `Fail`'s parts, with `…` for a value, shown text or item count read at run time). `lower` calls it and drops the notes. Runtime routines have no notes.

**The listing (`src/explain/`).** `listing(src, program, types, notes, assembly) -> String`:
- First line `Explain x.gloss for x86-64 Linux`, a blank line.
- Each feature function (`f_<name>`, in module order, which is source order): `feature NAME (line N)`, its inputs, variables and loop variables (D54; types as the checker describes them, the slot as `[rbp-8k]`, an input as `[rbp+16+8(p-k)]`), a blank line, then its instructions.
- Instructions are 4 spaces in; a label (`X::Label`) is 2 spaces in and ends with `:`. `X::Comment("line N")` becomes a header (D52); a `Mark` of a failure label (from the notes) becomes `failure C, line N`. Local labels `L<f>_<n>` print as `.L<n>` in both labels and jumps.
- Comments (D53) come from each instruction's refs: `Slot`/`Input` with a note (feature functions only), `Text(k)` (the module's text `k` in quotes, each `"` doubled, each character `chars::unshowable` spelled `\u{XXXX}`, cut after 30 characters with `…` inside the quotes), `Jump(n)` to a failure label in the notes, and `System(name)`. Several refs join with `, `. A comment starts at column 36 (counting from 0), or one space after the instruction when it is longer.
- (As built, M6-R9/R10.) Code that belongs to the feature as a whole is listed under the feature's own line: the entry code before the first `Line`, and, as `(continued)`, the end guard and the implicit return of a feature that gives nothing. `otherwise` has no header of its own. A comment is cut after 30 characters of the decoded text (M6-R8).
- (As built, M6 final review.) The printer puts labels that are directly followed by a `Line` after that line's header (when the line is already current, no header is printed and the labels simply follow), except a failure label, which stays right after its `failure` header, so a `repeat while`'s top label sits under the loop's line and an `if`'s end label under the next statement's. Labels encode nothing, so the order is equivalent. Every printed source line goes through `spell_invisible`.
- Then a blank line, `runtime`, and every other function and helper in assembly order as `routine NAME` with its instructions (D55).

**The CLI (D56).** `explain` takes exactly one `.gloss` file, as `format` does. It shares `build`'s diagnostics step; it never calls `approve`. It prints through one locked, buffered standard output and maps a write error to `cannot write the explanation` (status 3), so a closed pipe (`gloss explain x.gloss | head`) does not panic.

**Testing (D57).** `tests/explain_cases/` with a sweep (`MIN_EXPLAIN_CASES`), run on any host. Unit tests in `src/explain/` for headers, `(continued)`, failure headers, label renaming and each comment rule. Invariants in `src/target/x86_64/` (assembly equals what is encoded) and, as built, in `src/explain/` unit tests (the printed instruction lines of every example and case equal the assembled instruction texts; every routine is listed in layout order), because `assemble` and the instruction text are crate-private. `tests/cli.rs` covers statuses and messages.

**Reviews.** The `assemble` split (it sits on the path of every executable) gets its first review on the top tier; the rest on the default tier.

### 4.11 Milestone 7: flowchart

**Scope.** `gloss flowchart x.gloss` (D59) writes one SVG chart per feature (D60 to D64). Nothing runs; the work is a reader of the checked AST plus a layout and an SVG writer. No change to lowering or the target.

**Pipeline.** check → for each `FeatureDecl`: `graph` (nodes and arrows from the AST and the source lines) → `layout` (rows, columns, lanes, routes) → `svg` (text). `Checked::flowcharts(&self, stem: &str) -> Vec<(String, String)>` returns `(file name, svg)` pairs in source order; the CLI writes them.

**The graph (`src/flowchart/graph.rs`).** `Graph { nodes: Vec<Node>, arrows: Vec<Arrow>, regions: Vec<Region> }` (As built, M7 Task 3: `regions` lists each loop and each `if` chain as ranges of node indices, for placing columns). A `Node` is `{ shape: Shape, label: Vec<String>, line: u32 }` with `Shape::{Start, End, Step, Call, Decision}`; nodes are in source order, start first and end last, so a node's index is its row. An `Arrow` is `{ from: usize, to: usize, kind: ArrowKind }` with `ArrowKind::{Next, Yes, No, Each, Done, Skip, Stop, Give}`; the kind says how the arrow leaves its node and gives its label (`yes`, `no`, `next` for `Each`, `done`), and an arrow to an earlier node, or to its own, is a back arrow (`is_back`, `to <= from`; As built, M7-P2: there is no `Back` kind, a back arrow keeps the kind it leaves with). The builder walks each block keeping its open exits (the arrows still looking for a target) and the innermost loop; a jump closes its exit with its own arrow; a block's open exits go to the next node; a loop body's open exits go back to the loop's decision.

**The layout (`src/flowchart/layout.rs`).** Columns come from the block structure (D62); lanes from the loops and the feature (D63). Sizes: a label line is 20px high and 9px per character wide; a box adds 12px each side and 8px top and bottom; a decision adds half its height at each pointed end. Rows are stacked with a gap between them; a gap is 24px plus 10px per track beyond the first, where a track is a distinct target among the arrows that have a horizontal segment in that gap (As built, M7-R17: an arrow that only drops straight through adds none). Columns are placed left to right, each as wide as its widest node. Lanes sit in the gaps between columns, not at the chart's edges (As built, M7-R9): the gap before column k holds the done lanes of loops whose span ends at column k-1 (inner first, outer further right), then the back lanes of loops whose span starts at column k (outer first, inner nearest); lanes are 16px apart and 16px from the column edges, and the give lane is 16px right of the last done lane or column edge. The first gap right of every column is 40px (`LABEL_ROOM`) instead of 16, so a `no` or `done` label clears the turn of its own arrow (As built, M7-R20). Each column has one canonical centre, and every vertical arrow runs on it; a box is centred on it to within half a pixel (As built, M7-R11). `Layout { width, height, boxes, column, centres, loops, give_lane, gaps, no_cols, routes }`, with one route (a polyline, start point first) per arrow; `gaps` has one entry per row and `gaps[0]` is unused (As built, M7-R12).

**The SVG (`src/flowchart/svg.rs`).** `<svg xmlns="http://www.w3.org/2000/svg" width=W height=H viewBox="0 0 W H">`, a `<title>`, a white `<rect>` background, one `<marker>` arrowhead, then each node as `<rect>` (rounded with `rx` for start and end; two extra `<line>`s for a call) or `<polygon>` (decision) with one `<text>` per label line, then each arrow as a `<polyline>` with the marker and, when labelled, a `<text>` beside its first segment. Text is escaped (`&`, `<`, `>`, `"`). Elements are written in node order, then arrow order, one per line.

**The CLI (D59).** `flowchart` takes exactly one `.gloss` file, as `explain` does. It shares `build`'s diagnostics step and never calls `approve`. Files are named from the source's stem and written beside it.

**Testing (D65).** Unit tests in `src/flowchart/` for each node kind, label rule, arrow kind and lane (As built, M7-R26: also one through `Checked`, because tests built on `Source::new` missed a bug). `tests/flowchart_cases/` with a sweep, run on any host. The layout invariants are checked in a unit test over every case and every example. `tests/cli.rs` covers statuses, messages and file names.

**Reviews.** All tasks are reviewed on the default tier. The layout and routing tasks (the core of the milestone) get their first review on the top tier.

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
- **M3 and M4** (superseded in M3: see As built — Milestone 3): tables (refused, D22), `load`, `save`, `clear`, `expect` and `input`. Run-time failures 6–8 are worded in M3. The permission list must show file names unambiguously (carried from M1).
- **Memory:** nothing is freed. A run under Rosetta needs a `ulimit -v` above about 800 MB, so the out-of-memory case uses 1 GiB (M2-R35), unverified on native x86-64.
- **Stack:** `irsim` models a 1 MiB stack (smaller than the real 8 MiB default, to keep `too_deep` cheap), so the depth at which failure 10 triggers differs from a real run; the message is the same. The stack limit is capped at 1 GiB.
- **Mac runs** (superseded in M3: now QEMU, see As built — Milestone 3): every run under `orb` is Rosetta. The `tests/scaling.rs` reverse-chain test once took 10.5 s under heavy compile load (bound 10 s), and the difftest once failed in a cold run that could not be reproduced; both are WATCH items. Once, a runaway `dhcpcd` inside the machine kept OrbStack at 100% CPU and slowed the run tests about 20 times; if run tests crawl, check the machine's processes first.
- **Failure messages** quote the expression as `gloss format` writes it (M2-R23).
- **Reviews:** the encoder, ELF writer, runtime memory, input and descriptor code each had a first review on the top tier.

The milestone ledger (every ruling, review record and carry) is kept in `.superpowers/sdd/2026-11-23-compiler-m2-codegen/progress.md`.

### Milestone 3 (tables and files), finished 2026-11-24, `main` @ b6dcbfd plus this documentation

**What shipped:**
- **Approval at build time.** The permission list (D24 names, D27 only for programs with tables), the `<source>.allow` file, `--allow <file>`, the yes/no question, exit status 4 (D25) and `--permissions` on every executable.
- **Tables.** A table is a list in its own global slot with a loaded flag; every M2 list operation works on it. `clear`, `load` and `save` build and run.
- **Files at run time.** New primitives `openat2`, `close`, `fsync`, `renameat` and the start-up arguments (D19). `load` parses RFC 4180 CSV from untrusted input; `save` writes `<name>.gloss-new`, syncs it, renames it over the target and syncs the folder. Failures 6–8 are worded as in §4.7.
- **Tests and examples.** Run cases can carry files and a folder to compare afterwards (D29); `irsim` has an in-memory file system. `examples/` gained `expenses` (`load`, `append`, `save`) and `report` (read only).
- **Language spec.** `spec/v0.md` §8, §10 and §11 now describe what the compiler does, and `tests/spec_examples.rs` checks its tables programs and failure 6–8 messages against this document both ways.

**Pull requests** (all rebase-merged, one at a time; design #274, plan #275).

| Task | What | PRs |
|---|---|---|
| 1 | Permission list and D24 names | #276 |
| 2 | `.allow` file | #277–#278 |
| 3a, 3b | `--allow` arguments; approval (the question, status 4) | #279–#280, #281–#284 |
| 4 | `--permissions` and `argv` in `_start` | #285–#287 |
| 5a, 5b | File primitives in IR and `irsim`; Linux system calls | #288–#294, #295–#300 |
| CI | Clippy drift fix | #301 |
| 6 | File cases in the run harness | #302–#307 |
| 7 | Tables in memory, `clear`, D22 lifted | #308–#311 |
| 8 | CSV writer; `save` | #312–#329 |
| CI | Clippy and nesting-stack fixes | #330–#331 |
| 9 | CSV reader; `load` | #332–#366 |
| 10 | Run cases for every failure 6–8 message | #367–#384 |
| 11 | Language spec, examples | #385–#388 |

**Deviations from the decisions above** (rulings are numbered M3-R1 … M3-R24 in the milestone ledger):
- **D22:** lifted in task 7a, not in the last task. D23 was amended to say so (plan ruling P1).
- **D27:** accepted deviation from the language spec §8: builds of programs with no tables print `Built x` and no list.
- **D14 (200-line PRs):** tasks over the cap were split into sequential PRs, never trimmed (M3-R5, M3-R7, M3-R19). Task 9 is 35 PRs.
- **D2 and D17:** the machine runs under QEMU (`orb config set rosetta false`), not Rosetta, because Rosetta has no `openat2` (owner's decision, 2026-11-24, during task 5b). Memory-limit runs skip visibly under QEMU (M3-R13).
- **D26:** failure 6 gained a row, `a quote is out of place` (M3-R22).
- **D29:** the case `table_save_link` was added.
- **§4.7:** `save` opens the folder with `O_RDONLY | O_DIRECTORY`, not `O_PATH` (M3-R8). The wording is fixed above.
- **Plan:** `table_save` takes 8 arguments, not the plan's 7; the extra one is the folder name as shown in messages.
- **Plan rulings:** P2: a `save` in a test block does not make a table read and replace. P3: `--allow` for a file the program does not declare is a compile error, status 3, so a typo cannot approve nothing.

**Rulings** (final state; later rulings are noted against the ones they supersede):

*Approval and names*
- M3-R1: D24 escapes are unpadded hex (`\u{E9}`), as the approved example shows; the plan's four-digit rule was wrong.
- M3-R2: a `"` in a table file name is refused, so a quoted entry in the list is always the D24 form and a plain name cannot imitate it.
- M3-R3: the build writes `.allow` before lowering. `.allow` records the approval; the executable carries its own list. Cost: a stale executable outlives a narrower `.allow` until rebuilt.

*Test and build plumbing*
- M3-R4: the task 4 table test lowered through a test-only entry that skipped the D22 refusal; task 7a removed it.
- M3-R5: task 5a was split into smaller branches up front, to stay under the cap.
- M3-R6: the x86-64 backend panicked on the new file primitives until task 5b; no program could reach them.
- M3-R7: task 5a became four, then seven branches; tests were not trimmed to fit.
- M3-R13: QEMU ignores `ulimit -v`, so memory-limit runs probe once (`dd` of 200 MB under 64 MB) and skip visibly when the limit is not enforced. CI still runs them.
- M3-R14: the `permissions_flag` run case used a program without tables, because tables could not build before 7a; 7a added the table case.
- M3-R15: the cases that need a program able to write or read files (`.after`, the working folder proof, `.allow`) landed with the tasks that made that possible.
- M3-R19: task 8's review fixes went on top of the 14-branch stack as new branches, not into it.
- M3-R21: `tests/nesting.rs` runs on an 8 MiB thread, the size of the binary's main stack, because rustc 1.100 debug builds overflow 2 MiB at 99-deep parentheses.

*`irsim` and the system calls*
- M3-R8: the folder is opened `O_RDONLY | O_DIRECTORY | O_CLOEXEC`, because `fsync` on an `O_PATH` descriptor fails with `EBADF`. `Probe` stays `O_PATH`.
- M3-R9, superseded in part by M3-R10: `irsim` rename mirrors `renameat` on a folder descriptor (single-component names, a link at the target is replaced, a link at the source is moved).
- M3-R10: a `Probe` descriptor of a folder is a valid directory descriptor for open and rename, as on Linux; a read or create descriptor gives `ENOTDIR`.
- M3-R11: the remaining `irsim`/Linux fit fixes went in a seventh branch, not folded into branches at 187–200 lines.
- M3-R12: fix now the ENOSYS skip (I1), the missing real-folder `Open` difftest (I2), `irsim` and Linux edge disagreements and missing lowering tests; M5 (host symbolic links) was settled by running under QEMU; carry the single-name check, FIFO and hard-link concerns into tasks 8 and 9.
- M3-R20: the save order is tested by an `irsim` event log of `sync` and `rename`, which fails if the folder sync moves before the rename.

*Tables and lowering*
- M3-R16: after table `is` compared pointers (`type_desc` mapped `Ty::Table` to a number), every `match` on `Ty` in lowering was audited; only `type_desc` was wrong.

*Files*
- M3-R17: (a) `table_save` asserts that folder, name and temporary name are single plain names; (b) the `table_save_new` case proves the working folder and `.after`; (c) the hard-link and FIFO risks below are accepted; (d) the design's `O_PATH` text was superseded by M3-R8.
- M3-R18: a record of one text field with empty text is saved as `""`, so no row is a bare empty line (the reader ignores one final empty line). The reader does not trim cells.
- M3-R22: `load` follows RFC 4180 for quotes. Text after a closing quote, or a quote inside an unquoted cell, is failure 6. A lone `\r` is kept as data (save quotes it, so it round-trips). A blank middle line is a one-cell row. Silent repair of damaged input would hide the damage.
- M3-R23: `load` skips one leading UTF-8 byte order mark, as Excel and Notepad write one. `save` does not write it back.
- M3-R24: a FIFO at the table path blocks `load` forever. Accepted (see below).

*Final-review fixes (the whole-milestone review, 34eb0bb)*
- M3-R25: a table that reaches code through `give` is a copy. Root cause: `walked_list` treated every root `Name` as an owned variable, but a zero-input feature call is also a `Name`, so `for each r in rows` (a feature that gives `rs`) walked the table's own list, skipped the walk guard (a `load` in the loop was lost, an `append` ran to failure 10) and let `set n of r` change the table. Now only a variable, a loop variable, an input or a table named in the loop is walked in place; a feature call is copied. Checked alias paths: `for each` over a zero-input and a with-input feature, `set x to rows` then a change, `rows` passed to a feature (inputs cannot be changed), `item 1 of rows`, a parenthesised call. Run cases: `give_table_walk`, `give_table_load`, `give_table_append`.
- M3-R26: `save` keeps an existing table's permission bits. A new IR primitive `Mode` (`fstat`, bits 0o777) reads them from the probe descriptor, and `Open(Create)` takes the permissions as a third argument (the umask still applies); a new table is 0644. A new primitive `SetMode` (`fchmod`) then sets the bits on the open temporary file exactly, so a stale `.gloss-new` left by a crash cannot change the table's mode (case `table_save_stale_temp_mode`). `irsim` models modes; a difftest compares both. The `.after_mode` file of a run case asserts the bits after the run (cases `table_save_keeps_600`, `table_save_keeps_640`, `table_save_new`, `table_save_read_only_file`). The owner and group of the new file are the program's, and a hard link to the old file keeps the old content.
- M3-R27: failure 7 keeps naming the table, even when the `.gloss-new` temporary is what failed.
- M3-R28: an empty (0-byte) table file loads as an empty table, with no notice; stated in spec §8.
- M2 (final review): a `load` in a `test` block no longer satisfies the "saved but never loaded" check; only features count.
- M3, M4, M6 (final review), tests only: `table_save` leaves no descriptor open on success (on failure 7 only the folder and the file being written, as the program exits); a leftover `.gloss-new` is overwritten (`table_save_stale_temp`); `irsim` has short and zero-byte file writes (`Faults::short_file_write`) and the existing failing write stands for ENOSPC; run cases for a save through a symbolic-link folder, a table path that is a folder on load (failure 6) and on save (failure 7, the temporary stays) and a `load` inside `for each i from 1 to count of rs` (the end is worked out once).
- EINTR is untested: `file_write` does not retry a negative result, but the runtime installs no signal handler, so a write to a regular file is not interrupted; a failed write would be failure 7 (cannot be written).

**Tests:** 670 pass and 1 is ignored (the stress test) after the final-review fixes: 547 unit tests and 123 integration tests in 24 files (before them: 664, 541 and 123). They include 152 run cases (`MIN_CASES`, 142 before the fixes), each run as a real executable and under `irsim`, 11 examples (`MIN_EXAMPLES`), and run cases for every failure 6–8 message with exact `.err` and status 2. The 4 permission cases ran as a non-root user, and CI run 45405019166 (`ubuntu-latest`, non-root) printed `permission cases: 4 ran, 0 skipped`. Under QEMU the memory-limit runs skip. The objdump check is opt-in. CI found two clippy lints that the local toolchain (1.87) lacked; the local toolchain is now 1.100 (stable).

**Known limits and accepted risks:**
- **Save and a same-user attacker (M3-R17c).** All need someone who can already write the table's folder, which §8 does not defend against:
  - A hard link at `<name>.gloss-new` is truncated by `O_TRUNC`, which loses the other file's contents.
  - A FIFO at the table name or at `<name>.gloss-new` blocks `save`.
  - A planted symbolic link at `.gloss-new` makes every save fail with the symbolic-link message (safe, but blocking).
- **Load (M3-R24).** A FIFO at the table path blocks `load` forever. Refusing it needs an `fstat` primitive after the open. The program can be killed and no data is lost.
- **Empty file (M3-R28).** A zero-byte table file loads as an empty table without a notice, so a file truncated by a crashed tool loads quietly and a later `save` writes the header.
- **Sync errors after the rename.** If the folder sync or a close fails after the rename, the message says `cannot be written` although the table was replaced.
- **Mode 300 folders (M3-R8).** A folder that is write and search only cannot be opened `O_RDONLY`, so a save into it is refused.
- **Memory:** nothing is freed, as in M2. Memory-limit runs skip under QEMU, so the out-of-memory case is checked only by CI.
- **Save, probe fall-through.** When the probe open of the target fails in a way other than not found, `save` falls through to the write; refusing on `ESCAPES` there would be optional hardening.
- **`EBUSY` constant.** The `EBUSY` constant sits outside the target module, following the `irsim` errno pattern.
- **BOM:** a loaded and saved file loses its byte order mark (M3-R23).
- **Deferred:** test fixtures and pretend files (M4), `trace`'s `--allow` (M5).
- **Reviews:** the approval logic, `.allow` parsing, open and save paths, symbolic-link handling, safe writing, the new encoder primitives and the CSV reader each had a first review on the top tier.

The milestone ledger (every ruling, review record and carry) is kept in `.superpowers/sdd/2026-11-24-compiler-m3-tables-files/progress.md`.

### Milestone 4 (`gloss test`), finished 2026-11-25, `main` @ 4469cc9 plus the open stack and this documentation

**What shipped:**
- **`gloss test FILE`.** It checks the program (tests included), builds a test executable with the Test backend, runs it from a private folder (mode 0700, deleted on every path) and passes on its status: 0 all passed, 1 a test failed, 3 a compile error (D37). A program with no tests prints `No tests in x.gloss` and builds nothing.
- **The driver.** Generated IR forks one child per test, in source order, and waits for it. `Fork`, `Wait` and `DefaultChildSignal` are new IR primitives with `irsim` models (D31, D32). Each child starts from the same untouched heap, tables and globals.
- **The report.** `pass`/`fail` blocks, a blank line and `N passed, M failed`, written with `Prim::Write` through their own routines (D33). A child that ends other than 0 or 1 is `fail` with `ended without a result (status N)`.
- **The Test backend.** `add_reachable` takes a `Backend`; `write_all`, `fail_begin`, `fail_end` and `ask` are replaced by name, and `load` and `save` lower to `pretend_read` and `pretend_save`. A 2 KiB test state global holds the title, the `shown` bytes, the input answers, the failing flag, the failure text and the current line.
- **Statements.** `expect`, `input` (answers must be text), `shown`, `run start`, `ask` from the queue, tables that start empty and cleared, and pretend files that use the real CSV writer and reader (D34–D36).
- **Tests.** Cases in `tests/test_cases/` run as real executables and under `irsim`; the language spec's §13 program is a case; four examples carry tests that `tests/examples.rs` runs (D39).
- **Docs.** `spec/v0.md` §9, §10 and §11 describe what the compiler does; the README has a Tests section.

**Pull requests.** #410–#417 are merged (tasks 1 to 5). The rest are stacked branches, not yet opened; their PR numbers are added at merge. Plan #409.

| Task | What | PRs or branches |
|---|---|---|
| 1 | `Fork` and `Wait` in the IR | #410 |
| 2a, 2b | `irsim` models `Fork` and `Wait`; its tests | #411, #412 |
| 3a, 3b | Linux `fork` and `wait4`; difftests | #413, #414 |
| 4 | The test-state global | #415 |
| 5a, 5b | The backend switch; `err_int` through `write_all` | #416, #417 |
| 6 | Report routines: lines, pass block, summary | `m4/t06a-report-lines`, `t06b-report-pass`, `t06c-report-summary` |
| 7 | Test state, test backend, `test_expect` | `m4/t07a-test-state` … `t07c-test-expect` (6 branches) |
| 8 | Test-mode lowering, the driver, `build_test_executable`, SIGCHLD | `m4/t08a-lower-tests` … `t08d-sigchld-real-run` (13 branches) |
| 9, 10 | `expect`; tables start empty and cleared | `m4/t09-expect`, `m4/t10-tables-cleared` |
| 11 | `testrun` and the `gloss test` command | `m4/t11a-testrun`, `t11a2-run-program`, `t11a3-run-failures`, `t11b-test-command` |
| 12, 13 | Test-case harness, `irsim` cases; failure and isolation cases | `m4/t12a-case-harness` … `t13b-isolation-cases` |
| 14, 15 | `input` text rule; `input` and `ask` | `m4/t14-input-text`, `t15a-input` … `t15c-ask-lowering` |
| 16, 17 | Ask cases; `shown` | `m4/t16-ask-cases`, `t17-shown`, `t17b-shown-case` |
| 18, 19 | Split `table_read`; pretend `load` and `save` | `m4/t18-split-table-read`, `t19a-pretend-read` … `t19b2-pretend-save-lowering` |
| 20 | Pretend-file cases and CSV quirks | `m4/t20a-pretend-cases`, `t20b-pretend-quirks` |
| 21 | Language spec: tests as built; the §13 program as a case | `m4/t21a-spec-example`, `t21b-spec-text` |
| 22 | Examples gain tests | `m4/t22a-example-tests`, `t22b-example-tests-2` |
| 23 | README; this section | `m4/t23a-readme`, `m4/t23b-as-built` |

**Deviations from the decisions above** (rulings are numbered M4-R1 … M4-R17 in the milestone ledger, and the plan's P1–P18 are in the plan):
- **D38:** `gloss test` refuses unless the host is x86-64 Linux, not "non-Linux" (P6), with the same message and status 3. The refusal comes after the compile errors, so a Mac user still sees them.
- **D32:** three primitives, not two. `DefaultChildSignal` (`rt_sigaction`, 13) gives `SIGCHLD` its default disposition at the start of the test executable, because an inherited ignored `SIGCHLD` makes every `Wait` fail (M4-R12).
- **P8 and D31:** the test run is streamed, not captured: standard output and error are piped and copied chunk by chunk (M4-R1, M4-R4), so a hung test still leaves every finished result on screen. Standard input is null.
- **D18:** reworded to cover generated code, IR, `lower`, the runtime and `irsim` only; the host-side `testrun` decodes `ExitStatus` itself (M4-R4).
- **D36:** `table_read` and `table_rows` share one `parse_rows` emitter, so each executable holds one parser copy and real builds stay byte-identical (M4-R16).
- **§4.8 and scan 27:** `tests/examples.rs` runs each example's tests through `build_test_executable` and the runner, not through `gloss test`, which refuses on a host that is not Linux.
- **D39:** no newline-in-text case in `csv_quirks_round_trip` (M4-R17).
- **D34 and P7:** failures 6, 7 and 8 cannot occur in a program in a test; they are reachable only from a unit test.
- **P3:** `input` answers must be text (a checker error otherwise); `spec/v0.md` §9 states it (M4-R5).
- **Plan:** task 2 became two branches, task 6 three, task 7 six, task 8 thirteen, to stay under the 200-line cap.

**As-built note (task 5).** After 5b, a `write_all` failure while `err_int` writes digits exits 2. Before it, `err_int` used `Prim::Write` for each digit and ignored a failed write. `fail_begin` already used `write_all`.

**As-built note (final review).** Since task 4 every executable laid out `Global::TestState` (2 KiB more memory in the data segment). The backend now leaves it out of an executable that never takes its address, so a real build has M3's layout again; only a test executable reserves it. `TestState` is the last global, so no other global moved.

**Rulings** (final state):

*Plan and process*
- M4-R1: P8 changed to streaming output before merge; a hung test must leave finished results visible.
- M4-R2: every proposed ruling in the pre-flight scan (problems 1–28) was accepted as written.
- M4-R3: `test_begin` allocates the shown buffer, and `test_fail` and `test_expect` flush before setting `FAILING`, so output shown before a failure is kept.
- M4-R4: §4.8 amended for piped streaming and a null standard input, and D18 reworded (see above).
- M4-R5: P3 kept; `input` answers are text, and `spec/v0.md` §9 says so.
- M4-R6: task 1 was reviewed by the controller, not on the top tier; the semantics were reviewed in tasks 2 and 3.

*Fork, wait and `irsim`*
- M4-R7: `Faults` derives `Clone`, so the child machine can be cloned.
- M4-R8: `irsim` panics at `Fork` if the parent holds open descriptors, because file offsets would be shared; the driver parent never opens files.
- M4-R9: the brief's difftest names are cases of `system_programs_agree` plus `wait_decodes_a_signal`; same coverage.
- M4-R10: `rcx` around `Fork` and `Wait` is pinned only by lowering tests, since QEMU's syscall keeps `rcx`; a native runner would add coverage.
- M4-R11: the driver checks `Fork`'s result before `Wait`; a negative result is reported lost and `Wait` is never called, because `wait4` with a pid of 0 or less waits for a process group (fixes P14).
- M4-R12: the test executable resets `SIGCHLD` to default before any fork (the third primitive).

*The test backend*
- M4-R13: the shown buffer keeps at least `IO_BUF_BYTES` free after every non-failing `shown_add`, so a failing flush always fits without allocating; each test uses 4 KiB more memory.
- M4-R14: in test builds, lowering stores each statement's line in the state's line word, and growth of `shown` passes that word, so an out-of-memory failure names the `show` line.
- M4-R15: after a user-feature call returns, lowering stores the caller's line again, so R14 names the line of the `show` and not the callee's last statement.

*Pretend files and cases*
- M4-R16: `table_read` and `table_rows` share one `parse_rows` emitter (see D36 above).
- M4-R17: no newline-in-text case; v0 source cannot make text with a line break, and the M3 CSV unit tests cover newline cells.

**Tests:** 847 pass and 1 is ignored (the stress test) at `m4/final-c-test-state-only-in-tests`, after the final-review fixes: 706 unit tests and 141 integration tests in 28 test binaries (the Linux-only CLI tests are not compiled on the Mac), from `CARGO_BUILD_JOBS=4 cargo test` on the Mac. They include 153 run cases (`MIN_CASES`, 152 in M3), 18 test cases (`MIN_TEST_CASES`, each run as a real executable and under `irsim`), 11 examples (`MIN_EXAMPLES`) of which 4 carry tests (`MIN_EXAMPLES_WITH_TESTS`). `MIN_TEST_CASES` rose 2, 5, 7, 10, 11, 14, 16, 17 over tasks 12 to 21, and 18 in the final review. The tests of the `gloss test` command that need a Linux host were run in the machine (see below): after the task 11 review fixes `tests/cli.rs` passed 36, `testrun` 14 and `tests/test_runs.rs` 3.

**Known limits and accepted risks:**
- **A looping test hangs (D40).** v0 has no time limit. Streaming (M4-R1) means the finished tests' results are already on screen.
- **No `--permissions`.** A test executable leaves out `permissions_text`, so it does not answer `--permissions`.
- **Signals.** The test executable resets `SIGCHLD` and nothing else. Any other disposition the caller set (an ignored `SIGINT` from a `nohup`-style parent, say) is inherited; an ignored `SIGPIPE` is not, because Rust's `Command` restores its default in the child, and `gloss test` starts it from the compiler's own process.
- **A broken pipe leaves an orphan (R19).** When the output writer fails (`gloss test x.gloss | head -1`), only the driver is killed. A looping test child still holds the pipes, so `gloss test` waits for it and the orphan keeps running. std has no process-group kill without libc; this follows from D40.
- **Exit status 3 for internal failures.** `gloss test` also exits 3, with a message on standard error, when the private folder cannot be made, the executable cannot be written or started, or the output cannot be written. The language spec lists 3 for compile errors; the README and spec section 11 now say 3 covers these as well.
- **Working directory (R18).** The test executable runs with the private folder as its working directory. Pretend files make the folder irrelevant, so nothing outside it can be touched by a relative path.
- **`ETXTBSY`.** If another thread forks while the executable is written, the file can still be open for writing and the spawn fails with `ETXTBSY`. `gloss test` retries up to 5 times with a backoff from 20 ms to 320 ms. The race itself is not reproduced; the retry path is tested through a seam.
- **Out-of-memory line cases.** The line named by an out-of-memory failure inside `test_input` and while `shown` grows is pinned only in the IR (M4-R14), not by an end-to-end run.
- **Linux only (D38).** On a Mac, the tests that start `gloss test` as a program cannot run natively. They are cross-compiled with `CARGO_TARGET_X86_64_UNKNOWN_LINUX_MUSL_LINKER=rust-lld cargo test --target x86_64-unknown-linux-musl --no-run` (after `rustup target add x86_64-unknown-linux-musl`) and the printed executables are run in the OrbStack machine with `orb -m gloss-amd64 sh -c "cd <worktree> && ./<exe>"`. The machine has no network or Rust. The time bounds in `tests/scaling.rs` (10 seconds, for example `a_long_chain_of_variables_declared_in_reverse_checks_quickly`) do not apply under QEMU, where the compiler itself is emulated; a failure there should be re-run, not read as a regression. Natively, on the Mac and on CI, the bounds hold.
- **Deferred:** `trace` (M5), `explain` (M6), `flowchart` (M7); a time limit; fixtures that inject a bad file.
- **Reviews:** `Fork` and `Wait` (target numbers, status decoding, the `irsim` clone) and the pretend-file backend each had a first review on the top tier.

The milestone ledger (every ruling, review record and carry) is kept in `.superpowers/sdd/2026-11-24-compiler-m4-tests/progress.md`.

### Milestone 5 (`gloss trace`), finished 2026-11-26, merged to `main` @ de25111

**What shipped:**
- **`gloss trace FILE`.** It runs the steps of `build` (diagnostics, the `.suffix` rule, `--allow`, approval), checks the host, builds a trace executable, writes the events to `NAME.trace`, narrates them on standard output and passes the program's standard error through. Status: 0 finished, 2 a run-time failure, 3 an internal or setup fault, 4 not approved (D48).
- **The trace build.** `lower_trace` and `Backend::Trace` produce an executable that writes one JSON event per line (spec §10). `Global::TraceState` is laid out only in a trace build, so a real build's layout and a test build's layout are unchanged (a test asserts neither holds `trace_state` or a trace routine). The real build's code changed only by three splits that keep its behaviour: `out_bytes` / `buf_bytes`, `write_all` / `write_loop` and `ask` / `ask_read` (D43, §4.9).
- **Events.** `show`, `ask`, `set` (variables, fields, items, places, `append`, the four verbs), `check`, `loop`, `enter` / `leave`, `file` and `fail`, with the key order of §4.9.
- **The narrator.** `src/trace/` reads events with its own small JSON reader and prints the sentences of spec §10. `Narrator::line` returns `Narrated::{Event, Failed}` (a `fail` event writes its message on standard error and returns `Failed`) or an error that names the event number.
- **`testrun` for traces.** The traced run is streamed: standard output is split into whole lines in linear time, standard error is inherited, and the child is killed if the reader panics.
- **Tests.** 47 cases in `tests/trace_cases/` (`MIN_TRACE_CASES`), each run as a real executable, under `irsim`, and through the narrator; six failing cases also agree with a real build's standard error under irsim (M5-R14). Spec §10's example is the case `spec_trace`, and `tests/spec_examples.rs` pins the spec text to it.
- **Docs.** `spec/v0.md` §10 and §11 describe the events, the sentences and the messages; the README has a Trace section.

**Pull requests.** #475 (the plan), then #476 to #567, 92 branches (`m5/t01a-trace-state` … `m5/final-f-trace-flush`, each at most 200 changed lines and 10 files), rebase-merged in order on 2026-11-26. The merged tree equals the tested tip.

| Task | What |
|---|---|
| 1 | Trace state, backend switch, `lower_trace`, `build_trace_executable` |
| 2 | `buf_bytes`, `out_bytes`, `json_bytes` (escaping) |
| 3 | `json_value`: typed values |
| 4 | `show` event |
| 5 | Case harness; `irsim` cases |
| 6 | `fail` event, UTF-8 carry, not-found notice |
| 7 | `ask` event |
| 8, 9 | `set` events on variables, then fields, items and `append` |
| 10, 11 | `check` and `loop` events |
| 12 | `enter` / `leave`, depth test, stack-check sizing |
| 13 | `file` events |
| 14, 15 | JSON reader; typed events |
| 16, 17 | Value text; condition sentences |
| 18 | All sentences; narration cases; `Narrated`; R14 |
| 19 | The traced run (`testrun`) |
| 20 | The `gloss trace` command |
| 21 | Language spec; `spec_trace` case |
| 22 | README; this section |

**Deviations from the decisions above** (rulings are numbered M5-R1 … M5-R16 in the milestone ledger; the plan's P1–P8 are in the plan):
- **§4.9, `show` (P4, M5-R5):** the plan kept a `show` event open while its items ran. A call inside an item (`show "t: ", (sum with xs)`) then wrote its own events inside the open event, which breaks D43. Now `show` evaluates every item into hidden slots first, then opens the event and writes from the slots. Cost: a failure part-way through a `show` writes no `show` event, although the real program prints the partial line. The `fail` event still names the line. `fail_begin` keeps its `OPEN` close only as a guard.
- **§4.9, invalid bytes (P3, M5-R6):** an invalid byte is written as U+FFFD itself (EF BF BD), not the escape `�`. The two are the same JSON, and the raw form keeps the file readable.
- **§4.9, failure message and UTF-8 (M5-R4):** `err_put` writes in 24-byte chunks that can split a character, so the trace `write_all` keeps an incomplete tail (at most 3 bytes) in the new `CARRY` word, and `fail_end` writes one U+FFFD per leftover byte, the same per-byte rule as `json_bytes`. The trace state therefore has four words, not three.
- **§4.9, `start` (P5, M5-R10):** `start` has no `enter` or `leave` event. Writing them would have changed all 37 existing `.trace` files at once, and the narrator prints nothing for them. Depth 0 is inside `start`.
- **D46 and indentation (M5-R11):** an `enter` or `leave` sentence is indented as the caller is, and the callee's own sentences are 2 spaces deeper. That reads as a call staircase.
- **D45 (M5-R7, M5-R8):** a check event's `values` lists a plain name used as an item position (`i` in `item i of xs`), so `item 2 of xs is 3` can be read without reading anything that can fail. It does not list the indexed list, the item read, or the root of a field read (`e` in `amount of e`). A list or table name is never listed, because `repeat while count of xs …` would write the whole list on every turn. The narrator keeps unlisted parts as written.
- **D47 (M5-R12, M5-R13):** events carry no item type, so an empty list is narrated `empty list`, not `empty list of number`. Text over 60 characters is cut inside its quotes and ends `…"`.
- **D44 (task 8, task 9):** the verbs read `from` from the variable's slot, not from a copy saved before the operation. The value is the same. A set on a place (field, item) uses the value's own type for its text, which equals the declared type in a checked program.
- **D50 (M5-R9):** `fail_too_deep` is not a case folder: one `enter` per level gives a `.trace` of about 100,000 lines. Two tests run the traced program on Linux and under `irsim` and compare only the tail (the last `enter`, then the `fail` event, with no `enter` after the failed stack check). The two hosts fail at different depths, and each is compared with its own tail.
- **Stack sizing (task 12):** in a trace build `enter_event` sits between a call's `StackCheck` and the call, so the check was never sized for the callee. Each check now pairs with the next `f_` call, and a twin test fails on the old code.
- **D48 (M5-R15, task 20):** an event line the narrator cannot decode stops the traced program, and `gloss trace` prints `the traced run wrote an event gloss cannot read (event N: why)` and exits 3. That means gloss itself is wrong. Exit status 2 without a `fail` event is `the traced run ended without finishing (status 2)`, not silent. A write failure of the narration prints `cannot write the trace output`, and one of the file prints `NAME.trace cannot be written: <reason>`; both exit 3. The `.trace` file holds every complete event line the program wrote, a line the narrator cannot read included. A last line without its newline (unfinished) and a line that is not UTF-8 are not written, and both can only come from a gloss bug. The file is written through a `BufWriter`: each line and its newline are one write, flushed at once, so Ctrl-C keeps every line so far; it is flushed again when the run ends, on every path, before any closing message is printed.
- **D48 (M5-R14):** the narrator's standard error for a failing case is also compared with a real build of the same program run under `irsim`, because the trace's own decoder produced the expected text otherwise.
- **Final-review fixes (M5-R16 and the Minor findings):** `gloss trace` setup failures say "traced run", not "test run"; the approval refusal says "Trace in a terminal" (`gloss build`'s wording is unchanged); a truth condition that is not a comparison is narrated `<it> is yes` / `<it> is no`; the real/test-build invariant test checks every JSON routine by name.
- **Plan:** task 1 became four branches, task 2 five, task 6 eight and others two to five, to stay under the 200-line and 10-file caps (M5-R2, M5-R3).

**Rulings** (M5-R1 … M5-R16; the ones that changed the design are above):
- M5-R1: `Checked::program()` returns the program, because `Narrator::new` takes `&Program` and `Checked` hides its fields.
- M5-R2, M5-R3: how tasks 1 and 2 were split, and a decode refactor in `shown.rs` ahead of `json_bytes` (one UTF-8 decoder, not two).
- M5-R4 to M5-R15: see the deviations above (carry, hoisted `show`, U+FFFD, `values`, deep recursion, `start`, indentation, empty list, long text, stderr comparison, unreadable event).
- M5-R16: a `show` item that calls a feature which shows is narrated in a different order than the program prints; accepted as a known limit (below).

**Tests:** 995 pass and 2 are ignored (the stress test and the probe run by its parent) in 31 test binaries from `CARGO_BUILD_JOBS=4 cargo test` on the Mac, at `m5/t21-spec`. The Linux-only tests (the `gloss trace` command tests in `tests/cli.rs` and the real-executable trace cases) were run in the OrbStack machine at the end of each task, and at the end of task 20 `tests/cli.rs` passed 40 there. The three trace harnesses read the same cases: `tests/trace_cases.rs` (real), `src/irsim/trace_cases.rs` (`irsim`) and `tests/trace_narration.rs` (the narrator, any host). `MIN_TRACE_CASES` rose from 12 (task 7) to 17, 26, 30, 34, 37, 40, 41, 44 and 45 over tasks 8 to 13 and 18, and to 46 in task 21, and to 47 in the final review (`show_calls_show`, M5-R16). Real builds are unchanged except for the `buf_bytes`, `write_loop` and `ask_read` splits. After the final-review fixes (`m5/final-a` … `m5/final-f`): 999 pass and 2 are ignored in 31 binaries on the Mac; in the OrbStack machine `tests/cli.rs` passed 43 and `tests/trace_cases.rs` 6.

**Known limits and accepted risks:**
- **No event cap (D49).** A run with millions of events produces a very long narration and `.trace` file. Ctrl-C stops it.
- **A program that never closes stdout.** `gloss trace` reads the program's standard output to the end, so a program that never ends (or leaves stdout open in a child) runs until interrupted.
- **Notice ordering.** The not-found notice of `load` is written on standard error by the program. The narration is written by `gloss` on standard output. Their order on a terminal is the order the two pipes deliver, not the order the program wrote them.
- **Failure depths differ.** Failure 10 (nested too deep) happens at different depths on Linux and under `irsim`, which is why task 12 compares only tails (M5-R9).
- **The depth test is large.** On Linux the test runs the traced program to its stack limit. QEMU (OrbStack on a Mac) ignores `ulimit -s`, so it writes about 7 MB of events there.
- **A write error can leave half a line.** If writing an event fails part-way, the last line of the file may be incomplete, and D48 treats the run as ended without finishing (accepted).
- **A nested `show` is narrated out of order (M5-R16).** When a `show` item calls a feature that shows, the feature's lines come first and then the whole outer line (`inner`, then `ax`), while a real run prints `ainner` and then `x`. A fix would need partial-line `show` events, which is a design change. Spec §10 says so, and the case `show_calls_show` pins it.
- **`show` before a failure.** The partial line of a `show` that fails part-way is lost from the trace (M5-R5).
- **Unit coverage only.** A narrated list of exactly 1 row or 1 item is pinned by unit tests, not by a case.
- **Deferred:** `explain` (M6), `flowchart` (M7), the step-through viewer (v0.1).
- **Reviews:** tasks 2, 6, 7, 8 and 12 (the backend swaps, `ask`, the set events and the call-site events) had their first review on the top tier; every other task was reviewed by Sonnet or, for the small ones, by the controller.

The milestone ledger (every ruling, review record and carry) is kept in `.superpowers/sdd/2026-11-25-compiler-m5-trace/progress.md`.

### Milestone 6 (`gloss explain`), finished 2026-11-26, branches stacked on `main` @ b952692, not yet merged

**What shipped:**
- **The `assemble` split.** `program::executable` is now `assemble` (every function's instructions, then the arithmetic helpers) followed by the same encode and ELF steps. The bytes of every example executable are unchanged (sha256 checked by the controller at each step that touched lowering or the backend).
- **The target-neutral listing.** `Target::listing` returns `Routine`, `Item` and `Ref` values (`src/target/listing.rs`); the x86-64 target fills in the refs (slots, inputs, texts, jumps, system call names). `src/explain/` never sees an instruction type.
- **The notes.** `lower_explained` returns the module plus `Notes`: slot names, inputs, and each failure's code, line and message. `lower` drops them. A test shows that taking notes does not change the module.
- **The printer.** `src/explain/` writes the header, the names block, line headers with `(continued)`, failure headers, comments at column 36, and then `runtime` with every other routine in layout order.
- **The command.** `gloss explain FILE` shares `build`'s diagnostics, never asks for approval, runs on any host, and maps a write error to `cannot write the explanation` (status 3), so a closed pipe does not panic.
- **The golden cases.** `tests/explain_cases/` with a bless switch; spec §10's example is the case `spec_explain`, and `tests/spec_examples.rs` pins the spec text to it.
- **The spec.** `spec/v0.md` §10 and §11 describe the listing and its messages; the README has an Explain section.
- **The examples amendment (owner, mid-milestone).** Every example now has `test` blocks and a `.tests.out`, and a new example, `inventory`, covers a table with pretend files in its tests.

**Pull requests.** #568 (the plan); the work is stacked branches `m6/t01-assemble` ... `m6/t13-as-built`, each at most 200 changed lines and 10 files; PRs: listed at merge.

| Task | What |
|---|---|
| 1 | `assemble` split (`m6/t01-assemble`) |
| 2 | Listing items and their tests (`t02a`, `t02b`) |
| E1 to E3 | Tests for every example; the `inventory` example (amendment 1) |
| 3 | Refs: slots, inputs, texts, jumps, system calls |
| 4 | `Notes` and `lower_explained` |
| 5 | Failure notes |
| 6 | Line headers and `(continued)`; loop line markers (`t06b`) |
| 7 | Comments, failure headers and the comment column |
| 8 | Runtime routines and the completeness test |
| 9 | The `gloss explain` command |
| 10, 11 | Case harness and the golden cases (`t10a` to `t11g`) |
| 12 | Language spec §10, §11 and the `spec_explain` case |
| 13 | README; this section |

**Deviations from the decisions above** (rulings are numbered M6-R1 ... M6-R10 in the milestone ledger, written R1 ... R10 there):
- **D51, helpers (R4):** the plan split the helpers at every `x86_64.` label. `arith` has inner labels with that prefix and a third entry the plan missed, so the split is at `arith::ENTRIES` = `multiply_number`, `divide_number`, `remainder_number`: three helpers, not two. Inner labels are listed with `local: None`.
- **Plan sample (R6):** the plan's sample program used `to total_of with prices`, which is not Gloss. The tasks used `feature total_of` with `needs prices as list of number`. No effect on the design.
- **D52, loops (R7):** lowering emitted no line marker before the step-and-test code of `for each`, counting loops and `repeat while`, so that code listed under the body's last line. Lowering now emits a bare `Inst::Line(loop line)` there. A line marker encodes nothing and `irsim` skips it; the controller checked that the example executables are byte-identical.
- **D52, feature line (R9, R10):** code that belongs to the feature as a whole lists under the feature's header line. The printer heads instructions before the first `Line` with the feature's line, and lowering marks the end guard (`fall_off_end`) and the implicit return of a feature that gives nothing with a bare `Inst::Line(feature line)`, shown `(continued)`. Executables are unchanged (sha check).
- **`otherwise` (R10):** it has no header of its own. Its jump belongs to the branch before it, and its statements carry their own lines.
- **D54, loops (final review I1):** a loop variable is listed once for each name and type. Two loops with one name and one type share a slot and a line; a second loop that reuses the name with another type gets its own line, with its own loop line.
- **D53, text (R8):** a text comment is cut after 30 characters of the decoded text, counted before quote doubling and `\u{XXXX}` spelling. The cut never splits an escape or a doubled quote.
- **D57, bless (R2):** the case harness gets `GLOSS_BLESS=1`, because the repo had no bless convention. A bless run can create a new case's `.out`, skips dotfiles in the sweep, and fails after blessing so it is never left on.
- **Spec §10 example:** it is 21 lines and includes the entry code (the `line 1: feature add_to_total` block), where the plan expected the first source line.

**Rulings** (R1 ... R10 in the ledger; the plan's P1 to P4 are in the plan):
- R1: the overflow test compares the `jo` comment with `failure 4: ` plus the note's message, not a literal source text.
- R2: `GLOSS_BLESS=1` on the explain_cases harness only.
- R3: chain mode as M5: each branch is cut from the previous one, and the owner merges the chain at the end.
- R4: three helpers at `arith::ENTRIES`.
- R5: task 2 was over the cap and became `t02a` (code) and `t02b` (tests).
- R6: the plan's sample syntax was wrong; real syntax used.
- R7: loop line markers in lowering (`t06b-loop-lines`).
- R8: text comments are cut at 30 characters.
- R9: entry code and the end guard list under the feature's line.
- R10: the implicit return does too; `otherwise` gets no header.

**Tests:** the last gate, at `m6/t12-spec`, was 1062 passed, 0 failed (`CARGO_BUILD_JOBS=4 cargo test` on the Mac; 1002 at task 1). `MIN_EXPLAIN_CASES` is 8 here, 13 after the final review. After the final-review fixes the gate is 1073 passed, 0 failed, and the example executables are still identical. `MIN_EXAMPLES` is 12 and so is `MIN_EXAMPLES_WITH_TESTS`. Invariants cover what the golden files do not: the printed instruction lines of every example and every explain case equal `inst::text` of the assembled instructions, in order (comments cut, `.L<n>` mapped back; a unit test in `src/explain`), every routine is listed in layout order, and taking notes does not change the module. Linux (musl, in OrbStack) was run at task 1 and with the examples amendment. The controller took sha256 sums of the example executables before the work and after each step that touched lowering or the backend (tasks 1, 6, 10 and 11); all were identical.

**Known limits and accepted risks:**
- **The listing is long.** `gcd` is 1293 lines, almost all runtime.
- **Runtime bodies are not golden.** The cases pin the program part and the routine names; the invariants cover the bodies (P4).
- **The 30-character cut counts characters.** It never splits an escape, but it can split a character built from several, such as a flag or a letter with its accent written separately. Full grapheme segmentation is outside std.
- **Runtime routines have no slot names.** Only feature functions have notes.
- **Comments come only from refs.** An instruction that refers to nothing has no comment.
- **A flaky gate run.** At task 2 one run printed "855 passed, 2 failed"; the rerun was clean, and so were all later runs. The cause was not found. Watch for it.

**Final review.** The whole-milestone review (Opus, b952692..ba9f82a) gave the verdict "ready to merge: with fixes", with three Important and eight Minor findings, all fixed on `m6/final-a-...` to `m6/final-j-...`; no executable changed (sha256 of every example identical before and after, except that M7 changes one note string that only Explain prints).
- I1: a loop variable is listed once for each name and type (a number loop then a text loop gives two lines). I2: line headers and the entry header go through `spell_invisible`; `chars::unshowable` covers every control character, so a tab is spelled too, though a tab in a real program is already a parse error. I3: fixed in the printer, not in lowering: labels directly followed by a `Line` print after its header, in feature and runtime routines alike (goldens `if_otherwise` and `loop_continued` moved their labels).
- M1: the remembered system-call number is forgotten by every instruction except `movabs rax, N` and `push`. M2: a unit test compares the printed instruction lines of every example and explain case with the assembled ones; the As built text now says so. M3: new cases `repeat_while`, `nested_count`, `table_records`, `ask_number` and (after the re-review) `table_save`; `MIN_EXPLAIN_CASES` is 13. The Sonnet re-review confirmed every finding fixed and raised three minor points, also fixed: a failure label stays right after its `failure` header even before a line header, the testing paragraph names where the invariant lives, and `f_NAME` has its own bullet in spec §10. M4, M5, M6, M8: spec §10, §11 and the README now name Explain's write failure, the `.L<n>` rule, `f_NAME` and the character cut. M7: the item-failure note reads `it has …`, the count and its word being known only at run time.
- Tests: 1072 passed, 0 failed (1062 at `m6/t12-spec`).

**Deferred:** `flowchart` (M7), the step-through viewer (v0.1).

The milestone ledger (every ruling, review record and carry) is kept in `.superpowers/sdd/2026-11-26-compiler-m6-explain/progress.md`.

### Milestone 7 (`gloss flowchart`), finished 2026-11-27, branches stacked on `main` @ 8c9448d, not yet merged

**What shipped:**
- **The graph (`src/flowchart/graph.rs`).** `Graph { nodes, arrows, regions }`: nodes in source order (start first, end last), arrows of kind `Next`, `Yes`, `No`, `Each`, `Done`, `Skip`, `Stop`, `Give`, and `regions` (each loop and `if` chain as node ranges). Labels are the statement's source lines, continuation included, with invisible characters spelled.
- **The layout (`src/flowchart/layout.rs`).** One row per node, columns from the block structure, canonical column `centres`, loop back and done lanes and the give lane in the gaps between columns, and `LABEL_ROOM` of 40px.
- **The routes (`src/flowchart/route.rs`).** Tracks per gap (R17), then gap heights and row positions are recomputed from them (R1). A back arrow leaves as its forward kind would, takes a track below the loop's last row, runs left to the back lane, up, and right into the decision (R15). A `done` or `stop` arrow that is the last of an inner loop's body runs through the inner done lane, then as a back arrow.
- **The checker (`src/flowchart/check.rs`).** `invariants` has rules 1 to 6 (boxes overlap; a segment through a foreign box, including its own target; two targets on one segment; a segment not horizontal or vertical; node count and bounds; a segment through an arrow label), gap heights, and route end points. `well_formed` is a small XML checker for the SVG. One sweep lays out and checks every feature of every `examples/**/*.gloss` and `tests/**/*.gloss` (243 files, 309 features at Task 5, none skipped; the example floor is 12).
- **The SVG (`src/flowchart/svg.rs`).** As §4.11, with one element per line and the same bytes for the same program.
- **The command.** `gloss flowchart FILE` (`src/cli.rs`) shares `checked_or_status` with `explain` and `test`, writes `<stem>.<feature>.svg` through `write_atomically` (mode 0644), prints `Wrote <file>`, and exits 3 on an error or an unwritable file. `COMMANDS` drives the usage check (R22). `Checked::flowcharts(name)` is the library entry.
- **The golden cases.** `tests/flowchart_cases/` with `straight`, `if_chain`, `loops`, `jumps`, `calls`, `continued` (`MIN_FLOWCHART_CASES` is 6) and `GLOSS_BLESS=1`; a bless run deletes stale SVGs, names them, and fails so it is never left on (R25). The explain harness got the same bless fix.
- **The spec.** `spec/v0.md` and the README describe `flowchart`; the README example is real output.

**Pull requests.** #610 (the plan); the work is stacked branches `m7/t01a-...` to `m7/t11-as-built`, each at most 200 changed lines; PRs: listed at merge.

**Deviations from D59 to D65 and §4.11** (rulings are written R1 ... R26 in the ledger; each correction is marked in §4.11):
- **No `Back` arrow kind (P2).** A back arrow keeps its forward kind; `is_back` is `to <= from`, so an empty loop body's self-arrow (R8) is a back arrow.
- **Lanes in the gaps between columns (R9).** Not 16px beyond the leftmost and rightmost boxes; this keeps every lane clear of every box and column centre line. Charts can be a little wider.
- **Column centres are canonical (R11).** `Layout.centres` is the x of every vertical arrow, and a box is centred on it to within half a pixel.
- **`gaps[0]` is unused (R12).** `gaps` has one entry per row; row 0 has no gap above.
- **The give lane sits right of the last done lane (accepted at Task 3),** or the last column edge, so it never runs along a done lane.
- **Tracks count only targets with a horizontal segment in the gap (R17).** An arrow that only drops through adds no track; the checker counts the same way.
- **40px label room (R20) and rule 6.** The first gap right of every column is 40px, so a label at (x0+4, y0+12) clears the turn of its own arrow; rule 6 makes the checker refuse a segment through a label box (8px a character, 12px high). Charts are 24px wider per column.
- **`Checked` keeps the source as written (R26).** `Checked::src` joins a continued statement onto every one of its lines, which `explain` needs; the flowchart read it, so a continued label collapsed to one line and a feature ending in a continued statement ended early. The `continued` golden case found it. `Checked.written` is `Source::new(text)`, used only by flowcharts, with a `Checked`-level test.
- **Track order (R14).** Tracks in a gap run by target row, then arrow kind, so a back arrow's drop and a forward arrow's final drop never overlap.

**Tests:** the last gate, at `m7/t10-spec`, was 1163 passed, 0 failed (1084 after Task 1, 1104 after Task 3). Unit tests in `src/flowchart/` (70) guard each node kind, label rule, arrow kind, the geometry and the routes; mutants were run by the reviewers and each surviving one got a test. The checker has a test per rule that breaks the rule on purpose, so no rule can pass unseen. The sweep checks every feature of every example and program for the invariants and a well-formed SVG. The golden cases (10 tests in `tests/flowchart_cases.rs`, 6 cases, 10 SVGs) pin the exact bytes; the controller rendered each SVG and approved it by eye. `tests/cli.rs` has 7 flowchart tests (statuses, messages, file names, the kept files, a bare file name, warnings only), and `src/cli.rs` and `src/lib.rs` one each.

**Rulings** (R1 ... R26 in the ledger; the plan's P1 to P4 are in the plan):
- R1: T4 owns gap heights. R2, R6, R10, R16, R18, R21, R23: over-cap tasks split into more branches. R3: continuation ends at a trailing comma. R4: `End` is the last non-blank line of the feature. R5, R13: overflow tests. R7: `skip`/`stop` outside a loop are a compile error, so the builder asserts. R8: the empty-body self-arrow is kept.
- R9 to R12, R14, R15, R17: lanes, centres, `gaps[0]`, track order, back arrows, tracks (above). R19: the sweep counts skipped files and asserts a floor. R20: label room. R22: `COMMANDS`. R24: the flowchart sweep is not shared with explain's. R25: bless deletes stale SVGs. R26: the source as written.

**Known limits and accepted risks:**
- **Text width is 9px a character.** A wide (CJK) character overflows its box.
- **A chart with no loop or `give` keeps an empty give lane and label room** on the right: v0 has one fixed layout (spec §15).
- **Tall charts are long.** One node a row, no compact columns.
- **The give lane is always reserved.**

**Process notes:** Tasks 1 to 6 and 8 went over the 200-line cap and were split (R2, R6, R16, R18, R21, R23). A T4 implementer left three self-matching `pgrep` watcher shells running; it killed them and later briefs banned watcher loops. Test runs that timed out were moved to the background. A T10 agent committed its git-ignored report; the controller dropped it from the commit. The README console example was first invented (the `expenses` program has no `start`, so `flowchart` rejects it) and was replaced with the real output of the README's Explain program.

**Deferred:** the step-through viewer (v0.1).

The milestone ledger (every ruling, review record and carry) is kept in `.superpowers/sdd/2026-11-27-compiler-m7-flowchart/progress.md`.
