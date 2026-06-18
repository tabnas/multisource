# Concepts

This explains how `@tabnas/multisource` works, why it is built the way it is,
and how it relates to the parser engine. For task recipes see the
[how-to guide](./guide.md); for the exact API see the
[reference](./reference.md).

## What problem it solves

Configuration and data rarely live in one file. You want to split a document
across files, reuse shared fragments, layer overrides, and pull values in from
packages — all while the result is still a single parsed object. multisource
adds *references* to the jsonic grammar: a marked path (`@a.jsonic`) that the
parser replaces, in place, with the parsed contents of another source.

The result is composition without a separate build step. Parsing a top
document transparently parses everything it references.

## The engine relationship

multisource is a plugin, not a parser. The parser engine is `@tabnas/parser`
(the `Tabnas` class); `@tabnas/jsonic` supplies the relaxed-JSON grammar that
lets you write `a:1`. You assemble them:

```ts
new Tabnas().use(jsonic).use(MultiSource, options)
```

multisource builds on two further plugins:

- **`@tabnas/directive`** — multisource defines its `@` mark as a *directive*.
  The directive plugin handles the mechanics of recognising an open token and
  invoking an action; multisource supplies the action (resolve + process +
  splice).
- **`@tabnas/path`** — when present, multisource passes the current key path
  down to nested parses (`rule.k.path`), so a referenced source knows where it
  sits in the overall tree.

Because the jsonic processor re-parses through the *same* engine instance
(`tn.parse`), a referenced `.jsonic` file can itself contain references, and
they resolve recursively with the same grammar and options.

## The resolve → process → splice pipeline

Every reference goes through three stages. Separating them is the core design
decision: *where* a source comes from (resolve), *how* its bytes become a
value (process), and *where* the value goes (splice) are independent.

### 1. Resolve

The directive action reads the reference — a string, or an object with a
`path` key — and hands it to the configured **resolver**. The resolver returns
a `Resolution`: the loaded source text (`src`), its detected `kind`, the
`full` path it was found at, and whether it was `found`.

`resolvePathSpec` does the shared normalisation first: it detects absolute
paths, joins the base path, and extracts the `kind` from the file extension.
Each resolver then differs only in *where* it looks:

- **mem** — a `path → content` map. No I/O; ideal for tests and embedding.
- **file** — `node:fs` (or a virtual `ctx.meta.fs`). Adds `node_modules`
  walking and preload support.
- **pkg** — Node module resolution (`require.resolve`), then `node_modules`
  walks, then the filesystem.

This split means you can write an HTTP resolver, a database resolver, or a
test stub without touching the parsing logic — a resolver is just a function.

### 2. Process

A **processor** turns the resolved `src` string into a value, keyed by the
source's `kind` (its extension without the dot). The default set:

- `''` (the `NONE` fallback) — returns the raw string.
- `json` — strict JSON.
- `jsonic` / `jsc` — re-parse through the engine, enabling recursion.
- `js` — `require` the module and unwrap a `.default` export.

Processor selection allows one level of *aliasing*: a string value names
another kind, so `{ conf: 'jsonic' }` routes `.conf` files through the jsonic
processor. Anything with no matching processor falls back to `NONE`.

### 3. Splice

The value is then placed into the parse tree, and *how* depends on where the
reference appears:

- **As a pair value** (`x:@a.jsonic`) — the value becomes the value of that
  key: `{ x: <value> }`.
- **Alone in a map** (`{@a.jsonic, c:3}`, or a leading `@a.jsonic`) — the
  referenced map's keys are merged into the surrounding map.

The merge respects the engine's map-combination policy. If `cfg.map.merge` is
configured it is called per key; otherwise a deep merge (`tn.util.deep`) is
used (so layering keeps unmentioned keys); a `map.extend: false` setting falls
back to a shallow `Object.assign`. This is why `{@base, @override}` keeps
`base`'s keys and lets `override` win on the keys it sets.

## Implicit extensions and index files

References usually omit the extension. When `kind` is empty, resolvers build a
list of *potentials* (`buildPotentials`) and try each in order:

1. `path + ext` for each implicit extension.
2. `path/index + ext` (a folder index file).
3. `path/index.<foldername> + ext` (a named folder index).

The first existing source wins, and its actual extension sets the `kind` that
chooses the processor. The default extension order is `.jsonic, .jsc, .json,
.js` — the most specific format first. This is the same convention as Node's
module resolution, which makes references feel familiar.

## The grammar tweaks

To make references appear in three positions — mid-map, top-level, and as the
sole content of a pair — multisource registers a handful of grammar
alternates under the `multisource` group tag (the `directive` plugin's
`custom` hook):

- **`val`** — recognise the mark; at depth 0 push into a map so a bare leading
  `@a.jsonic` produces an object.
- **`map`** — when a mark appears inside a map, open the implicit top-level map
  node and a following pair; close an inner map when a new mark arrives.
- **`pair`** — close the current pair so a mark following a value starts fresh.

These rules are why `@a.jsonic b:2`, `b:2 @a.jsonic`, and `{x:@a.jsonic}` all
parse correctly. The implicit top-level map node is explicitly allocated
because the core no longer auto-allocates it — without that, a pair following a
leading directive would have nowhere to write.

## Dependency tracking

Because resolution is recursive, multisource can record the *tree* of which
source pulled in which. Pass an empty `deps` object in the `multisource` meta
and the plugin populates a flat `DependencyMap`: for each target path, the set
of source paths it referenced, each with a timestamp. The top of the tree is
the `TOP` symbol. This is the raw material for cache invalidation and watch
lists: when an upstream file changes, you know which documents to re-parse.

## Design trade-offs

- **Resolvers are pulled out of the parser.** The plugin never assumes a
  filesystem. The cost is a little ceremony (you must choose a resolver); the
  benefit is that the same plugin works against memory, disk, packages, or
  anything you can write a function for, including virtual filesystems for
  tests.
- **Recursion uses the live engine.** Re-parsing through `tn.parse` keeps
  grammar and options consistent across the tree, at the cost of sharing one
  engine instance. The `js` processor instead delegates to Node's module
  cache.
- **Implicit extensions trade explicitness for convenience.** Trying several
  extensions means a reference is slightly ambiguous, but it matches how
  developers already think about imports and keeps documents terse.
- **Merging is in-place and deep by default.** This makes layering overrides
  natural, but means a referenced map shares structure with the parent — the
  plugin is careful to keep the parent node reference stable so following
  pairs write into the same object.
