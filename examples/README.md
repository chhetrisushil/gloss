# Examples

Small Gloss programs that compile to x86-64 Linux executables with today's compiler.

| Program | What it shows |
|---|---|
| [fizzbuzz.gloss](fizzbuzz.gloss) | A counting loop, `remainder of`, and an `if` chain; has tests |
| [gcd.gloss](gcd.gloss) | Euclid's greatest common divisor by recursion, and the least common multiple built on it; has tests |
| [primes.gloss](primes.gloss) | Trial division in a feature that gives yes or no, used as a condition; has tests |
| [fibonacci.gloss](fibonacci.gloss) | Fibonacci numbers two ways: a loop, and recursion that checks it; has tests |
| [collatz.gloss](collatz.gloss) | Collatz step counts in a `repeat while`, and the longest run below 31; has tests |
| [square_root.gloss](square_root.gloss) | Newton's method on fixed-point numbers, and `as text with 2 places`; has tests |
| [palindromes.gloss](palindromes.gloss) | Reversing a number's digits with `remainder of` and `whole of`; has tests |
| [binary_search.gloss](binary_search.gloss) | Binary search: finding a hidden number in a range, counting the steps, and the whole square root of a number; has tests |
| [bubble_sort.gloss](bubble_sort.gloss) | Bubble sort on a copy of a list: swapping with `set item i of`, stopping early once a pass makes no swap, and counting the passes; has tests |
| [expenses.gloss](expenses.gloss) | A table: `clear`, `append`, `save` to `expenses.csv`, then `load` it back and show it; approved by `expenses.allow`; has tests |
| [report.gloss](report.gloss) | A read-only table loaded from the committed `report.csv`, with totals and a `paid` truth column; has tests |

Programs that use files are approved by the `<name>.allow` beside them, and find their files in the folder they are run from; `tests/examples.rs` runs every example in a temporary copy of this folder, so `expenses.gloss` never leaves an `expenses.csv` here.

Tests: every example has `test` blocks, which are not part of the built program. `gloss test examples/gcd.gloss` runs them on Linux and prints each title with `pass` or `fail`; each `<name>.tests.out` is exactly that output, and `tests/examples.rs` checks it.

Build and run one (on a Mac, through the `gloss-amd64` OrbStack machine):

```
gloss build examples/primes.gloss
orb -m gloss-amd64 ./examples/primes
```

On x86-64 Linux, run `./examples/primes` directly. Each `<name>.out` is the output the program
must show; `tests/examples.rs` builds and runs every example and checks it, and checks that each
one is already in the layout `gloss format` writes.
