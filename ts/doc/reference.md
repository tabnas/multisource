# Reference

Complete API surface of `@tabnas/multisource`. The package is CommonJS;
imports below use ESM syntax for brevity.

## Installation

```sh
npm install @tabnas/multisource @tabnas/parser @tabnas/jsonic
```

Peer dependencies: `@tabnas/parser` (the engine), `@tabnas/jsonic` (grammar),
`@tabnas/directive`, `@tabnas/path`.

## Entry points (package exports)

| Specifier | Exports |
| --- | --- |
| `@tabnas/multisource` | `MultiSource`, `resolvePathSpec`, `preloadFiles`, `NONE`, `TOP`, `meta`, and all types |
| `@tabnas/multisource/resolver/mem` | `makeMemResolver`, `buildPotentials` |
| `@tabnas/multisource/resolver/file` | `makeFileResolver` |
| `@tabnas/multisource/resolver/pkg` | `makePkgResolver` |
| `@tabnas/multisource/processor/jsonic` | `makeJsonicProcessor` |
| `@tabnas/multisource/processor/js` | `makeJavaScriptProcessor` |

## `MultiSource`

```ts
const MultiSource: Plugin
```

The plugin. Install it with `tn.use(MultiSource, options)` on a `Tabnas`
instance that already has a grammar (typically `jsonic`).

```ts
import { Tabnas } from '@tabnas/parser'
import { jsonic } from '@tabnas/jsonic'
import { MultiSource } from '@tabnas/multisource'

new Tabnas().use(jsonic).use(MultiSource, options)
```

`MultiSource.defaults` holds the default options (see below).

## `MultiSourceOptions`

```ts
type MultiSourceOptions = {
  resolver: Resolver
  path?: string
  markchar?: string
  processor?: { [kind: string]: Processor }
  implictExt?: string[]
  preload?: PreloadOptions
}
```

| Field | Type | Default | Purpose |
| --- | --- | --- | --- |
| `resolver` | `Resolver` | required | Resolves a reference to source content. |
| `path` | `string` | — | Base path prefixed to relative references. |
| `markchar` | `string` | `'@'` | Single character that opens a reference. |
| `processor` | `{ [kind]: Processor }` | see defaults | Per-kind source transformers. |
| `implictExt` | `string[]` | `['jsonic','jsc','json','js']` | Extensions tried when a reference has none. Normalised to begin with `.`. |
| `preload` | `PreloadOptions` | — | Reserved option for preload configuration (see `preloadFiles`). |

Note the spelling `implictExt` (this is the actual property name).

### Default options

```ts
MultiSource.defaults = {
  markchar: '@',
  processor: {
    '':       defaultProcessor,   // raw string passthrough
    jsonic:   jsonicProcessor,
    jsc:      jsonicProcessor,
    json:     jsonProcessor,
    js:       jsProcessor,
  },
  implictExt: ['jsonic', 'jsc', 'json', 'js'],
}
```

The empty-string key (`NONE`) is the fallback processor used when no kind
matches.

## Resolvers

A resolver is any function of type `Resolver`. Three are provided.

### `makeMemResolver`

```ts
import { makeMemResolver } from '@tabnas/multisource/resolver/mem'
function makeMemResolver(filemap: { [fullpath: string]: string }): Resolver
```

Resolves references against an in-memory `path → content` map. Tries implicit
extensions and `index` files when the reference has no extension.

### `makeFileResolver`

```ts
import { makeFileResolver } from '@tabnas/multisource/resolver/file'

type PathFinder = (spec: any) => string
type FileResolverOptions = {
  pathfinder?: PathFinder
  preload?: { [fullpath: string]: string }
}

function makeFileResolver(
  pathfinderOrOpts?: PathFinder | FileResolverOptions
): Resolver
```

Reads from `node:fs`, or from `ctx.meta.fs` when a virtual filesystem is
passed in the parse meta. Argument forms:

- omitted — read straight from the resolved path.
- a `PathFinder` function — rewrite the spec to a path before resolving.
- a `FileResolverOptions` object — supply a `pathfinder` and/or a `preload`
  map. Preloaded content is checked before disk.

When a reference has no extension, the resolver also tries `node_modules`
lookups (up to 7 levels up), implicit extensions, and `index` files.

### `makePkgResolver`

```ts
import { makePkgResolver } from '@tabnas/multisource/resolver/pkg'
function makePkgResolver(options: {
  require: Function | string | string[]
}): Resolver
```

Resolves references through Node module resolution. `options.require`:

- a `require` function — used as `require.resolve`.
- a `string` — a single `node_modules` search root.
- a `string[]` — multiple search roots.

Resolution order: `require.resolve`, then a `node_modules` walk up the tree,
then the `require.main.paths`, then implicit-extension potentials, then a
direct filesystem check (which also supports `ctx.meta.fs`).

## Processors

A processor is any function of type `Processor`. It must assign the parsed
value to `res.val`. Defaults:

| Kind | Behaviour |
| --- | --- |
| `''` (NONE) | Returns the raw source string unchanged. |
| `json` | Parses with a strict-JSON jsonic instance. |
| `jsonic`, `jsc` | Re-parses the source through the current engine. |
| `js` | `require`s the module, unwrapping a `.default` export. |

### `makeJsonicProcessor`

```ts
import { makeJsonicProcessor } from '@tabnas/multisource/processor/jsonic'
function makeJsonicProcessor(): Processor
```

Returns a processor that re-parses `res.src` through the engine (`tn.parse`),
inheriting the current grammar — so nested references resolve recursively.
Used for `.jsonic` and `.jsc` by default.

### `makeJavaScriptProcessor`

```ts
import { makeJavaScriptProcessor } from '@tabnas/multisource/processor/js'
function makeJavaScriptProcessor(opts?: {}): Processor
```

Returns a processor that `require`s the resolved module by its full path and
returns its export (preferring `module.exports.default`). Used for `.js` by
default.

### Processor aliasing

A processor entry may be a *string* naming another kind. The plugin resolves
one level of aliasing, so `{ conf: 'jsonic' }` makes `.conf` files parse as
jsonic.

## Utilities

### `resolvePathSpec`

```ts
function resolvePathSpec(
  popts: MultiSourceOptions,
  ctx: Context,
  spec: any,
  resolvefolder: (path: string, fs: FST) => string,
): PathSpec
```

Normalises a reference (a string, or an object with a `path` key) into a
`PathSpec`: detects absolute paths, joins the base, and extracts the `kind`
from the extension. Used internally by every resolver; exported so custom
resolvers can reuse it.

### `preloadFiles`

```ts
function preloadFiles(
  opts: PreloadOptions,
  fs?: FST,
): { [fullpath: string]: string }

type PreloadOptions = {
  folders: string[]
  ext?: string[]          // default: ['.jsonic', '.json']
  recursive?: boolean     // default: false
}
```

Scans the given folders and returns a flat map of full path → file contents
for files matching `ext`. Set `recursive` to descend into subfolders.
Non-existent folders and unreadable files are skipped silently. Feed the
result to `makeFileResolver({ preload })`.

### `NONE` and `TOP`

```ts
const NONE = ''            // the unknown/empty kind
const TOP: unique symbol   // marker for the top of the dependency tree
```

### `meta`

```ts
const meta = { name: 'MultiSource' }
```

Plugin metadata.

## Parse meta

Pass these under the second argument to `parse(src, meta)`:

| Key | Type | Purpose |
| --- | --- | --- |
| `fs` | filesystem object | Virtual filesystem for file/pkg resolvers. |
| `fileName` | `string` | Name used in error messages. |
| `multisource.path` | `string` | Base path for this parse run. |
| `multisource.deps` | `DependencyMap` | Empty object to be filled with the dependency tree. |
| `multisource.parents` | `string[]` | Parent source paths (managed by the plugin). |

## Types

```ts
type PathSpec = {
  kind: string      // source kind (extension without the dot), or ''
  path?: string     // original (possibly relative) path
  full?: string     // normalised full path
  base?: string     // current base path
  abs: boolean      // true if the path was absolute
}

type Resolution = PathSpec & {
  src?: string      // loaded source text, undefined if not found
  val?: any         // processed value
  found: boolean    // true if a source was found
  search?: string[] // paths the resolver tried
}

type Resolver = (
  spec: PathSpec,
  popts: MultiSourceOptions,
  rule: Rule,
  ctx: Context,
  tn: Tabnas,
) => Resolution

type Processor = (
  res: Resolution,
  popts: MultiSourceOptions,
  rule: Rule,
  ctx: Context,
  tn: Tabnas,
) => void

type Dependency = {
  tar: string | typeof TOP  // the target that depends on the source
  src: string               // the source depended upon
  wen: number               // resolution timestamp (Date.now())
}

type DependencyMap = {
  [tar_full_path: string]: {
    [src_full_path: string]: Dependency
  }
}

type MultiSourceMeta = {
  path?: string
  parents?: string[]
  deps?: DependencyMap
}
```

## Errors

| Code | Message | When |
| --- | --- | --- |
| `multisource_not_found` | `source not found: {path}` | The resolver returned `found: false` (no source at any searched path). |

The error includes the searched paths and the source location, e.g.
`...:1:3` (line:column) or `fileName:line:column` when a `fileName` meta is
supplied.
