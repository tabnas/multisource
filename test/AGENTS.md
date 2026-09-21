# Agents Guide — shared spec fixtures

`spec/*.tsv` holds the cross-runtime conformance fixtures. Both runtimes
auto-discover and run **every** file in this directory, so a change here
affects TypeScript and Go together — edit with that in mind.

## Format

Tab-separated, one case per line, with a header row naming the columns.
Blank lines are skipped, and so are comment lines — a line starting with
`#` that contains no tab. (A data row always has at least one tab, so a
`#`-leading source such as a C preprocessor directive still works.)

| Column | Meaning |
|---|---|
| `input` | Tabnas source with @-references into the in-memory source set given by the opts column. Escapes `\n` `\r` `\t` `\\` are decoded. |
| `expected` | A JSON value (the parse result), or `ERROR` / `ERROR:<code>` for inputs that must fail. The code is compared **exactly** — it is the error's code, not a substring of its message. |
| `opts` | JSON: `mem` is the in-memory source set the case resolves against; `options` (optional) is engine options to apply. |

`expected` and `opts` are **not** escape-decoded — they are raw JSON, so
JSON's own escape rules apply (`"a\nb"` is a string containing a newline).
To put a literal backslash in `input`, write `\\`.

A real filesystem cannot live in a fixture, so these cases all run against
the in-memory resolver built from the `mem` map. The file, pkg and preload
resolvers stay covered by the in-language tests in `ts/test/multisource.test.ts`,
`go/{fs,resolver,preload}_test.go` and `rs/tests/file_corpus_test.rs`.

Results are compared after a JSON round-trip, so key order and the
`OrderedMap` / null-prototype-object representations do not affect the
comparison.

## Who runs what

- TypeScript: `ts/test/parity.test.ts` — `makeRunner(...).dir(...)`.
- Go: `go/parity_test.go` — `support.Runner{...}.Dir(t, dir)`.
- Rust: `rs/tests/parity_test.rs` — `Runner::new_with_row(...).dir(&dir)`.

All three are a dozen lines holding only what is specific to
multisource: how to build the parser for a row's options. Everything
else — finding `test/spec`, reading the file, decoding escapes, the
`ERROR:` contract, the comparison, the `<file>:<line>` in a failure
message — comes from [`@tabnas/support`](https://github.com/tabnas/support)
and its Go and Rust halves, so the loaders cannot drift from each other
either.

All three discover files by directory listing: adding a `.tsv` here runs
it in every runtime without touching any runner. An empty fixture, and a spec
directory with no fixtures in it, both **fail** — a runner that reports
green having run nothing is indistinguishable from coverage that was never
there.

## Rules

- Prefer adding a fixture here over a one-off in-language assertion when a
  case is expressible as input → output. That is what keeps the two
  runtimes honest against each other.
- TypeScript is canonical. If the two runtimes disagree, the TS behaviour is
  the expected value — unless Go has exposed a genuine TS defect, in which
  case fix TS first and pin the corrected behaviour here.
- A new fixture must pass in EVERY runtime: run `go test ./...` (from
  `go/`), `npm test` (from `ts/`) and `cargo test --all-targets` (from
  `rs/`) before considering it done.
