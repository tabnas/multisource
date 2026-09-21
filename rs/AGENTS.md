# Agents Guide: rs/

The Rust port of the canonical TypeScript in [`../ts`](../ts). Read
[`../AGENTS.md`](../AGENTS.md) first: it holds the cross-runtime rules,
the error codes, the untrusted-input rules and the shared fixture
contract. This file covers only what is specific to this crate.

## Layout

| Path | |
|---|---|
| `src/lib.rs` | the plugin: options, `PathSpec`, `Resolution`, the `Resolver` trait, the directive action (resolve, cycle check, depth check, dependency record, process, splice), the grammar, `multisource`, `plugin`, `plugin_with`, `make`, `make_with`, `parse`, `VERSION` |
| `src/resolver.rs` | `MapResolver`, `FileResolver`, `PkgResolver`, and the root confinement |
| `src/processor.rs` | the `Processor` trait, the three default processors, and `parse_nested` |
| `src/preload.rs` | `PreloadOptions` and the folder scan |
| `src/vfs.rs` | `SourceFs`, `OsFs`, `MapFs`, path cleaning and `within_root` |
| `tests/parity_test.rs` | every `../test/spec/*.tsv` fixture through `tabnas_support::Runner::new_with_row`, a fresh parser per row from its `opts` column |
| `tests/multisource_test.rs` | the in-language port of `go/multisource_test.go`, `go/{cycle,deps,nested,colon_chain}_test.go` and the parts of `ts/test/multisource.test.ts` a fixture cannot express |
| `tests/file_corpus_test.rs` | the real-filesystem cases over the same `ts/test/*` files the canonical suite loads, plus the package resolver and the preload scan on disk |
| `tests/untrusted_test.rs` | hostile input: odd documents, odd source content, very long input, unbounded chains, and the sandbox root |
| `tests/divergence_test.rs` | the Rust side of every row in `../DIVERGENCE.md` |
| `tests/debug_model_test.rs` | composition with `tabnas-debug`: the rule set, the entry rule, and the `val` to `multisource` edges |
| `tests/perf_test.rs` | instance reuse beats rebuild-per-parse (`go/perf_test.go`, `ts/test/perf.test.ts`) |
| `tests/version_test.rs` | `Cargo.toml` == `VERSION` == `ts/package.json` == `go/multisource.go` |
| `tests/common/mod.rs` | the per-row parser, JSON flattening, number canonicalization, failure conversion |
| `README.md` | the crate front page, prose-gated; its `rust` fences are doctests of this crate |

Crate `tabnas-multisource`, library `tabnas_multisource`. The engine
(`tabnas`), the jsonic base (`tabnas-jsonic`, which brings
`tabnas-json`) and the directive plugin (`tabnas-directive`) are **path
dependencies on sibling checkouts**, as are the dev-only
`tabnas-support`, `tabnas-path` and `tabnas-debug`. None is published,
so there is no registry version to fall back on.

```bash
cargo build --all-targets
cargo test --all-targets && cargo test --doc
cargo clippy --all-targets --all-features -- -D warnings
cargo fmt
```

`make test-rs` from the repository root is the fast loop; `ci/rust/run.sh`
is the full gate and adds `fmt --check`, the lockfile check and the MSRV
pin.

## How the plugin is built

`multisource(parser, options)` mirrors the TypeScript `MultiSource`
function, in this order:

1. **The host grammar is checked.** The plugin modifies `val`, `map` and
   `pair`; it defines no value grammar. Installing it on a bare engine
   would register a directive that can never match, so `install` reads
   `parser.rule_names()` and refuses instead.
2. **The error and hint templates are registered** through `set_options`,
   matching the TypeScript `tn.options({error, hint})` text exactly, plus
   the Rust-only `multisource_depth` pair.
3. **The live instance is published for nested parses.** See below.
4. **The directive is applied** through `tabnas_directive::apply`, with
   the `pair` open rule gated on `r.lte('pk', 0)` as in TypeScript, and a
   `custom` hook that installs the `val` / `map` / `pair` alternates
   letting a reference stand as a bare top-level key, a value, or a pair
   value.

### The nested-parser slot

The canonical TypeScript processor calls `tn.parse(res.src, ctx.meta)`
on the instance the plugin closed over, and a JavaScript closure sees
every later change to that instance, so a plugin installed AFTER
multisource still reaches a nested parse. A Rust callback cannot hold a
borrow across a parse, so instead:

- `parse_prepare_with_instance` publishes `Arc<Tabnas>` (one clone of the
  live instance) at the start of a top-level parse, and only when the
  source contains the mark character at all, so a document with no
  reference pays nothing.
- A nested parse carries `multisource.path` in its metadata and skips
  publishing, so a chain costs one clone, not one per level.
- `tests/multisource_test.rs`
  `a_plugin_installed_afterwards_reaches_the_nested_parse` is what keeps
  the freshness honest.

### The segmented stack

A nested source is parsed inside the parse that referenced it, and an
engine parse frame is large: a debug build uses around a third of a
megabyte per level, so six levels exhausted the two megabytes a spawned
thread gets and the process aborted. `processor::parse_nested` therefore
moves every second level onto a fresh thread with an eight-megabyte
stack and joins it. The call stays synchronous; what changes is that the
chain is bounded by `MultiSourceOptions::max_depth` rather than by
whatever stack the caller happened to have. Go needs none of this
because a goroutine stack grows.

### The report slot

A nested parse RETURNS its error rather than throwing it, and
`TabnasError` does not expose the detail bag its message and hint were
rendered from. Without help, re-raising a nested `multisource_cycle` at
the outer level would print `{loop}` unsubstituted. `LastReport` records
the bag as a diagnostic is raised and hands it back when the same code
comes back, so the caller reads the innermost report, which is what the
TypeScript exception carries out. It is TAKEN, not read, so a report is
used once.

## Where the Rust shape differs

Recorded in [`../DIVERGENCE.md`](../DIVERGENCE.md) and summarized in
`README.md`. In short: the filesystem is a typed option rather than parse
metadata, the dependency tree goes to a caller-supplied sink rather than
into a metadata object, `TOP` is a sentinel string rather than a
`Symbol`, there is no `js` kind, and a chain of sources is capped.

Two things the Rust port gets RIGHT where Go cannot, so do not "align"
them to Go:

- **A processor entry may alias another kind** (`with_processor_alias`),
  one level deep, as in TypeScript. Go's processor map holds functions
  only.
- **The `map.merge` callback is handed the whole node**, as in
  TypeScript, rather than being called per key.

## Verify your work

```bash
cd rs && cargo test --all-targets && cargo test --doc
cd rs && cargo clippy --all-targets --all-features -- -D warnings
bash ci/rust/run.sh        # from the repository root
```

Then the prose gate, because `rs/README.md` is in the gated set:

```bash
vale --minAlertLevel=error $(node ts/scripts/gated-docs.cjs)
node ts/scripts/vale-counts.cjs
node --test ts/test/docs.test.js
```

Every row of every `../test/spec/*.tsv` fixture must pass. A row green in
one runtime and red in another is a failure, not a discrepancy.
