# Gloss

Gloss is a small language that reads like plain sentences and compiles straight to a native executable. It is for command-line programs that work on records and tables stored in files.

```gloss
record Expense
  name as text
  amount as number
  category as text

table expenses of Expense in "expenses.csv"

feature category_total
  needs wanted as text
  set total to 0
  for each expense in expenses
    if category of expense is wanted
      add amount of expense to total
  give total

test "category total is zero when nothing matches"
  expect category_total with "rent" is 0
```

## Why

Agents now write most code, and fewer people read it. Gloss aims to make program logic readable again, for three groups:

- **Developers who have drifted from code** use it to stay close to algorithms and to what the machine does.
- **People who have never programmed** use it to read what an agent wrote for them, check it through tests, and change it themselves.
- **Agents** write it. The whole language is small enough to fit in a prompt.

## Design rules

1. **Structured like code, worded like English.** Every statement starts with a fixed keyword.
2. **One form per idea.** There are no synonyms and no optional words.
3. **No grammar words.** There are no articles, pronouns or plurals.
4. **Symbols for arithmetic, words for everything else.**
5. **The compiler infers but never guesses.**
6. **Nothing hidden.** Every line can show the machine instructions it became, and file access is approved at build time.

## Status

Gloss is a specification only. No compiler exists yet, and none of the examples have been compiled or run.

- [`spec/v0.md`](spec/v0.md) is the v0 spec. Its appendices are the complete grammar (Appendix A) and the changes from earlier drafts (Appendix B).
- v0 targets x86-64 Linux (kernel 5.6 or later). It produces a single static executable, built with no assembler, linker, C library or LLVM.
- Open decisions are listed in section 14 of the spec. They include the language the compiler is written in, and the constructs added in the latest revision that still need approval.

## Planned commands

| Command | What it does |
|---|---|
| `gloss build expenses.gloss` | Checks the program, asks for approval of its file access, and writes the executable `expenses` |
| `gloss test expenses.gloss` | Runs every test block and prints each title with pass or fail |
| `gloss format expenses.gloss` | Rewrites the file into the one accepted form |
| `gloss trace expenses.gloss` | Runs the program and narrates each step in plain words |
| `gloss explain expenses.gloss` | Prints each source line beside its machine instructions |
| `gloss flowchart expenses.gloss` | Writes one SVG flowchart per feature |
