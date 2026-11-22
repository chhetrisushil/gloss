# Gloss compiler M2: code generation and runtime - implementation plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task by task. Steps use checkbox (`- [ ]`) syntax for tracking. Ledger: `.superpowers/sdd/2026-11-23-compiler-m2-codegen/progress.md`.

**Goal:** `gloss build x.gloss` writes a working static x86-64 Linux executable `x` for every v0 program without tables, with byte-exact output and the run-time failures 1–5, 9 and 10.

**Architecture:** check → `lower` (AST + `TypeInfo` → Gloss IR) + `runtime` (routines written once in IR) → `target::x86_64` (IR → x86-64 instructions → bytes) → `target::elf` (static ELF). `irsim`, a test-only IR interpreter, checks the runtime against the Rust `number` module and runs every case on any host.

**Tech stack:** Rust 2024 edition, stable 1.87 or later, no crates. OrbStack Alpine machine `gloss-amd64` to run executables on the Mac.

**Spec:** `docs/specs/2026-11-22-compiler-design.md` (binding; section 4.6 and D17–D22 for this milestone). The language is defined by `spec/v0.md`: §4 (values, copies, numbers, conversions, showing), §7 (statements, `ask`), §10 (run-time failures and their messages), §12 (runtime). Conflicts about the language resolve against the language spec; conflicts about the implementation resolve against the design spec.

**How to read the tasks.**
- **Binding:** the tests in each task, with their exact strings; the types, names and signatures in each **Interfaces** block; the IR text form (Task 2); the memory layouts (Task 9).
- **Guidance:** implementation steps give the algorithm and the key code. The implementer writes the rest.
- A test may be adjusted only if it contradicts the language spec or the design spec. Record that as a ruling in the ledger.

## Global constraints

- [D1] Rust 2024 edition, `rust-version = "1.87"`.
- [D3] No `[dependencies]` and no `[dev-dependencies]`. A task that wants a crate stops and asks.
- [D4] One crate, `gloss`.
- [D18] Nothing outside `src/target/` names an x86-64 register, instruction or encoding. `lower`, `runtime` and `irsim` speak only IR.
- [D19] Every runtime routine is written in IR, except the backend's arithmetic helpers for `num_mul`, `num_div` and `num_rem` (Task 8b), which a backend may implement natively.
- [D20] Output: static ELF64, x86-64, `ET_EXEC`, no interpreter, no dynamic section; direct Linux system calls only.
- [D21] `irsim` is compiled only under `cfg(test)`. The real executable is the authority; both must agree on every `tests/run` case.
- [D22] A program that declares a table: `gloss build` prints `tables arrive in milestone 3` to standard error, exits 3, writes nothing.
- §10 run-time failure messages are binding, exactly as the language spec words them. They go to standard error and the program exits with status 2, after flushing standard output.
- Problem strings never end with a period (compile diagnostics, as in M1).

## Rules for every PR

- **One PR at a time** (owner directive). The next PR opens only after the previous one merges, branched from the new `main`. Never stacked.
- **Size.** At most 200 changed lines, tests and expected-output files included, and at most 10 files. Estimates run about 40% low, so every task below is estimated at 140 lines or fewer; tasks that would not fit are already split (a, b, c). If a task still grows past 200, split it into sequential PRs and record the split as a ruling.
- **Never** `#[rustfmt::skip]` or golfed code to fit the cap; split instead.
- **Branches:** `m2/tNN-<slug>` from `origin/main`.
- **Pull requests:** draft (`gh pr create --draft`); the owner marks them ready and merges with `gh pr merge <n> --rebase`.
- **Commits** end with:
  ```
  Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>
  Claude-Session: https://claude.ai/code/session_011uGukQR6EwQQdiSVNu4ERC
  ```
  PR bodies end with `🤖 Generated with [Claude Code](https://claude.com/claude-code)`, a blank line, and the session link.
- **Verify gate** before reporting a task done: `cargo fmt --check && cargo clippy --all-targets -- -D warnings && cargo test`. Every test passes with no warnings. On the Mac the run tests execute in `gloss-amd64`; the report states whether they ran or were skipped.
- **Reviews:** implementers and reviewers use `sonnet`. Tasks 5a–5c (encoder), 6 (ELF writer), 8a–8b (backend and Linux primitives) and 9 (memory and stack) get their **first** review on `opus`; every follow-up review uses `sonnet`. Re-check a one-line fix with the targeted test only.

## Resource limits

- At most 2 agents at once, reviews included. Only one may run `cargo` at a time.
- Every agent uses `CARGO_BUILD_JOBS=4`.
- Tests that write files use `std::env::temp_dir()` with a unique subfolder and remove it in a `Drop` guard.
- The Linux machine is shared: tests run executables there one at a time per test process; never start or stop the machine from a test.

## Findings that shape this plan

| # | Finding (verified 2026-11-23) | Consequence |
|---|---|---|
| F1 | OrbStack is installed; `gloss-amd64` is Alpine 3.23, x86-64, sees `/Users`. A hand-written 120-byte static ELF calling `exit(42)` ran there and exited 42. | ELF without sections or interpreter runs under Rosetta; the run harness can call `orb -m gloss-amd64 <path>` |
| F2 | `Number` has `parse_literal`, `parse_text`, `show`, `with_places` but no arithmetic | Task 1 adds the reference arithmetic that `irsim` and the runtime are checked against |
| F3 | `TypeInfo` has `expr_types` (by `ExprId`), `names` (what each `Name` refers to), per-feature `vars`, `loops` and `inputs` | `lower` needs no scope analysis of its own |
| F4 | `cli::build` prints `Checked …` and writes nothing; `write_atomically` exists for `format` | Task 10b reuses the atomic writer with mode 0755 |
| F5 | x86-64 `idiv` traps on `i64::MIN / -1` and on a quotient that does not fit | Task 8b's helpers handle both before dividing |
| F6 | On Rosetta, `getrlimit(RLIMIT_STACK)` works and the default soft limit is 8 MiB | Task 9's stack limit is testable in the machine |

## File map

| File | Responsibility | Task |
|---|---|---|
| `src/number.rs` | Reference arithmetic (`checked_add`, `mul_round`, …) | 1 |
| `src/ir/mod.rs`, `src/ir/text.rs` | IR types; text form | 2a, 2b |
| `src/irsim/mod.rs`, `src/irsim/num.rs` | Test-only interpreter | 3a, 3b |
| `src/target/mod.rs` | `Target` trait, `build_executable` | 4 |
| `src/target/x86_64/inst.rs` | x86-64 instruction type and Intel text | 4 |
| `src/target/x86_64/encode.rs`, `encode_alu.rs`, `encode_flow.rs` | Encoder | 5a, 5b, 5c |
| `src/target/elf.rs` | ELF writer | 6 |
| `scripts/objdump-check.sh`, `tests/objdump.rs` | Opt-in disassembly cross-check | 7 |
| `src/target/x86_64/lower.rs`, `arith.rs`, `linux.rs` | IR → x86-64; arithmetic helpers; Linux primitives | 8a, 8b |
| `src/runtime/mod.rs`, `core.rs`, `numbers.rs`, `text.rs`, `values.rs`, `fail.rs` | Runtime routines in IR | 9, 11, 17, 18, 19, 20–23 |
| `src/lower/mod.rs`, `expr.rs`, `stmt.rs`, `fail.rs` | AST → IR | 10a, 12–23 |
| `src/cli.rs`, `tests/cli.rs` | `gloss build` writes the executable | 10b |
| `tests/run.rs`, `tests/run_support/mod.rs`, `tests/run/*.gloss` (+ `.out`, `.in`, `.err`, `.status`) | Run harness and cases | 10b onward |
| `src/irsim/cases.rs` | Every `tests/run` case under `irsim` | 10b |
| `tests/spec_examples.rs` | Spec programs run; §10 messages checked | 24 |
| `README.md`, design spec "As built" | Docs | 25 |

## The run-case format (binding from Task 10b)

`tests/run/<name>.gloss` is a program. Beside it:
- `<name>.out`: exact expected standard output (required, may be empty).
- `<name>.in`: standard input (optional; empty input if absent).
- `<name>.err`: exact expected standard error (optional; empty if absent).
- `<name>.status`: expected exit status as a decimal number and a newline (optional; `0` if absent).

`tests/run.rs` builds each case with the real `gloss` binary (`env!("CARGO_BIN_EXE_gloss")`) into a temporary folder and runs the executable:
- with the command prefix from `GLOSS_RUNNER`, split on spaces, when it is set (empty means "run directly");
- else on macOS, with `orb -m gloss-amd64` when `orb` is on `PATH`;
- else directly when the host is x86-64 Linux;
- else it prints `run tests skipped: no x86-64 Linux runner (set GLOSS_RUNNER)` to standard error and checks only that every case builds.

A sweep guard asserts the number of cases is at least `MIN_CASES`, a constant raised by every task that adds cases.

---
## Task 1: Reference arithmetic on `Number`  (est. 130 lines / 1 file)

**Files:** Modify `src/number.rs` (functions and unit tests).

**Interfaces:**
- Produces, on `Number`:
  ```rust
  #[derive(Clone, Copy, Debug, PartialEq, Eq)]
  pub enum NumFail { DivideByZero, OutOfRange }
  pub fn checked_add(self, o: Number) -> Result<Number, NumFail>;
  pub fn checked_sub(self, o: Number) -> Result<Number, NumFail>;
  pub fn checked_neg(self) -> Result<Number, NumFail>;
  pub fn mul_round(self, o: Number) -> Result<Number, NumFail>;   // (a*b)/10000, 128-bit, round half away from zero
  pub fn div_round(self, o: Number) -> Result<Number, NumFail>;   // (a*10000)/b, 128-bit, round half away from zero
  pub fn rem(self, o: Number) -> Result<Number, NumFail>;         // exact; sign follows self; DivideByZero when o is 0
  pub fn whole(self) -> Number;                                    // drop the fraction toward zero
  ```
- Consumed by: `irsim` (Task 3b), the reference tests (Tasks 11, 12, 17, 18).

- [ ] **Step 1: Write the failing tests** (in `number.rs`'s test module). Values are written as ten-thousandths (`Number(35_000)` is 3.5):

| Call | Result |
|---|---|
| `0.1 + 0.2` (`Number(1000).checked_add(Number(2000))`) | `Ok(Number(3000))` |
| `MAX.checked_add(Number(1))` with `MAX = Number(i64::MAX)` | `Err(OutOfRange)` |
| `Number(i64::MIN).checked_neg()` | `Err(OutOfRange)` |
| `7 / 2` | `Ok(Number(35_000))` |
| `1 / 3` | `Ok(Number(3333))` |
| `2 / 3` | `Ok(Number(6667))` |
| `-2 / 3` | `Ok(Number(-6667))` (half away from zero, and 0.66666 rounds up in size) |
| `0.0001 * 0.5` (`Number(1).mul_round(Number(5000))`) | `Ok(Number(1))` (0.00005 rounds away from zero) |
| `-0.0001 * 0.5` | `Ok(Number(-1))` |
| `1 / 3 * 3` (`Number(3333).mul_round(Number(30_000))`) | `Ok(Number(9999))` |
| `MAX * 2` | `Err(OutOfRange)` |
| `Number(i64::MIN).div_round(Number(-10_000))` (MIN / -1) | `Err(OutOfRange)` |
| `x / 0` | `Err(DivideByZero)` |
| `remainder of 7 by 2` | `Ok(Number(10_000))` |
| `remainder of -7 by 2` | `Ok(Number(-10_000))` |
| `remainder of 7.5 by 2` | `Ok(Number(15_000))` |
| `Number(i64::MIN).rem(Number(-1))` | `Ok(Number(0))` (no trap) |
| `rem` by 0 | `Err(DivideByZero)` |
| `whole of 3.5`, `whole of -3.5` | `Number(30_000)`, `Number(-30_000)` |

Also a sweep: for 2000 pseudo-random pairs from a fixed-seed xorshift over `i64` (and over small values in `-10^8..10^8`), `mul_round` equals the i128 formula written independently in the test (`(a*b + sign*5000) / 10000`, range-checked).

- [ ] **Step 2: Run** `cargo test number::` — FAIL (functions missing).
- [ ] **Step 3: Implement** with `i128`: `mul_round`: `p = a*b`; `q = (|p| + 5000) / 10000` with the sign of `p`; out of `i64` range → `OutOfRange`. `div_round`: `b == 0` → `DivideByZero`; `n = a*10000`; `q = (2*|n| + |b|) / (2*|b|)` with sign `sign(a)*sign(b)`; range-check. `rem`: `b == 0` → `DivideByZero`; `(a as i128 % b as i128) as i64`. `whole`: `(a / 10000) * 10000` in `i64` (cannot overflow).
- [ ] **Step 4: Run the gate.** **Step 5: Commit** "Add reference arithmetic to Number".

**How this could pass vacuously:** tests that only check small positive values would accept truncation instead of rounding, and wrapping instead of range errors. The halves (`±0.00005`), the negative rounding row, `MIN / -1` and `MIN rem -1` rows fail such implementations.

---

## Task 2a: IR types  (est. 140 lines / 2 files)

**Files:** Create `src/ir/mod.rs`; modify `src/lib.rs` (`pub mod ir;`).

**Interfaces (binding):**
```rust
#[derive(Clone, Copy, Debug, PartialEq, Eq)] pub enum Reg { A, B }
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)] pub struct Label(pub u32);
#[derive(Clone, Copy, Debug, PartialEq, Eq)] pub enum Cond { Eq, Ne, Lt, Le, Gt, Ge }   // signed
#[derive(Clone, Copy, Debug, PartialEq, Eq)] pub enum IntOp { Add, Sub, Mul, Div, Rem, And, Or }  // a = a op b; Div/Rem signed, truncating
#[derive(Clone, Copy, Debug, PartialEq, Eq)] pub enum Shift { Left, Right, RightLogical }
#[derive(Clone, Copy, Debug, PartialEq, Eq)] pub enum Prim { Write, Read, Map, Exit, StackSize }
#[derive(Clone, Copy, Debug, PartialEq, Eq)] pub enum Global { HeapNext, HeapEnd, StackLimit, OutLen, OutBuf, InLen, InPos, InBuf }
#[derive(Clone, Copy, Debug, PartialEq, Eq)] pub enum Data { Text(u32), Desc(u32), Global(Global) }

#[derive(Clone, Debug, PartialEq)]
pub enum Inst {
    Line(u32),
    Const(Reg, i64),
    LoadSlot(Reg, u32), StoreSlot(u32, Reg),        // slots are numbered from 1
    LoadArg(Reg, u32),                               // arguments are numbered from 1, left to right
    Move { to: Reg, from: Reg },
    Push(Reg), Pop(Reg),
    AddressOf(Reg, Data),
    StackPointer(Reg),
    Load { to: Reg, base: Reg, offset: i32 },  Store { base: Reg, offset: i32, from: Reg },
    LoadByte { to: Reg, base: Reg, offset: i32 },  StoreByte { base: Reg, offset: i32, from: Reg },
    NumAdd { range: Label }, NumSub { range: Label }, NumNeg { range: Label },
    NumMul { range: Label }, NumDiv { zero: Label, range: Label }, NumRem { zero: Label }, NumWhole,
    Int(IntOp), AddImm(Reg, i32), Shift(Shift, u8),
    Compare,                                         // compares a with b
    JumpIf(Cond, Label), SetIf(Cond), Jump(Label), Mark(Label),
    Call(String), Return, Enter(u32),
    StackCheck { bytes: u32, fail: Label },
    System(Prim),
}

pub struct Function { pub name: String, pub params: u32, pub slots: u32, pub body: Vec<Inst> }

#[derive(Clone, Debug, PartialEq)]
pub enum Desc { Number, Text, Truth, Record { name: String, fields: Vec<(String, u32)> }, List(u32) }

pub struct Module {
    pub functions: Vec<Function>,   // the entry routine `_start` is among them
    pub texts: Vec<Vec<u8>>,        // read-only text constants, by index
    pub descs: Vec<Desc>,           // type descriptors, by index (`Record` fields and `List` name other descriptors)
}
impl Module {
    pub fn text(&mut self, bytes: &[u8]) -> u32;   // interns: the same bytes give the same index
    pub fn desc(&mut self, d: Desc) -> u32;        // interns likewise
}
```
**Semantics (binding, used by every backend and by `irsim`):**
- Numeric instructions work on `a` and `b` and leave the result in `a`: `NumAdd` is `a = a + b` with a range check (jump to `range` on overflow); `NumSub` `a = a - b`; `NumNeg` `a = -a`; `NumMul` `a = mul_round(a, b)`; `NumDiv` `a = div_round(a, b)` (zero → `zero`, out of range → `range`); `NumRem` `a = rem(a, b)`; `NumWhole` `a = whole(a)`. These mean exactly the `Number` methods of Task 1.
- `Int(op)`: `a = a op b`, wrapping. `AddImm(r, n)`: `r = r + n`. `Shift(s, n)`: `a = a << n`, arithmetic or logical `>> n`.
- `Compare` then `JumpIf(c, L)` jumps when `a c b`; `SetIf(c)` sets `a` to 1 or 0.
- Calls: the caller pushes the arguments left to right, `Call(name)`, then drops them with `AddImm`-free bookkeeping done by the backend (the callee's `params` says how many). The result is in `a`. `b` and the slots of the caller survive a call; `a` does not.
- `Enter(slots)` is the first instruction of every function; it reserves `slots` zeroed 8-byte slots. `Return` leaves the function with `a` as its result.
- `StackCheck { bytes, fail }` jumps to `fail` when calling a function whose frame needs `bytes` more bytes would pass `Global::StackLimit`.
- `System(p)`: the arguments were pushed left to right; `System` consumes them and leaves the result in `a`. `Write(fd, addr, len)` → bytes written or a negative error; `Read(fd, addr, len)` → bytes read, 0 at end of input, negative on error; `Map(len)` → address of `len` fresh zeroed bytes, or 0; `Exit(status)` does not return; `StackSize()` → the soft stack limit in bytes, or -1 when unlimited.
- `Line(n)` only marks where source line `n`'s code starts.

- [ ] **Step 1: Tests:** `Module::text` interns (`text(b"a")` twice → same index; `text(b"b")` → a new one); `desc` interns likewise.
- [ ] **Steps 2–5:** run (fail), implement, gate, commit "Add the Gloss IR types".

---

## Task 2b: IR text form  (est. 130 lines / 2 files)

**Files:** Create `src/ir/text.rs`; modify `src/ir/mod.rs`.

**Interfaces:** `pub fn function_text(f: &Function) -> String` and `pub fn module_text(m: &Module) -> String` (functions in order, a blank line between them).

**The text form (binding; lowering tests in later tasks compare against it):**

| Inst | Text |
|---|---|
| `Line(4)` | `line 4` |
| `Const(A, 42)` | `  a = 42` |
| `LoadSlot(A, 2)` / `StoreSlot(2, A)` | `  a = slot 2` / `  slot 2 = a` |
| `LoadArg(B, 1)` | `  b = arg 1` |
| `Move { to: B, from: A }` | `  b = a` |
| `Push(A)` / `Pop(B)` | `  push a` / `  pop b` |
| `AddressOf(A, Text(3))` / `Desc(2)` / `Global(OutBuf)` | `  a = address of text 3` / `desc 2` / `out_buf` |
| `StackPointer(A)` | `  a = stack pointer` |
| `Load { to: A, base: B, offset: 8 }` / `Store` | `  a = [b + 8]` / `  [b + 8] = a` (offset 0 prints `[b]`; negative prints `[b - 8]`) |
| `LoadByte` / `StoreByte` | `  a = byte [b + 8]` / `  byte [b + 8] = a` |
| `NumAdd { range: L3 }` | `  a = num_add a, b; out of range L3` |
| `NumSub`, `NumMul` | likewise `num_sub`, `num_mul` |
| `NumNeg { range: L3 }` | `  a = num_neg a; out of range L3` |
| `NumDiv { zero: L4, range: L3 }` | `  a = num_div a, b; zero L4; out of range L3` |
| `NumRem { zero: L4 }` | `  a = num_rem a, b; zero L4` |
| `NumWhole` | `  a = num_whole a` |
| `Int(Add)`…`Int(Or)` | `  a = a + b`, `- `, `* `, `div`, `mod`, `and`, `or` (`  a = a div b`) |
| `AddImm(A, -8)` | `  a = a - 8` (`+ 8` when positive) |
| `Shift(Left, 3)` / `Right` / `RightLogical` | `  a = a << 3` / `>> 3` / `>>> 3` |
| `Compare` | `  compare a, b` |
| `JumpIf(Lt, L3)` / `SetIf(Ge)` | `  jump if lt L3` / `  a = 1 if ge` |
| `Jump(L3)` / `Mark(L3)` | `  jump L3` / `L3:` |
| `Call("show_number")` / `Return` / `Enter(3)` | `  call show_number` / `  return` / `  enter 3` |
| `StackCheck { bytes: 48, fail: L5 }` | `  stack check 48; too deep L5` |
| `System(Write)` | `  system write` (`read`, `map`, `exit`, `stack_size`) |

A function prints as `function <name>, <params> params, <slots> slots` then its instructions one per line.

- [ ] **Step 1: Tests:** one function containing every row above prints exactly the expected multi-line string (write it out in the test).
- [ ] **Steps 2–5:** run, implement, gate, commit "Print the Gloss IR as text".

**How this could pass vacuously:** a test per row could skip rows. The single function holding every variant fails when any arm is missing, and `function_text` is an exhaustive `match` with no wildcard arm.

---

## Task 3a: `irsim` core  (est. 140 lines / 3 files)

**Files:** Create `src/irsim/mod.rs`; modify `src/lib.rs` (`#[cfg(test)] mod irsim;`).

**Interfaces:**
```rust
pub struct Outcome { pub stdout: Vec<u8>, pub stderr: Vec<u8>, pub status: i32 }
pub fn run(m: &Module, entry: &str, stdin: &[u8]) -> Outcome;
pub fn call(m: &Module, name: &str, args: &[i64]) -> i64;     // runs one function; for unit tests
```
- Memory: a sparse `HashMap<u64, u8>` (or paged `Vec`s). Read-only data and globals are laid out at fixed simulated addresses: texts from `0x1000_0000` (each as an 8-byte length then the bytes, 8-byte aligned), descriptors after them (Task 9's descriptor layout), globals from `0x2000_0000` in `Global` order (`OutBuf` and `InBuf` are 4096 bytes each, the others 8). The simulated stack starts at `0x7fff_0000_0000` and grows down; `StackSize` returns 8 MiB.
- Frames: a Rust `Vec` of frames is fine; `StackPointer` must still return a simulated address that falls by at least `8 * (params + slots + 2)` per call so `StackCheck` behaves as on hardware.
- 3a covers every instruction except the `Num*` family and `System`, which panic with `todo` until 3b.

- [ ] **Step 1: Tests (IR built by hand in the test):** a loop summing 1..10 with `Compare`/`JumpIf` gives 55 through `call`; a function calling another with two pushed arguments reads them with `LoadArg` 1 and 2 in left-to-right order; `Store`/`Load`/`StoreByte`/`LoadByte` round-trip at an address taken from `AddressOf(Global(OutBuf))`; `b` and slots survive a `Call`; `Shift(RightLogical, 1)` of -2 gives `i64::MAX`.
- [ ] **Steps 2–5:** run, implement, gate, commit "Interpret the Gloss IR in tests".

## Task 3b: `irsim` numbers and primitives  (est. 120 lines / 2 files)

**Files:** Create `src/irsim/num.rs`; modify `src/irsim/mod.rs`.

- `Num*` instructions call the Task 1 methods; `Err(OutOfRange)` jumps to `range`, `Err(DivideByZero)` to `zero`.
- `System`: `Write` to fd 1/2 appends to `stdout`/`stderr`; `Read` from fd 0 copies from `stdin`; `Map(n)` returns fresh zeroed simulated memory at increasing addresses from `0x4000_0000_0000`, or 0 when `n` exceeds 1 TiB; `Exit(s)` ends `run` with status `s`; `StackSize` returns 8 MiB.
- [ ] **Tests:** `NumDiv` by zero jumps to `zero`; `NumMul` of `i64::MAX` by 2 jumps to `range`; `Write` of 3 bytes to fd 2 lands in `stderr`; `Read` returns 0 at end of input; `Map(16)` returns non-zero and zeroed memory; `Exit(2)` stops with status 2.
- [ ] Commit "Run numbers and system primitives in irsim".

---

## Task 4: `Target` trait and x86-64 instructions  (est. 140 lines / 4 files)

**Files:** Create `src/target/mod.rs`, `src/target/x86_64/mod.rs`, `src/target/x86_64/inst.rs`; modify `src/lib.rs`.

**Interfaces:**
```rust
pub trait Target {
    fn name(&self) -> &'static str;                              // "x86-64 Linux"
    fn executable(&self, m: &crate::ir::Module) -> Result<Vec<u8>, String>;
}
pub fn x86_64_linux() -> Box<dyn Target>;                        // Task 8b makes it work end to end

// src/target/x86_64/inst.rs
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum R { Rax, Rcx, Rdx, Rbx, Rsp, Rbp, Rsi, Rdi, R8, R9, R10, R11, R12, R13, R14, R15 }
#[derive(Clone, Copy, Debug, PartialEq, Eq)] pub enum Cc { E, Ne, L, Le, G, Ge, B, Ae, O }
#[derive(Clone, Debug, PartialEq)]
pub enum Mem { Base(R, i32), Rip(Sym) }                 // [r + disp] or [rip + symbol]
#[derive(Clone, Debug, PartialEq)]
pub enum Sym { Text(u32), Desc(u32), Global(crate::ir::Global), Label(String) }
#[derive(Clone, Debug, PartialEq)]
pub enum X {
    MovRR(R, R), MovRI(R, i64), MovRM(R, Mem), MovMR(Mem, R), MovzxRM8(R, Mem), MovM8R(Mem, R),
    Lea(R, Mem), Push(R), Pop(R),
    Alu(AluOp, R, R), AluI(AluOp, R, i32),             // AluOp: Add, Sub, And, Or, Xor, Cmp
    Imul(R, R), ImulWide(R), Mul(R), Idiv(R), Div(R), Cqo, Neg(R), Test(R, R),
    Shl(R, u8), Sar(R, u8), Shr(R, u8),
    Setcc(Cc, R), Jcc(Cc, String), Jmp(String), Call(String), Ret, Syscall,
    Label(String), Comment(String),
}
pub fn text(x: &X) -> String;                             // Intel syntax
```
`text` prints lower-case Intel syntax with a comma and one space between operands: `mov rax, [rbp-16]`, `mov [rbp-16], rax`, `mov rax, 42`, `lea rax, [rip+text_3]`, `movzx eax, byte [rcx+8]`, `mov byte [rcx+8], al`, `add rax, rcx`, `sub rsp, 24`, `cmp rax, rcx`, `imul rax, rcx`, `imul rcx` (wide), `idiv rcx`, `div rcx`, `cqo`, `neg rax`, `shl rax, 3`, `setl al`, `jl L3`, `jmp L3`, `call show_number`, `ret`, `syscall`, `L3:`, `; line 4`. Symbols print as `text_3`, `desc_2`, the global's name in snake case (`out_buf`), or the label.

- [ ] **Tests:** each form above prints exactly as listed; `x86_64_linux().name()` is `"x86-64 Linux"`.
- [ ] Commit "Add the Target trait and x86-64 instruction text".

---

## Task 5a: Encoder: moves, memory, stack  (est. 140 lines / 2 files)

**Files:** Create `src/target/x86_64/encode.rs`.

**Interfaces:** `pub struct Encoded { pub bytes: Vec<u8>, pub labels: HashMap<String, usize>, pub fixups: Vec<Fixup> }`, `pub struct Fixup { pub at: usize, pub target: Sym, pub end: usize }` (a 32-bit field at `at`, relative to the end of its instruction `end`), and `pub fn encode(xs: &[X]) -> Result<Encoded, String>`. Fixups for labels inside `xs` are resolved in 5c; data symbols are resolved by the ELF writer (Task 6).

**Encoding rules (binding):** always the long forms, so every instruction has one fixed length whatever its target:
- `Mem::Base` always uses a 32-bit displacement (`mod = 10`), with a SIB byte when the base is `rsp` or `r12`; `Mem::Rip` uses `mod = 00, rm = 101` with a 32-bit fixup.
- `MovRI` uses `mov r64, imm64` (`REX.W B8+r`) always.
- Every 64-bit form carries `REX.W`; `REX.R`/`REX.B` for `r8`–`r15`. `MovM8R` with `rsi`/`rdi`/`rsp`/`rbp` as the byte source emits a REX prefix so the low byte (not `ah`–`bh`) is used.

- [ ] **Step 1: Golden-byte tests** (one or more per form). Exact bytes:

| Instruction | Bytes |
|---|---|
| `mov rax, rcx` | `48 89 c8` |
| `mov r12, rax` | `49 89 c4` |
| `mov rax, 42` | `48 b8 2a 00 00 00 00 00 00 00` |
| `mov rax, [rbp-16]` | `48 8b 85 f0 ff ff ff` |
| `mov [rbp-16], rax` | `48 89 85 f0 ff ff ff` |
| `mov rax, [rsp+8]` | `48 8b 84 24 08 00 00 00` |
| `mov rax, [r12+0]` | `49 8b 84 24 00 00 00 00` |
| `mov rcx, [r13+8]` | `49 8b 8d 08 00 00 00` |
| `movzx eax, byte [rcx+8]` | `0f b6 81 08 00 00 00` |
| `mov byte [rcx+8], al` | `88 81 08 00 00 00` |
| `mov byte [rax+0], sil` | `40 88 b0 00 00 00 00` |
| `lea rax, [rip+X]` | `48 8d 05` + 4-byte fixup (end = 7) |
| `push rbp` / `pop rbp` / `push r12` | `55` / `5d` / `41 54` |

- [ ] **Steps 2–5:** run, implement, gate, commit "Encode x86-64 moves and memory access".

## Task 5b: Encoder: arithmetic  (est. 130 lines / 2 files)

**Files:** Create `src/target/x86_64/encode_alu.rs`; modify `encode.rs`.

| Instruction | Bytes |
|---|---|
| `add rax, rcx` | `48 01 c8` |
| `sub rax, rcx` | `48 29 c8` |
| `cmp rax, rcx` | `48 39 c8` |
| `and rax, rcx` / `or rax, rcx` / `xor rdx, rdx` | `48 21 c8` / `48 09 c8` / `48 31 d2` |
| `sub rsp, 24` (`AluI`, always imm32) | `48 81 ec 18 00 00 00` |
| `add rax, -8` | `48 81 c0 f8 ff ff ff` |
| `imul rax, rcx` | `48 0f af c1` |
| `imul rcx` (wide) / `mul rcx` | `48 f7 e9` / `48 f7 e1` |
| `idiv rcx` / `div rcx` | `48 f7 f9` / `48 f7 f1` |
| `cqo` / `neg rax` / `test rax, rax` | `48 99` / `48 f7 d8` / `48 85 c0` |
| `shl rax, 3` / `sar rax, 3` / `shr rax, 3` | `48 c1 e0 03` / `48 c1 f8 03` / `48 c1 e8 03` |
| `setl al` then (the backend emits) `movzx eax, al` | `0f 9c c0` / `0f b6 c0` |
| `syscall` / `ret` | `0f 05` / `c3` |

- [ ] Commit "Encode x86-64 arithmetic".

## Task 5c: Encoder: labels, jumps, calls  (est. 110 lines / 2 files)

**Files:** Create `src/target/x86_64/encode_flow.rs`; modify `encode.rs`.

- `jcc` always `0f 8x rel32`, `jmp` `e9 rel32`, `call` `e8 rel32`. `Label` emits no bytes and records its offset; `Comment` emits nothing. After the pass, every fixup whose target is a label in `xs` is patched; an unknown label is `Err("unknown label <name>")`; a label defined twice is `Err("label <name> defined twice")`.

| Case | Bytes |
|---|---|
| `L: jmp L` | `e9 fb ff ff ff` |
| `jmp L` then `L:` | `e9 00 00 00 00` |
| `jo L` then `ret` then `L:` | `0f 80 01 00 00 00 c3` |
| `call f` with `f` 10 bytes later | `e8 05 00 00 00` + 5 bytes … |
| `jl`, `jge`, `jb`, `jae`, `je`, `jne`, `jle`, `jg` | second byte `8c`, `8d`, `82`, `83`, `84`, `85`, `8e`, `8f` |

- [ ] Commit "Resolve labels, jumps and calls in the encoder".

**How 5a–5c could pass vacuously:** golden tables written from the same wrong understanding as the encoder. Task 7's `objdump` cross-check catches that; until then, every row above was taken from the Intel manual's encoding tables, and the `rsp`/`r12`/`r13` and `sil` rows are the classic traps.

---
## Task 6: ELF writer  (est. 140 lines / 3 files)

**Files:** Create `src/target/elf.rs`; create `tests/run_support/mod.rs` (the runner lookup from "The run-case format", reused by Task 10b) and `tests/elf_runs.rs`.

**Interfaces:**
```rust
pub struct Image { pub code: Vec<u8>, pub entry: usize, pub fixups: Vec<Fixup>,
                   pub rodata: Vec<u8>, pub symbols: HashMap<Sym, usize>,   // offsets into rodata (texts, descs)
                   pub data_size: usize, pub globals: HashMap<Global, usize> } // offsets into the zeroed read-write segment
pub const CODE_BASE: u64 = 0x40_0000;      // file offset 0; the code follows the headers
pub const RODATA_BASE: u64 = 0x1000_0000;
pub const DATA_BASE: u64 = 0x2000_0000;
pub fn write(img: &Image) -> Result<Vec<u8>, String>;
```
**Layout (binding):** ELF64 little-endian, `ET_EXEC`, `EM_X86_64`, no section headers (`e_shnum = 0`). Program headers, in order:
1. `PT_LOAD` offset 0, vaddr `CODE_BASE`, flags R+X, holding the ELF header, the program headers and the code (`e_entry = CODE_BASE + header size + entry`).
2. `PT_LOAD` for `rodata` at the next 4096-aligned file offset, vaddr `RODATA_BASE`, flags R.
3. `PT_LOAD` vaddr `DATA_BASE`, `p_filesz = 0`, `p_memsz = data_size`, flags R+W.
4. `PT_GNU_STACK`, flags R+W (not executable).
All `p_align = 0x1000`. Data fixups are patched as `target address - (code address of fixup end)`; a displacement that does not fit in `i32` is an `Err`.

- [ ] **Step 1: Tests.** Unit: the header fields above (read them back from the bytes); a fixup to a rodata symbol is patched with the right displacement. Integration (`tests/elf_runs.rs`): an image whose code is `mov eax, 60` (`48 b8 3c …`), `mov rdi, [rip+answer]` (a rodata symbol holding 42), `syscall` builds a file that the runner executes with exit status 42; skipped with the runner's skip message when no runner exists.
- [ ] Commit "Write static ELF executables".

**How this could pass vacuously:** a header test that reads back what it wrote passes with wrong constants. The integration test fails unless Linux really loads all three segments: the exit status comes from read-only data through a RIP-relative load.

---

## Task 7: `objdump` cross-check  (est. 90 lines / 2 files)

**Files:** Create `scripts/objdump-check.sh` and `tests/objdump.rs`.

- The test is skipped unless `GLOSS_OBJDUMP=1`. It encodes one instance of every `X` variant (every register class: `rax`, `rsp`, `rbp`, `r12`, `r13`, `rsi` for byte stores), writes the raw bytes to a temporary file, runs `scripts/objdump-check.sh <file>` (which runs `objdump -D -b binary -m i386:x86-64 -M intel` through the runner) and compares each disassembled line, after normalising spaces and `QWORD PTR`/`BYTE PTR`, with `text(x)`.
- One-time machine setup is documented in the script's header: `orb -m gloss-amd64 sudo apk add binutils` (already done on the owner's machine, 2026-11-23).
- [ ] **Tests:** run once with `GLOSS_OBJDUMP=1` and paste the result line into the report; the gate runs it skipped.
- [ ] Commit "Cross-check the encoder against objdump".

---

## Task 8a: x86-64 backend: moves, memory, control  (est. 140 lines / 2 files)

**Files:** Create `src/target/x86_64/lower.rs`.

**Interfaces:** `pub fn lower_function(f: &ir::Function) -> Vec<X>`. Register and frame mapping (binding): `a`→`rax`, `b`→`rcx`; slot k→`[rbp-8k]`; argument k of n→`[rbp+16+8(n-k)]`; scratch `rdx`, `rsi`, `rdi`, `r8`–`r11` only inside one IR instruction's expansion.
- `Enter(n)`: `push rbp; mov rbp, rsp; sub rsp, 8n` and zero each slot. `Return`: `mov rsp, rbp; pop rbp; ret`.
- `Call(f)` with `params = p`: `call f; add rsp, 8p` (the backend looks up `p` from the module).
- `Compare` + `JumpIf`: `cmp rax, rcx; j<cc>`. `SetIf`: `cmp rax, rcx; set<cc> al; movzx eax, al`.
- `StackCheck { bytes, fail }`: `lea rdx, [rsp - bytes]; cmp rdx, [rip+stack_limit]; jb fail`.
- `Line(n)` becomes `Comment("line n")`. Labels become `L<function>_<n>`, so labels never clash across functions.
- [ ] **Tests:** the x86 text of the IR for `Enter(2)`, a slot load and store, a `Call` of a two-parameter function, a `JumpIf(Le, …)` and a `StackCheck`, each exactly; and through `irsim` and the encoder: no `X` produced for these instructions refers to a register outside the mapping.
- [ ] Commit "Lower Gloss IR moves, memory and control to x86-64".

## Task 8b: x86-64 backend: numbers, Linux primitives, end to end  (est. 140 lines / 3 files)

**Files:** Create `src/target/x86_64/arith.rs` and `src/target/x86_64/linux.rs`; modify `lower.rs`, `src/target/mod.rs`.

- `NumAdd`/`NumSub`: `add`/`sub rax, rcx; jo range`. `NumNeg`: `neg rax; jo range`. `NumWhole`: `cqo; mov rsi, 10000; idiv rsi; imul rax, rsi`.
- `NumMul`, `NumDiv`, `NumRem` call backend helpers `multiply_number`, `divide_number`, `remainder_number` written in `X` in `arith.rs` (allowed by D19). They take `rax`, `rcx`, return in `rax`, and signal failures by returning with a status in `rdx` (0 ok, 1 range, 2 zero) that the call site tests to jump to `range`/`zero`. They implement Task 1 exactly: 128-bit product with `imul rcx`; rounding by taking the magnitude, adding 5000 across the 128-bit pair, dividing the high half first so the quotient never traps (F5); `MIN / -1` and `MIN rem -1` handled before `idiv`.
- Linux primitives (`linux.rs`): `Write` = syscall 1, `Read` = 0, `Map` = 9 (`mmap(0, len, PROT_READ|PROT_WRITE, MAP_PRIVATE|MAP_ANONYMOUS, -1, 0)`, returning 0 for any error), `Exit` = 231 (`exit_group`), `StackSize` = 97 (`getrlimit(RLIMIT_STACK)` into a stack buffer; `RLIM_INFINITY` → -1). Arguments are popped from the stack into `rdi`, `rsi`, `rdx` (and `r10`, `r8`, `r9` for `mmap`); `rcx` and `r11` are saved around `syscall` because the kernel clobbers them.
- `x86_64_linux().executable(m)`: lowers every function, adds the helpers, encodes, builds the `Image` (texts and descriptors to rodata; `Global`s to the data segment with `OutBuf`/`InBuf` 4096 bytes), writes the ELF. The entry is `_start`.
- [ ] **Tests:** for 300 fixed-seed operand pairs including range ends, halves and zero, a tiny module whose `_start` computes `num_mul`, `num_div` and `num_rem` and exits with a status derived from the result (or writes the 8 result bytes to stdout) gives exactly `Number::mul_round`/`div_round`/`rem`: run in the machine (skipped without a runner). A module whose `_start` writes `hi\n` and exits 0 prints `hi\n`.
- [ ] Commit "Lower numbers and Linux system calls; build executables".

---

## Task 9: Runtime core in IR  (est. 140 lines / 3 files)

**Files:** Create `src/runtime/mod.rs`, `src/runtime/core.rs`; modify `src/lib.rs`.

**Interfaces:**
```rust
pub fn routines() -> Vec<ir::Function>;                     // every runtime routine
pub fn add_reachable(m: &mut ir::Module, called: &[String]);   // adds the routines reachable from these names, once each
```
**Memory layouts (binding, used by every later task, by `irsim` and by Explain):**
- Text: 8-byte length, then the bytes. A text value is the address of the length.
- List: header block of three 8-byte fields `[count, capacity, items]`; `items` is the address of `capacity` 8-byte slots.
- Record: one 8-byte field per declared field, in declaration order.
- Descriptor (read-only): 8-byte kind (`0` number, `1` text, `2` truth, `3` record, `4` list); a record continues with the address of its name text, its field count, then per field the address of the field-name text and of the field's descriptor; a list continues with the address of its item descriptor.

**Routines (names binding):**
- `_start`: `heap_init`, `stack_init`, `call start`, `flush`, `exit 0`.
- `alloc(bytes, line) -> address`: 8-byte aligned bump from `heap_next`; when `heap_end` is passed, `map` a new piece of `max(64 MiB, bytes)`; a `map` result of 0 calls `fail_memory(line)`. Every caller passes the source line it is lowering, so the failure message can name it without a global.
- `stack_init`: `limit = stack pointer - min(size, 1 GiB) + 64 KiB`, with `size = 8 MiB` when `stack_size` is -1.
- `out_bytes(addr, len)`, `out_text(text)`: append to `out_buf`, flushing when full (a write longer than the buffer goes straight out after a flush).
- `flush`: writes `out_buf[0..out_len]` to fd 1, looping on short writes; a negative result ends the program with `exit 2`.
- `err_text(text)`: writes straight to fd 2 (no buffer).
- `fail_begin(line, code)`: `flush`, then writes `line <line>: failure <code>: ` to fd 2. `err_int(n)` writes a plain integer in decimal to fd 2 (line numbers and counts; `fail_begin` uses it). `fail_end()`: writes `\n` to fd 2 and exits with status 2. Every failure message is built from these plus `err_text` and Task 11's `err_number`.
- `fail_memory(line)`: `fail_begin(line, 10)`, `err_text("out of memory")`, `fail_end`.
- [ ] **Tests (irsim):** `alloc` returns 8-byte aligned, increasing, non-overlapping blocks across a piece boundary (allocate 64 MiB then 16 bytes); with `irsim`'s `Map` made to fail (a 2 TiB request), the outcome is status 2 and stderr `line 7: failure 10: out of memory\n`; `out_text` of 5000 bytes then `flush` produces exactly those bytes; a module whose `start` shows nothing exits 0 with empty output; `stack_init` with `stack_size` -1 sets the limit 8 MiB − 64 KiB below the stack pointer.
- [ ] Commit "Write the runtime core in Gloss IR".

**How this could pass vacuously:** `alloc` tests that allocate only small blocks never cross a piece boundary; the 64 MiB allocation forces a second `map`.

---

## Task 10a: Lowering skeleton: features, slots, `show` of text  (est. 140 lines / 3 files)

**Files:** Create `src/lower/mod.rs`, `src/lower/stmt.rs`; modify `src/lib.rs`.

**Interfaces:**
```rust
pub fn lower(program: &Program, types: &TypeInfo, src: &Source) -> Result<ir::Module, Diagnostic>;
```
- Each feature becomes `Function { name: "f_<feature>", params: inputs.len(), slots: vars + loop vars + temporaries, body }` (prefix `f_` so a feature can never clash with a runtime routine name); `_start` calls `f_start`.
- Slot assignment: inputs are read with `LoadArg`; each variable and loop variable from `FeatureInfo.vars`/`loops` gets a slot, in that order, numbered from 1. Name lookups use `TypeInfo.names`.
- `Line(n)` before each statement. `show` with text literals: `out_text(address of text k)` per item, then `out_text` of the newline text.
- A program with a `table` returns `Err` with the program-level diagnostic `tables arrive in milestone 3` (D22).
- [ ] **Tests:** the IR text of `feature start\n  show "hi"\n` is exactly:
  ```
  function f_start, 0 params, 0 slots
    enter 0
  line 2
    a = address of text 0
    push a
    call out_text
    a = address of text 1
    push a
    call out_text
    return
  ```
  (text 0 is `hi`, text 1 is `\n`); a program with a table gives `tables arrive in milestone 3`.
- [ ] Commit "Lower features and show of text to Gloss IR".

## Task 10b: `gloss build` writes executables; the run harness  (est. 140 lines / 8 files)

**Files:** Modify `src/cli.rs`, `tests/cli.rs`, `src/lib.rs` (`pub fn build_executable(text: &str) -> Result<Vec<u8>, Vec<Diagnostic>>`); create `tests/run.rs`, `src/irsim/cases.rs`, `tests/run/hello.gloss` + `.out`, `tests/run/two_lines.gloss` + `.out`.

- `gloss build x.gloss`: after a clean check, `lower`, `x86_64_linux().executable`, write `x` beside the source atomically with mode `0o755` (extend `write_atomically` to take the mode; a new file has no original permissions), print `Built x`. The old `Checked …` line is gone. Tables: `tables arrive in milestone 3.`, exit 3 (rendered as a program-level diagnostic).
- `tests/run.rs` implements "The run-case format" with `MIN_CASES = 2`. `src/irsim/cases.rs` runs the same cases through `lower` + `irsim::run` and compares the same files.
- Cases: `hello.gloss` = `feature start\n  show "Hello"\n` → `Hello\n`; `two_lines.gloss` shows `"a"` then `"b"` → `a\nb\n`.
- [ ] **Tests:** `tests/cli.rs`: build prints `Built hello`, the file exists with mode 0755 (unix), a table program exits 3 with `tables arrive in milestone 3.` and writes nothing; the old `build_checks_clean_program` expectation changes to `Built …` (sanctioned).
- [ ] Commit "Build real executables and run them in tests". **This is the first program that runs.**

---
## Task 11: Showing numbers  (est. 140 lines / 5 files)

**Files:** Create `src/runtime/numbers.rs`; modify `src/runtime/mod.rs`, `src/lower/stmt.rs`; create `src/lower/expr.rs`; add cases.

- Runtime: `number_text(n, line) -> text` builds the show form (Task 1's `Number::show`): sign, whole part digits, then `.` and the fraction with trailing zeros dropped. It must handle `i64::MIN` (no negation overflow: work on the magnitude as unsigned with `Shift(RightLogical)`-safe digit extraction, or peel one digit before negating). `out_number(n)` and `err_number(n)` write it.
- Lowering: `ExprKind::Number` → `Const(A, n.0)`; `show` of a number item → `push a; call out_number`.
- [ ] **Tests:** reference test in `irsim`: for 5000 fixed-seed values (all of `i64::MIN`, `i64::MAX`, 0, ±1, ±9999, ±10000, ±10001, and random), `number_text` equals `Number::show`. Run cases: `numbers.gloss`:
  ```gloss
  feature start
    show 120.50
    show 120.0
    show -7.5
    show 0.0001
    show 922337203685477.5807
    show -922337203685477.5808
  ```
  → `120.5\n120\n-7.5\n0.0001\n922337203685477.5807\n-922337203685477.5808\n`. `MIN_CASES = 3`.
- [ ] Commit "Show numbers".

## Task 12a: Number expressions and failure 4  (est. 140 lines / 5 files)

**Files:** Modify `src/lower/expr.rs`; create `src/lower/fail.rs`; add cases.

- Variables: `set v to <number expr>` stores into the slot; a `Name` loads it (`TypeInfo.names`: `Variable`/`LoopVariable` → slot, `Input` → `LoadArg`, `FeatureCall` → Task 15).
- `Binary`: evaluate left → `push a`; evaluate right → `b = a; pop a`; then `NumAdd`/`NumSub`/`NumMul`/`NumDiv`. `Negate` → `NumNeg`. `Paren` → its inner expression. `Fact::Whole` → `NumWhole`. `Remainder` → `NumRem`.
- Failure blocks (`src/lower/fail.rs`) are emitted once per failing instruction at the end of the function, after a `Mark`. Failure 4: `fail_begin(line, 4)`, `err_text("a number went out of range (")`, `err_text(<expression source>)`, `err_text(")")`, `fail_end`. The expression source is `ast::print::expr` of the expression that failed (for a statement like `add`, Task 12b).
- [ ] **Tests:** the IR text of `feature start\n  set t to 1 + 2\n  show t\n` is exactly (texts: 0 `\n`, 1 `a number went out of range (`, 2 `1 + 2`, 3 `)`):
  ```
  function f_start, 0 params, 1 slots
    enter 1
  line 2
    a = 10000
    push a
    a = 20000
    b = a
    pop a
    a = num_add a, b; out of range L0
    slot 1 = a
  line 3
    a = slot 1
    push a
    call out_number
    a = address of text 0
    push a
    call out_text
    return
  L0:
    a = 2
    push a
    a = 4
    push a
    call fail_begin
    a = address of text 1
    push a
    call err_text
    a = address of text 2
    push a
    call err_text
    a = address of text 3
    push a
    call err_text
    call fail_end
  ```
  Run cases:
  - `arith.gloss`: `show 0.1 + 0.2`, `show 7 / 2`, `show 1 / 3 * 3`, `show -(2 / 3)`, `show whole of -3.5`, `show remainder of -7 by 2`, `show remainder of 7.5 by 2` → `0.3\n3.5\n0.9999\n-0.6667\n-3\n-1\n1.5\n`.
  - `range.gloss`: `set price to 922337203685477` then `show price * 1000000` → stdout empty, `.err` = `line 3: failure 4: a number went out of range (price * 1000000)\n`, `.status` = `2`.
- [ ] Commit "Compute number expressions; report failure 4".

## Task 12b: Number statements and failure 1  (est. 130 lines / 4 files)

- `add x to t`, `subtract x from t`, `multiply t by x`, `divide t by x` on variable targets (field and item targets arrive with records and lists). Failure 4's parenthesis for these statements is the whole statement as written (`ast` printing of the statement via `format::print::stmt` at level 0).
- Failure 1: `fail_begin(line, 1)`, `err_text("dividing by zero (")`, the divisor's source, `err_text(" is 0)")`, `fail_end`. The divisor is `b` of `a / b`, `by` of `divide … by` and of `remainder of … by`.
- [ ] **Tests:** run cases:
  - `verbs.gloss`: `set t to 10` / `add 5 to t` / `subtract 2.5 from t` / `multiply t by 3` / `divide t by 2` / `show t` → `18.75\n`.
  - `divide_zero.gloss` (the spec's example):
    ```gloss
    feature start
      set total to 120
      set people to 0
      show "Before"
      divide total by people
    ```
    → stdout `Before\n`, `.err` `line 5: failure 1: dividing by zero (people is 0)\n`, status 2. This also proves standard output is flushed before the message.
  - `rem_zero.gloss`: `show remainder of 7 by (1 - 1)` → `line 2: failure 1: dividing by zero ((1 - 1) is 0)\n`.
  - `add_range.gloss`: `set t to 922337203685477` / `add 922337203685477 to t` → `line 3: failure 4: a number went out of range (add 922337203685477 to t)\n`.
- [ ] Commit "Lower number statements; report failure 1".

## Task 12c: The number reference test on real executables  (est. 100 lines / 2 files)

**Files:** Create `tests/number_reference.rs`.

- Generates one Gloss program that shows 2000 results of `+`, `-`, `*`, `/`, `remainder of` and `whole of` over fixed-seed operands chosen inside the range (results that would fail are skipped by computing them with `Number` first), builds it with `gloss`, runs it with the runner, and compares every line with `Number` computed in the test. Skipped (with the runner's message) when there is no runner.
- [ ] Commit "Check number results of real executables against Number".

**How Tasks 11–12 could pass vacuously:** cases with only "nice" values. The reference tests cover halves, signs and range ends on both `irsim` and the real executable.

---

## Task 13: Truths, comparisons, `if` and `repeat while`  (est. 140 lines / 4 files)

- `ExprKind::Truth` → 1/0; `Compare` of numbers → `Compare` + `SetIf`; of truths `is`/`is not` the same; `Not` → `a = 1 - a` (via `b`); `And`/`Or` short-circuit left to right; `show` of a truth → `out_text("yes"/"no")`.
- `If` with branches and `otherwise`; `RepeatWhile` (the condition is evaluated before each turn).
- [ ] **Tests:** run case `branches.gloss`:
  ```gloss
  feature start
    set price to 90
    if price is greater than 100
      show "dear"
    otherwise if price is less than 80
      show "cheap"
    otherwise
      show "fair"
    set n to 1
    repeat while n is at most 3
      show n
      add 1 to n
    show (2 is 2 and not (3 is less than 1)), (1 is 2 or no)
  ```
  → `fair\n1\n2\n3\nyesno\n`. IR text test: `and` jumps past its right side when the left is `no` (check the `jump if eq` on 0).
- [ ] Commit "Lower truths, if and repeat while".

## Task 14: Counting loops, `skip`, `stop`, failure 3 (for the end)  (est. 130 lines / 3 files)

- `for each i from s to e`: both ends worked out once into hidden slots before the first turn; `e` (and `s`) must be whole: `Compare` of the value with its `NumWhole`, else failure 3: `fail_begin(line, 3)`, `err_text("the end of a count must be whole (")`, source of the end expression, `err_text(" is ")`, `err_number(value)`, `err_text(")")`, `fail_end`. A start that is not whole uses `the start of a count must be whole (…)` (plan ruling P1; add that wording to spec §10's failure 3 row in this PR).
- `skip` jumps to the increment of the innermost loop; `stop` to its exit. `repeat while` loops support both.
- [ ] **Tests:** run cases `count.gloss`: `for each i from 1 to 5` with `if i is 2` → `skip`, `if i is 4` → `stop`, `show i` → `1\n3\n`; `count_empty.gloss`: from 3 to 1 shows nothing then `show "done"` → `done\n`; `count_frac.gloss`: `set top to 2.5` / `for each i from 1 to top` → `line 3: failure 3: the end of a count must be whole (top is 2.5)\n`, status 2.
- [ ] Commit "Lower counting loops, skip and stop".

---

## Task 15: Features: inputs, `give`, `run`, recursion, failure 10  (est. 140 lines / 4 files)

- A call (`ExprKind::Call`, `Run`, or a `Name` with `FeatureCall`): evaluate inputs left to right, `push a` each; `StackCheck { bytes: 8 * (callee slots + 2) + 64, fail }`; `Call("f_<name>")`. `give e` → evaluate, `Return`. A feature with no `give` returns at its end. A feature that gives must give on every path, which the checker guarantees. Still, the end of such a feature calls `fail_internal`, which prints `internal error: feature <name> ended without giving a value` and exits 2 (plan ruling P2), so a checker bug can never return garbage.
- Failure 10 (stack): `fail_begin(line, 10)`, `err_text("calls nested too deep (")`, the callee's name, `err_text(")")`, `fail_end`.
- [ ] **Tests:** run cases:
  - `features.gloss`: the spec's §9 program (`is_even`, `count_even`, `report`, `start`) → `Even numbers: 2\n`.
  - `recursion.gloss`:
    ```gloss
    feature countdown
      needs n as number
      if n is 0
        give 0
      give countdown with n - 1

    feature start
      show (countdown with 1000)
    ```
    → `0\n`.
  - `too_deep.gloss` (the base case never matches):
    ```gloss
    feature deep
      needs n as number
      if n is -1
        give 0
      give (deep with n + 1) + 1

    feature start
      show (deep with 1)
    ```
    → stdout empty, `.err` `line 5: failure 10: calls nested too deep (deep)\n`, status 2 (the line of the call that ran out of room).
  - `no_input_call.gloss`: `feature answer` / `give 42` and `show answer` → `42\n`.
- [ ] Commit "Lower feature calls, give, run and recursion".

---

## Task 16: Text equality and `as text`  (est. 140 lines / 4 files)

- Runtime `text_equal(x, y) -> 1/0` (byte for byte); `Compare` of texts with `is`/`is not` calls it. `as text` of a number → `number_text`; of a truth → the `yes`/`no` texts. `show` of any text value → `out_text`.
- Runtime `number_places(n, places, line) -> text` implementing `Number::with_places`.
- [ ] **Tests:** `irsim` reference: 3000 values × places 0–4 equal `Number::with_places`, including `-0.00001`-style values that round to zero (whatever Task 1's `with_places` prints, the runtime prints). Run case `text.gloss`:
  ```gloss
  feature start
    set a to "Food"
    show (a is "food"), (a is not "food"), (a is "Food")
    show 120.5 as text with 2 places, 7 as text with 0 places, 2.5 as text with 0 places
    show (1 is 1) as text, 3.25 as text
  ```
  → `noyesyes\n120.5073\nyes3.25\n` (`7` with 0 places is `7`, `2.5` with 0 places rounds half away to `3`).
- [ ] Commit "Compare text and convert values to text".

## Task 17: `as number`, `is number`, failure 5  (est. 140 lines / 4 files)

- Runtime `parse_number(text) -> (status, value)` implementing `Number::parse_text` exactly (optional `-`, digits, optional `.` and 1–4 digits, spaces at the ends ignored, range checked); `is number`/`is not number` → status; `as number` → value, or failure 5.
- Failure 5: `fail_begin(line, 5)`, `err_text("\"")`, the text cut to its first 40 characters (characters, not bytes: never split a UTF-8 sequence) with `…` when cut and invisible characters written `\u{XXXX}` (the same set as compile-error echoes; reuse M1's classification by generating a lookup table into read-only data, or by porting the ranges to IR), `err_text("\" is not a number (")`, the expression source, `err_text(")")`, `fail_end`.
- [ ] **Tests:** `irsim` reference: 2000 generated strings (valid, `+3`, `1,200`, `1e3`, `.5`, `0.12345`, spaces, `-`, empty, out of range) give the same status and value as `Number::parse_text`. Run cases: `to_number.gloss`: `show " -7 " as number + 1, ("0.5" is number), ("1e3" is number)` → `-6yesno\n`; `bad_number.gloss`: `set answer to "12a"` / `show answer as number` → `line 3: failure 5: "12a" is not a number (answer as number)\n`, status 2; `long_bad.gloss`: `set s to` a text of 50 `a` characters / `show s as number` → `line 3: failure 5: "` + 40 `a` characters + `…" is not a number (s as number)\n`.
- [ ] Commit "Convert text to numbers; report failure 5".

## Task 18: `ask` and failure 9  (est. 130 lines / 4 files)

- Runtime `ask(question, line) -> text`: `out_text(question)`, `out_text("\n")`, `flush`, then read through `in_buf` (refilling with `Read`) up to `\n`, building the line in an allocated text that grows by doubling; remove a final `\n` and a `\r` before it. End of input with nothing read → `fail_begin(line, 9)`, `err_text("input ended while ask was waiting")`, `fail_end`. End of input after some bytes → that text.
- [ ] **Tests:** run cases with `.in`: `ask.gloss`: `ask "Name?" into name` / `show "Hi ", name` twice with input `Asha\r\nRavi` (no final newline) → `Name?\nHi Asha\nName?\nHi Ravi\n`; `ask_end.gloss`: one `ask` with empty input → stdout `Q\n`, `.err` `line 2: failure 9: input ended while ask was waiting\n`, status 2; `ask_long.gloss`: `ask "Q" into s` / `show s` with input of 10000 `x` characters and a newline → `Q\n` then the 10000 characters and a newline.
- [ ] Commit "Read input with ask; report failure 9".

---
## Task 19: Records  (est. 140 lines / 5 files)

**Files:** Create `src/runtime/values.rs`; modify `src/lower/*`; add cases.

- Descriptors: `lower` interns a `Desc` for every record and list type it uses (`Module::desc`).
- `E with f1 v1, f2 v2`: `alloc(8 * fields, line)`, store each field at its declaration-order offset (fields may be written in any order). `f of r` → `Load` at the field's offset. `set f of r to v`, and `add`/`subtract`/`multiply`/`divide` on a field target, update it in place (the target's root is a variable or a loop variable; inputs were rejected by the checker).
- Runtime `show_value(value, desc)` for records: `E with name "Lunch", amount 120, category "food"`; text fields quoted, with `"` doubled.
- [ ] **Tests:** run case `records.gloss`:
  ```gloss
  record Expense
    name as text
    amount as number
    paid as truth

  feature start
    set lunch to Expense with amount 120, name "Say ""hi""", paid no
    add 30 to amount of lunch
    show amount of lunch
    show lunch
  ```
  → `150\nExpense with name "Say ""hi""", amount 150, paid no\n`.
- [ ] Commit "Build, read, change and show records".

## Task 20a: Lists: building, reading, failure 2 and 3 (positions)  (est. 140 lines / 4 files)

- `list of a, b, c` → `alloc` a header and `max(4, n)` item slots; `empty list of T` → header with count 0, capacity 4. `count of xs`, `first of xs`, `last of xs`, `item p of xs`.
- Positions: `p` must be whole (failure 3: `an item position must be whole (<p source> is <p>)`) and within 1..count (failure 2: `there is no item <p> in <list source>; it has <count> items`, with `1 item` for a count of 1, and for an empty list `it has 0 items`). `first`/`last` of an empty list: `there is no first item in <list source>; it is empty` (`last` likewise).
- [ ] **Tests:** run cases `lists.gloss`: `set xs to list of 4, 9, 2` / `show count of xs, " ", first of xs, " ", last of xs, " ", item 2 of xs` → `3 4 2 9\n`; `no_item.gloss`: `set prices to list of 1, 2, 3` / `show item 7 of prices` → `line 3: failure 2: there is no item 7 in prices; it has 3 items\n`; `one_item.gloss`: `set xs to list of 5` / `show item 2 of xs` → `line 3: failure 2: there is no item 2 in xs; it has 1 item\n`; `empty_first.gloss`: `set xs to empty list of number` / `show first of xs` → `line 3: failure 2: there is no first item in xs; it is empty\n`; `frac_item.gloss`: `set xs to list of 1, 2` / `set n to 2.5` / `show item n of xs` → `line 4: failure 3: an item position must be whole (n is 2.5)\n`.
- [ ] Commit "Build and read lists; report failures 2 and 3".

## Task 20b: Lists: `append`, `set item`, `for each … in`  (est. 140 lines / 4 files)

- `append v to xs`: when `count == capacity`, `alloc` twice the capacity, copy the items, store the new `items` address in the same header (the header never moves, so a loop variable keeps pointing at the right item). `set item p of xs to v` and the arithmetic verbs on an item target, with the Task 20a position checks.
- `for each x in xs`: a hidden slot holds the position; the loop variable holds the item value (for records, the record's address, so changing its fields changes the list's record, as the spec says). `skip`/`stop` work as in Task 14.
- [ ] **Tests:** run case `grow.gloss`: start from `empty list of number`, append 1..100 in a counting loop, then sum with `for each` and show the sum and the count → `5050100\n` (`show total, count of xs`); `loop_alias.gloss`: a list of two `Expense` records, `for each e in es` / `set amount of e to 0`, then `show amount of item 1 of es, amount of item 2 of es` → `00\n` (list show arrives in Task 21).
- [ ] Commit "Append to lists, set items and loop over lists".

## Task 21: Showing and comparing lists and records  (est. 140 lines / 3 files)

- `show_value` for lists: `list of 4, 9, 2`; text items quoted; record items parenthesised: `list of (E with …), (E with …)`; empty lists `empty list of number` / `empty list of Expense` (the descriptor gives the item type name).
- Runtime `equal_values(x, y, desc) -> 1/0`: numbers and truths by value, texts by `text_equal`, records field by field, lists by count then item by item. `is`/`is not` on lists and records call it.
- [ ] **Tests:** run case `show_values.gloss`:
  ```gloss
  record P
    name as text
    age as number

  feature start
    set xs to list of 4, 9, 2
    set ws to list of "a", "b"
    set ps to list of (P with name "Asha", age 9), (P with name "Ravi", age 7)
    set none to empty list of P
    show xs
    show ws
    show ps
    show none
    show (xs is list of 4, 9, 2), (xs is list of 4, 9), (item 1 of ps is P with name "Asha", age 9)
  ```
  → `list of 4, 9, 2\nlist of "a", "b"\nlist of (P with name "Asha", age 9), (P with name "Ravi", age 7)\nempty list of P\nyesnoyes\n`.
- [ ] Commit "Show and compare lists and records".

## Task 22: Copies  (est. 130 lines / 3 files)

- Runtime `copy_value(value, desc, line)`: records → a new block with the same fields; lists → a new header and items, with each record item copied too. Text is never copied.
- Lowering: copy when a list or record is stored by `set` (including `set item`), by `append`, as an item of `list of`, and when it is a feature's result — unless the stored expression is itself `list of`, `empty list of` or `E with` (or that inside parentheses). Inputs are pushed without copying.
- [ ] **Tests:** run case `copies.gloss`:
  ```gloss
  record P
    age as number

  feature same
    needs xs as list of number
    give xs

  feature start
    set a to list of 1, 2
    set b to a
    append 3 to b
    set c to same with a
    append 4 to c
    set r to P with age 1
    set ps to list of r
    set age of r to 2
    set q to item 1 of ps
    set age of q to 3
    show a, " ", b, " ", c, " ", ps
  ```
  → `list of 1, 2 list of 1, 2, 3 list of 1, 2, 4 list of (P with age 1)\n`. The spec's §8 "Lists of records" program → `Top student: Meera with 91\nSecond in list: Ravi\n`.
- [ ] Commit "Copy lists and records when they are stored".

**How Tasks 19–22 could pass vacuously:** tests that never change a value after storing it cannot tell a copy from an alias; `copies.gloss` changes every stored value afterwards and shows the originals.

---

## Task 23: Out of memory, end to end  (est. 80 lines / 2 files)

- A run case that appends to a list until memory runs out is too slow and machine-dependent. Instead, `tests/run.rs` supports an optional `<name>.ulimit` file holding a virtual-memory limit in KiB, applied with `sh -c 'ulimit -v <n>; exec <exe>'` inside the runner. Case `out_of_memory.gloss` builds a list in a `repeat while yes` loop of `append` with `.ulimit` `65536` → `.err` `line <n>: failure 10: out of memory\n`, status 2. Under `irsim` the case is skipped (no address-space limit).
- [ ] Commit "Report running out of memory".

## Task 24: The spec guard runs the spec  (est. 140 lines / 2 files)

**Files:** Modify `tests/spec_examples.rs`.

- Every complete program block in `spec/v0.md` without a `table` is built and run (with the runner; skipped without one). Where the spec's prose says what it prints (`This program prints "…"`, `It prints "…"`), the test holds a table from block index to expected output, and a sweep guard asserts the table has at least as many entries as such sentences in the spec (count the phrase `prints "`).
- Every §10 failure message (the `Message` column) is produced by a minimal program held in the test and compared exactly, with the spec's `line 5` replaced by the real line; failures 6–8 are listed as "milestone 3".
- [ ] Commit "Run the spec's programs and check its failure messages".

## Task 25: Docs and as built  (est. 120 lines / 2 files)

- README: the status table marks M2 done; "Try it" shows `gloss build hello.gloss` → `Built hello` and running `./hello` (on Linux) or `orb -m gloss-amd64 ./hello` (on a Mac); the run tests and `GLOSS_RUNNER`; the one-time OrbStack setup (`brew install --cask orbstack`, `orb start`, `orb create --arch amd64 alpine gloss-amd64`, and `orb -m gloss-amd64 sudo apk add binutils` for the `objdump` check).
- Design spec: section 6 "As built" gains "Milestone 2" (what shipped, PRs, deviations by decision id and plan ruling, deferred items).
- [ ] Commit "Document milestone 2".

---

## Spec coverage (self-review)

| Spec item | Task |
|---|---|
| §4 numbers: fixed point, rounding, range, `whole`, `remainder` | 1, 11, 12a–c |
| §4 text, truth, conversions (`as text`, `with N places`, `as number`, `is number`) | 16, 17 |
| §4 showing values (all rows) | 11, 13, 16, 19, 21 |
| §4 values and copies; loop variable stands for the item | 20b, 22 |
| §7 every statement except `load`/`save`/`clear`/`expect`/`input` (M3/M4) | 10a, 12b, 13, 14, 15, 18, 19, 20b |
| §9 features, inputs, give, run, recursion, depth | 15 |
| §10 failures 1–5, 9, 10 with their messages | 12a, 12b, 14, 17, 18, 20a, 15, 23, 24 |
| §11 `gloss build` writes the executable beside the source | 10b |
| §12 target, static ELF, runtime layout, memory, stack | 6, 8b, 9 |
| Design D17–D22, §4.6 | 2a–9, 10a, 10b |
| Final M1 review's M2 notes: `TypeInfo.names`, per-loop variables, stack depth, fall-off-end trap, `MIN` negation is failure 4, executable clash and `.gloss` checks kept | 10a, 15, 12a, 10b |

## Plan rulings

- **P1 (Task 14):** a `for each … from … to` *start* that is not whole is failure 3 with `the start of a count must be whole (<source> is <value>)`. The spec requires both ends to be whole but words a message only for the end. Added to spec §10 in Task 14's PR. Cost if wrong: one message changes.
- **P2 (Task 15):** a feature that falls off its end (impossible if the checker is right) stops with `internal error: feature <name> ended without giving a value`, status 2, instead of returning garbage.
- **P3 (Task 9):** failure routines take the line as an argument; there is no "current line" global.
