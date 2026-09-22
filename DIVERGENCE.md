# Divergences

TypeScript is the canonical implementation; the Go and Rust ports track
it. This file records where a port produces a **different result for the
same input**.

Two divergences are recorded, both in the Rust port, both deliberate,
and both a consequence of what the host language can and cannot do.

## Why there is no executable register here

The porting playbook asks for a divergence register: a fixture every
port runs, so a row that stops being true fails the build, and a row
that gets FIXED fails it too. Neither row below can be written as one.

- The `js` kind needs a real file on disk AND a JavaScript runtime to
  execute it. A shared fixture resolves against an in-memory map (see
  [`test/AGENTS.md`](test/AGENTS.md)), and in that setting the canonical
  TypeScript does not produce a value at all: its `js` processor calls
  `require` on a path that is not a module and the resulting error is a
  plain JavaScript error with no code, which the `ERROR:<code>` contract
  cannot state.
- The depth cap needs an input of several thousand sources. A fixture
  row is one line of a TSV.

The playbook's own escape hatch applies: where a register row cannot
express a divergence, say so here and pin it with a test instead. Both
rows are pinned by [`rs/tests/divergence_test.rs`](rs/tests/divergence_test.rs),
which fails when the Rust side changes, so a repair goes red and names
the row to delete.

## How the columns below were measured

- **Rust** was executed, by the tests named in each row.
- **Go** was executed, in this repository, through the same in-memory
  resolver the fixtures use.
- **TypeScript** was executed, by its own suite in a wired checkout
  (`cd ts && npm test`, with the sibling packages built). Each
  TypeScript cell is cited to the assertion in the canonical suite that
  measures it, or, where no assertion covers it, marked as read from
  `ts/src/multisource.ts` and labelled as such. A cell read from source
  is a claim about the code, not a measurement, and is written that way
  rather than dressed up as one.

## 1. There is no `js` kind

The TypeScript `js` processor is
[`ts/src/processor/js.ts`](ts/src/processor/js.ts), which calls
`require(res.full)`: it EXECUTES the referenced module and takes its
exports, or its `default` export, as the value. `js` is also one of the
four default implicit extensions, so an extensionless reference can find
a `.js` file and end up executing it.

Neither Go nor Rust has a JavaScript runtime. Both leave the kind out,
so a `.js` reference falls through to the raw-text processor and `.js`
is not in the implicit-extension list.

| input | TypeScript | Go | Rust |
|---|---|---|---|
| `d:@"k02.js"`, with `ts/test/k02.js` on disk | `{d: {e: 3}}` | the module text | the module text |
| `d:@"k02"`, with only `ts/test/k02.js` on disk | `{d: {e: 3}}` | `ERROR:multisource_not_found` | `ERROR:multisource_not_found` |

The TypeScript cells are the assertions in
[`ts/test/multisource.test.ts`](ts/test/multisource.test.ts) `file-kind`
(first row, executed by the canonical suite) and the documented
`implictExt` default plus that same processor (second row, read from
source: no canonical assertion covers an extensionless `.js`
reference). The Go cells were executed here against the in-memory
resolver; the Rust cells are pinned by `a_js_source_is_raw_text_here`
and `a_js_file_is_not_found_by_an_extensionless_reference`.

**Reason.** A `js` source is code, and loading one runs it. Rust would
have to embed a JavaScript engine to reproduce the behaviour, and
`AGENTS.md` already says never to let the `js` kind near sources from
outside the system. Leaving the kind out is the safer of the two
answers, and it is the answer Go already gives.

**Who owns the repair.** Nobody, for now. Closing it means adding a
JavaScript runtime as a dependency of a configuration-loading plugin,
which is a larger decision than this port. It is recorded here so the
gap is visible rather than discovered.

## 2. A chain of sources is capped at `max_depth`

A nested source is parsed inside the parse that referenced it, so a
chain of sources is a chain of stack frames.

| input | TypeScript | Go | Rust |
|---|---|---|---|
| a 4000-deep acyclic chain of one-line sources | a `RangeError` thrown out of the parse (read from source: nothing catches it) | `{"end":1}` | `ERROR:multisource_depth` |

The Go cell was executed here. The Rust cell is pinned by
`a_chain_deeper_than_the_cap_is_an_error`, and
`a_chain_inside_the_cap_still_resolves` pins that an ordinary chain is
unaffected. The TypeScript cell is read from source: the plugin wraps no
nested parse in a `try`, so a stack overflow inside one reaches the
caller as V8's `RangeError`, which carries no jsonic error code.

**Reason.** Go is the odd one out in being fine: a goroutine stack grows
on demand, so 4000 levels cost memory and nothing else. A Rust thread's
stack does not grow, and an engine parse frame is large. Measured here,
a debug build used about a third of a megabyte per level, so six levels
exhausted the two megabytes a spawned thread gets by default and the
process ABORTED. A process abort on input a document chose is exactly
what the untrusted-input rules forbid.

Two things answer it, and both are in the port:

1. `processor::parse_nested` moves every second level of nesting onto a
   fresh thread with an eight-megabyte stack, so the chain no longer
   sits on one stack at all. This part is invisible: it changes no
   result.
2. `MultiSourceOptions::max_depth`, default 64, bounds the chain, and
   exceeding it raises `multisource_depth` rather than consuming
   unbounded memory and threads. This part is the divergence.

**Who owns the repair.** The Rust port, and only if the canonical
implementations grow a bound of their own. `max_depth` is an option, so
a caller with a genuinely deeper chain raises it; the default exists so
that a document, rather than the caller, cannot choose how much stack to
use.

## Not divergences

These differ in SHAPE rather than in result, and are listed so that
nobody records them as parity debt later. `rs/README.md` covers them for
readers.

- **The filesystem is a typed option.** TypeScript injects one per parse
  as `ctx.meta.fs`, because a JavaScript metadata object can carry the
  `node:fs` module. A `tabnas::Value` cannot carry a trait object, so
  `SourceFs` lives on `MultiSourceOptions` or on a resolver. Go's
  `MultiSourceOptions.FS` makes the same move.
- **The dependency tree goes to a sink.** TypeScript fills an object the
  caller passed in the parse metadata; a Rust metadata value cannot be
  written back to, so `MultiSourceOptions::with_deps` takes the
  destination. `TOP` is a sentinel string rather than a `Symbol`, as in
  Go.
- **Options are a struct with builder methods**, and a resolver or
  processor is a trait object, rather than entries on an open object.
- **`multisource_depth` is a third error code.** It is declared in the
  error and hint tables alongside the two canonical ones. Nothing else
  in the catalogue changes.

## Where Rust matches TypeScript and Go does not

Recorded so these are not "aligned" to Go by mistake. Both are gaps the
Go port documents in
[`go/doc/concepts.md`](go/doc/concepts.md).

| case | TypeScript | Go | Rust |
|---|---|---|---|
| a processor entry naming another kind (`{foo: 'jsonic'}`) | the aliased processor runs | the entry cannot be expressed, so the kind falls back to raw text | the aliased processor runs |
| `map.merge` | called once with the whole enclosing node | called once per key | called once with the whole enclosing node |

The first is pinned by
`a_processor_alias_matches_typescript_where_go_cannot`; its TypeScript
cell is the canonical `custom-ext` assertion, and its Go cell was
executed here. The second is pinned by `map_merge_is_used_for_the_splice`
and is visible in `ts/src/multisource.ts`, which assigns
`ctx.cfg.map.merge(gp.node, res.val, rule, ctx)` to the grandparent node.

A third row stood here until tabnas/multisource#50 was repaired: a
numeric `path` in an object-form directive, which Go rendered with
`fmt.Sprintf("%v", p)` and so named `1e+20` where the canonical names
`100000000000000000000`. Go now spells such a reference the way
ECMAScript `Number::toString` does, and every runtime is held to it by
[`test/spec/numeric-path.tsv`](test/spec/numeric-path.tsv), so the row
is gone rather than merely marked fixed.
