# Concepts (Go)

This explains how the Go `tabnasmultisource` package works and how it relates
to the parser engine. For task recipes see the [how-to guide](./guide.md); for
the exact API see the [reference](./reference.md). This document tracks the
TypeScript original — the canonical implementation — and ends with a section
on where the Go port differs.

## What problem it solves

Configuration and data rarely live in one file. You want to split a document
across files, reuse shared fragments, layer overrides, and compose them — all
while the result is still a single parsed value. multisource adds *references*
to the jsonic grammar: a marked path (`@a.jsonic`) that the parser replaces,
in place, with the parsed contents of another source.

## The engine relationship

multisource is a plugin, not a parser. The engine is `github.com/tabnas/jsonic/go`
(the `jsonic.Jsonic` type), which carries the relaxed-JSON grammar.
`MakeJsonic` builds a `*jsonic.Jsonic`, applies default options, and installs
the plugin:

```go
j := tabnasmultisource.MakeJsonic(opts)
```

The plugin builds on two further Go packages:

- **`github.com/tabnas/directive/go`** — multisource defines its `@` mark as a
  *directive*. The directive package handles recognising the open token and
  invoking an action; multisource supplies the action.
- **`github.com/tabnas/path/go`** — composes on the same instance to track key
  paths through references when installed.

Because `JsonicProcessor` re-parses through the *same* engine (`j.Parse`), a
referenced `.jsonic` source can itself contain references, resolved
recursively with the same grammar.

## The resolve → process → splice pipeline

Every reference goes through three independent stages.

### 1. Resolve

`MultiSource`'s directive action reads the reference — a string, or a
`map[string]any` with a `path` key — and calls `ResolvePathSpec` to build a
`PathSpec` (kind, base, full, abs). It then calls the configured **resolver**,
which returns a `Resolution` with the loaded `Src`, the detected `Kind`, the
`Full` path, and whether it was `Found`.

The Go port ships one resolver, `MakeMemResolver`, over a `path → content`
map. Because a resolver is just a function, you can supply your own for files,
HTTP, databases, or test stubs.

### 2. Process

A **processor** turns `res.Src` into `res.Val`, keyed by `Kind`:

- `NONE` (`""`) — the raw string.
- `json` — `encoding/json`.
- `jsonic` / `jsc` — re-parse through the engine, enabling recursion.

`getProcessor` looks up `Processor[kind]`, then falls back to `Processor[NONE]`,
then to `DefaultProcessor`.

### 3. Splice

`resolveSource` returns the processed value; the action then places it:

- as a pair value (`x: @a.jsonic`), the value becomes the value of that key.
- alone in a map (`{@a.jsonic, c:3}`, or a leading `@a.jsonic`), the referenced
  map's keys are merged into the surrounding map.

The merge honours the engine's policy: `ctx.Cfg.MapMerge` per key if set, else
a deep merge (`jsonic.Deep`), else a plain overwrite. The merge writes
key-by-key into the grandparent map so existing nested values survive and a
pair following the directive writes into the same node.

## Implicit extensions and index files

When a reference has no extension, `buildPotentials` builds candidate paths and
the resolver tries each in order:

1. the path as given,
2. `path + ext` for each implicit extension,
3. `path/index + ext`.

The first match wins, and its extension sets the `Kind` that selects the
processor. The default order is `.jsonic, .jsc, .json`.

## The grammar tweaks

To let references appear mid-map, top-level, and as the sole content of a
pair, the plugin's `Custom` hook registers grammar alternates under the
`multisource` group tag (via `GrammarSetting.Rule.Alt.G`):

- **`val`** — recognise the mark; at depth 0 push into a map.
- **`map`** — open a following pair when a mark appears inside a map; close an
  inner map when a new mark arrives.
- **`pair`** — close the current pair so a mark following a value starts fresh.

These are why `@a.jsonic b:2`, `b:2 @a.jsonic`, and `{x: @a.jsonic}` all parse.

## Design trade-offs

- **Resolvers are pulled out of the parser.** The package never assumes a
  filesystem; the same plugin works against memory or anything you can write a
  function for.
- **Recursion uses the live engine.** Re-parsing through `j.Parse` keeps the
  grammar consistent across the tree.
- **Merging is in-place and deep by default**, making layered overrides
  natural while keeping the parent map reference stable.

## Differences from the TS version

The TypeScript implementation (`@tabnas/multisource`) is canonical; this Go
package tracks it but differs in scope and idiom:

- **Resolvers.** Go ships only `MakeMemResolver`. The TS package additionally
  provides `makeFileResolver` (disk / virtual `fs`, `node_modules` walking,
  preload) and `makePkgResolver` (Node module resolution). There is no Go
  equivalent of those, nor of `preloadFiles` / `PreloadOptions`.
- **Processors.** Go has `DefaultProcessor`, `JSONProcessor`, and
  `JsonicProcessor` (covering `json`, `jsonic`, `jsc`). There is no `js`
  processor — Go cannot `require` a JavaScript module — so the TS `js` kind and
  `makeJavaScriptProcessor` have no counterpart. Default `ImplicitExt` is
  `[.jsonic, .jsc, .json]` (no `.js`), versus TS's `[.jsonic, .jsc, .json, .js]`.
- **Processor aliasing.** TS lets a processor entry be a *string* that aliases
  another kind (`{ conf: 'jsonic' }`); Go's `Processor` map values are always
  functions, so register the function directly.
- **Function signatures.** Go's `Resolver` takes `(spec, opts)` and `Processor`
  takes `(res, opts, j)`. The TS equivalents also receive `rule`, `ctx`, and
  `tn` (the engine). Go's narrower signatures reflect the smaller resolver set.
- **Error handling.** A not-found reference in Go yields `nil` for that value
  (no error raised). The TS plugin raises a `multisource_not_found` error with
  searched paths and a source location.
- **Dependency tracking.** TS records a `DependencyMap` (and exposes `TOP`,
  `Dependency`, `MultiSourceMeta`) when you pass a `deps` object in parse meta.
  The Go port does not track dependencies.
- **Parse meta.** TS threads rich meta (`multisource.path`, `deps`, `parents`,
  `fs`, `fileName`) through `parse(src, meta)`. The Go `Parse`/`Jsonic.Parse`
  take only the source string; base path and options are set on the instance.
- **Number type.** Both produce numbers, but Go materialises them as `float64`
  in `map[string]any`, the jsonic Go default.
- **Performance.** Go's no-options `Parse` caches a single default parser
  (`sync.Once`) because building the grammar dominates a parse; the TS package
  does not need this because callers reuse a `Tabnas` instance directly.
- **Values type.** The Go engine is created with `ValueOptions.Lex` enabled so
  bare values lex correctly alongside the `@` mark; this is engine
  configuration the TS side handles through its own value plugin.
