# Concepts (Go)

This explains how the Go `tabnasmultisource` package works and how it relates
to the parser engine. For task recipes see the [how-to guide](./guide.md); for
the exact API see the [reference](./reference.md). This document tracks the
TypeScript original (the canonical implementation) and ends with a section
on where the Go port differs.

## What problem it solves

Configuration and data rarely live in one file. You want to split a document
across files, reuse shared fragments, layer overrides, and compose them, all
while the result is still a single parsed value. multisource adds *references*
to the jsonic grammar: a marked path (`@a.jsonic`) that the parser replaces,
in place, with the parsed contents of another source.

## The engine relationship

multisource is a plugin, not a parser. The engine is
`github.com/tabnas/parser/go` (the `tabnas.Tabnas` type), and the package
builds no parser of its own. You build the host parser, usually a jsonic
parser from `github.com/tabnas/jsonic/go`, which installs the relaxed-JSON
grammar on the engine. Then you install the plugin on it with its options:

```go
j := jsonic.Make()
j.Use(tabnasmultisource.MultiSource, map[string]any{
    "resolver": tabnasmultisource.MakeMemResolver(files),
})
```

The plugin builds on two further Go packages:

- **`github.com/tabnas/directive/go`**. Multisource defines its `@` mark as a
  *directive*. The directive package handles recognising the open token and
  invoking an action; multisource supplies the action.
- **`github.com/tabnas/path/go`**. Composes on the same instance to track key
  paths through references when installed.

Because `JsonicProcessor` re-parses through the *same* host parser
(`j.ParseMeta`), a referenced `.jsonic` source can itself contain references,
resolved recursively with the same grammar.

## The resolve → process → splice pipeline

Every reference goes through three independent stages.

### 1. Resolve

`MultiSource`'s directive action reads the reference (a string, or a
`map[string]any` with a `path` key) and calls `ResolvePathSpec` to build a
`PathSpec` (kind, base, full, abs). It then calls the configured **resolver**,
which returns a `Resolution` with the loaded `Src`, the detected `Kind`, the
`Full` path, and whether it was `Found`.

The Go port ships three resolvers: `MakeMemResolver` (a `path → content` map),
`MakeFileResolver` (disk or an injected `io/fs.FS`, with optional preload), and
`MakePkgResolver` (`node_modules` lookup). Because a resolver is just a
function, you can supply your own for HTTP, databases, or test stubs.

### 2. Process

A **processor** turns `res.Src` into `res.Val`, keyed by `Kind`:

- `NONE` (`""`). The raw string.
- `jsonic` / `jsc`. Re-parse through the host parser, enabling recursion.

There is no `json` processor. A `.json` source goes to `NONE` and its value is
the raw text, as for any kind with no processor, until you register a
processor for `json` through the `processor` option, built with the JSON
parser of your choice.

`getProcessor` looks up `Processor[kind]`, then falls back to `Processor[NONE]`,
then to `DefaultProcessor`.

### 3. Splice

`resolveSource` returns the processed value; the action then places it:

- as a pair value (`x: @a.jsonic`), the value becomes the value of that key.
- alone in a map (`{@a.jsonic, c:3}`, or a leading `@a.jsonic`), the referenced
  map's keys are merged into the surrounding map.

The merge honours the engine's policy: `ctx.Cfg.MapMerge` per key if set, else
a deep merge (`tabnas.Deep`), else a plain overwrite. The merge writes
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

- **`val`**. Recognise the mark; at depth 0 push into a map.
- **`map`**. Open a following pair when a mark appears inside a map; close an
  inner map when a new mark arrives.
- **`pair`**. Close the current pair so a mark following a value starts fresh.

These are why `@a.jsonic b:2`, `b:2 @a.jsonic`, and `{x: @a.jsonic}` all parse.

## Design trade-offs

- **Resolvers are pulled out of the parser.** The package never assumes a
  filesystem; the same plugin works against memory or anything you can write a
  function for.
- **Recursion uses the live engine.** Re-parsing through `j.Parse` keeps the
  grammar consistent across the tree.
- **Merging is in-place and deep by default**, making layered overrides
  natural while keeping the parent map reference stable.

## Differences from the TypeScript implementation

The TypeScript implementation (`@tabnas/multisource`) is canonical; this Go
package tracks it but differs in scope and idiom:

- **No `.js` sources (JavaScript processor).** The TS package registers
  `makeJavaScriptProcessor` for the `js` kind: a `.js` reference is loaded by
  *executing* the JavaScript module (`require(res.full)`) and taking its
  exports. Go has no JavaScript runtime, so executing a `.js` source is
  impossible; the `js` kind and `makeJavaScriptProcessor` have no Go
  counterpart, and a `@foo.js` reference falls through to the default raw-string
  processor. Accordingly, the default `ImplicitExt` is
  `[.jsonic, .jsc, .json]` (no `.js`), versus TS's
  `[.jsonic, .jsc, .json, .js]`. Note `.jsc` is *jsonic* content and is fully
  supported (it uses `JsonicProcessor`, as in TS). If you need executable
  configuration in Go, generate a `.json`/`.jsonic` file or register a custom
  `Processor` backed by an embedded interpreter.
- **Processor aliasing.** TS lets a processor entry be a *string* that aliases
  another kind (`{ conf: 'jsonic' }`); Go's `Processor` map values are always
  functions, so register the function directly.
- **Resolvers.** Go ships `MakeMemResolver`, `MakeFileResolver` (disk or
  injected `io/fs.FS`, preload map, pathfinder), and `MakePkgResolver`
  (`node_modules` walking, package.json `main`). The pkg resolver implements
  the portable subset of Node's resolution; it does not implement conditional
  `exports` or `require.resolve` semantics.
- **Preload.** `PreloadFiles` / `PreloadOptions` mirror the TS folder-scanning
  preload: scan folders (optionally recursive) for matching extensions into a
  `path -> content` map, fed to `FileResolverOptions.Preload`. As in TS, the
  `MultiSourceOptions.Preload` field is a declarative record; the plugin does
  not consume it directly.
- **Function signatures.** Go's `Resolver` takes `(spec, opts, ctx)` and
  `Processor` takes `(res, opts, ctx, j)`. The TS equivalents additionally
  receive `rule` and take `tn` (the engine) as the last parameter.
- **Error handling.** Both runtimes now agree: a not-found reference raises
  `multisource_not_found` (with the searched paths and a source location), and
  a source that is an ancestor of itself raises `multisource_cycle` (naming the
  loop). Go carries a processing failure back through `Resolution.Err` and
  re-raises it, so a nested `@missing`, or an error a processor reports, fails
  the whole parse rather than silently substituting raw text, as in TS.
- **Dependency tracking.** Both record a `DependencyMap` when you pass an empty
  `deps` map in the `multisource` parse meta (Go: a `DependencyMap` under
  `ctx.Meta["multisource"]["deps"]`, via `ParseMeta`). Go's `TOP` is a string
  sentinel constant (with a NUL byte, so it cannot collide with a path) rather
  than a JS `Symbol`, and `Dependency.Wen` is Unix milliseconds (`int64`)
  rather than a JS `Date.now()` number.
- **Parse meta.** TS threads meta through `parse(src, meta)`; Go uses
  `ParseMeta(src, map[string]any)` on the host parser. The same keys are honoured
  (`multisource.path`, `multisource.deps`, `multisource.parents`, `fs`).
- **Number type.** Both produce numbers, but Go materialises them as `float64`
  in `map[string]any`, the jsonic Go default. A non-string `path` in an
  object-form directive (`@{path: 1000000}`) is coerced the way the canonical
  coerces it, by ECMAScript `Number::toString` rather than by `%v`, so the
  same reference names the same source in both runtimes
  (`jsNumberToString` in `number.go`; `test/spec/numeric-path.tsv`).
- **Options.** TS merges plugin options with the defaults. Go reads the same
  plain keys (`resolver`, `path`, `markchar`, `processor`, `implictExt`,
  `fs`) over the defaults, and also accepts one typed `*MultiSourceOptions`
  under `"_opts"`, whose unset fields take the defaults.
