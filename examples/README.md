# Examples

Small Gloss programs that compile to x86-64 Linux executables with today's compiler.

| Program | What it shows |
|---|---|
| [fizzbuzz.gloss](fizzbuzz.gloss) | A counting loop, `remainder of`, and an `if` chain |
| [gcd.gloss](gcd.gloss) | Euclid's greatest common divisor by recursion, and the least common multiple built on it |
| [primes.gloss](primes.gloss) | Trial division in a feature that gives yes or no, used as a condition |

Build and run one (on a Mac, through the `gloss-amd64` OrbStack machine):

```
gloss build examples/primes.gloss
orb -m gloss-amd64 ./examples/primes
```

On x86-64 Linux, run `./examples/primes` directly. Each `<name>.out` is the output the program
must show; `tests/examples.rs` builds and runs every example and checks it, and checks that each
one is already in the layout `gloss format` writes.
