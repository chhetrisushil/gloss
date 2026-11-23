# Examples

Small Gloss programs that compile to x86-64 Linux executables with today's compiler.

| Program | What it shows |
|---|---|
| [fizzbuzz.gloss](fizzbuzz.gloss) | A counting loop, `remainder of`, and an `if` chain |
| [gcd.gloss](gcd.gloss) | Euclid's greatest common divisor by recursion, and the least common multiple built on it |
| [primes.gloss](primes.gloss) | Trial division in a feature that gives yes or no, used as a condition |
| [fibonacci.gloss](fibonacci.gloss) | Fibonacci numbers two ways: a loop, and recursion that checks it |
| [collatz.gloss](collatz.gloss) | Collatz step counts in a `repeat while`, and the longest run below 31 |
| [square_root.gloss](square_root.gloss) | Newton's method on fixed-point numbers, and `as text with 2 places` |
| [palindromes.gloss](palindromes.gloss) | Reversing a number's digits with `remainder of` and `whole of` |

Build and run one (on a Mac, through the `gloss-amd64` OrbStack machine):

```
gloss build examples/primes.gloss
orb -m gloss-amd64 ./examples/primes
```

On x86-64 Linux, run `./examples/primes` directly. Each `<name>.out` is the output the program
must show; `tests/examples.rs` builds and runs every example and checks it, and checks that each
one is already in the layout `gloss format` writes.
