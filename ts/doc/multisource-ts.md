# multisource plugin for Jsonic (TypeScript)

The `multisource` plugin loads partial values from external sources (files,
npm packages, in-memory maps, ...) while parsing Jsonic input. A directive
character (`@` by default) marks a reference; the plugin resolves the
reference, parses the resolved source, and splices the result into the
output.


## Installation

```sh
npm install @jsonic/multisource
```

Peer dependencies: `jsonic`, `@jsonic/directive`, `@jsonic/path`.


## Tutorials

### Parse a reference to an in-memory source

The memory resolver is the simplest way to try the plugin. Files are passed
as a `path → content` map:

```ts
import { Jsonic } from 'jsonic'
import MultiSource from '@jsonic/multisource'
import { makeMemResolver } from '@jsonic/multisource/resolver/mem'

const j = Jsonic.make().use(MultiSource, {
  resolver: makeMemResolver({
    'a.jsonic': 'a:1',
  }),
})

j('x:@a.jsonic, y:2')
// => { x: { a: 1 }, y: 2 }
```

### Load references from files on disk

Swap the memory resolver for the file resolver:

```ts
import { makeFileResolver } from '@jsonic/multisource/resolver/file'

const j = Jsonic.make().use(MultiSource, {
  resolver: makeFileResolver(),
  path: '/path/to/base',
})

j('@"config.jsonic"')
```

The `path` option sets the base directory for relative references.


### Merge a reference into the surrounding map

A reference at pair-level splices every key from the referenced map into the
parent:

```ts
// a.jsonic contains: a:1 b:2
j('{@a.jsonic, c:3}')
// => { a: 1, b: 2, c: 3 }
```


### Omit the extension (implicit extensions)

By default, `@foo` is tried against `.jsonic`, `.jsc`, `.json`, `.js` (in
that order) and against `foo/index.<ext>`:

```ts
// 'g/index.jsc' contains: g:6
j('g:@g')
// => { g: { g: 6 } }
```


## How-to guides

### Configure a custom resolver

A resolver is any function matching the `Resolver` signature. It receives a
`PathSpec` and must return a `Resolution`:

```ts
import { Resolver } from '@jsonic/multisource'

const httpResolver: Resolver = (spec, _popts, _rule, _ctx, _jsonic) => ({
  ...spec,
  src: fetchSync(spec.full),
  found: true,
})
```

### Register a processor for a new file kind

Processors map a resolved source string to a value. Pick them by kind
(extension without the dot):

```ts
import { MultiSource } from '@jsonic/multisource'

Jsonic.make().use(MultiSource, {
  resolver: makeFileResolver(),
  processor: {
    yaml: (res) => { res.val = YAML.parse(res.src) },
  },
})
```

### Use a custom mark character

If `@` collides with your syntax, pick another single character:

```ts
Jsonic.make().use(MultiSource, { resolver, markchar: '$' })
```

### Resolve from an installed npm package

Use the `pkg` resolver to reference files inside installed packages. Virtual
filesystems (`ctx.meta.fs`) are honoured.

```ts
import { makePkgResolver } from '@jsonic/multisource/resolver/pkg'

const j = Jsonic.make().use(MultiSource, {
  resolver: makePkgResolver(),
})

j('@"some-pkg/config.jsonic"')
```


## Explanation

### How multisource parses a reference

The plugin installs a Jsonic directive keyed to the mark character. When the
parser reaches the mark in a value or pair context, it hands control to the
multisource directive, which:

1. Reads the path specification (a string or an object with a `path` key).
2. Passes the spec and the current options to the configured **resolver**.
   The resolver returns a `Resolution` containing the loaded source text
   plus its detected kind.
3. Looks up a **processor** for that kind (falling back to the default
   processor) and asks it to transform the source into a value.
4. Splices the value into the parse tree — as a single value, or by
   merging keys when the reference appears alone inside a map pair.

### Resolution of implicit extensions

When a reference has no explicit extension, the resolver walks the
`implictExt` list and, for each extension, checks both `path + ext` and
`path/index + ext`. The first existing source wins; the detected kind
determines which processor is used.

### Directive-level grammar

multisource registers three grammar tweaks under the `multisource` group
tag (via the `grammar` method's setting argument):

- `val`: recognise a mark at map depth 0 and push into a map.
- `map`: stop an inner map when a new mark appears.
- `pair`: close the current pair so the mark begins a new one.

These rules are what let references appear mid-map, at the top level, or
as the sole content of a pair.


## Reference

### Plugin

```ts
import MultiSource, { MultiSourceOptions } from '@jsonic/multisource'

Jsonic.make().use(MultiSource, options: MultiSourceOptions)
```

### `MultiSourceOptions`

| Field         | Type                          | Default                                    | Purpose                           |
| ------------- | ----------------------------- | ------------------------------------------ | --------------------------------- |
| `resolver`    | `Resolver`                    | required                                   | Resolves paths to source content. |
| `path`        | `string`                      | —                                          | Base path prefix for references.  |
| `markchar`    | `string`                      | `'@'`                                      | Directive open character.         |
| `processor`   | `{ [kind]: Processor }`       | default set (`json`, `jsonic`, `jsc`, `js`)| Per-kind source transformers.     |
| `implictExt`  | `string[]`                    | `['.jsonic','.jsc','.json','.js']`         | Extensions tried when omitted.    |

### Resolvers

| Export               | Module                                  | Notes                              |
| -------------------- | --------------------------------------- | ---------------------------------- |
| `makeMemResolver`    | `@jsonic/multisource/resolver/mem`      | In-memory map of path → content.   |
| `makeFileResolver`   | `@jsonic/multisource/resolver/file`     | Reads from `node:fs` (or `ctx.meta.fs`). |
| `makePkgResolver`    | `@jsonic/multisource/resolver/pkg`      | Resolves via `node_modules`.       |

### Processors

| Export                     | Module                                 | Handles                  |
| -------------------------- | -------------------------------------- | ------------------------ |
| `makeJsonicProcessor`      | `@jsonic/multisource/processor/jsonic` | `.jsonic`, `.jsc`        |
| `makeJavaScriptProcessor`  | `@jsonic/multisource/processor/js`     | `.js` (opt-in `eval`)    |

JSON kinds are handled by a built-in processor and do not require a separate
import.

### Types

```ts
type PathSpec = {
  kind: string
  path?: string
  full?: string
  base?: string
  abs: boolean
}

type Resolution = PathSpec & {
  src?: string
  val?: any
  found: boolean
  search?: string[]
}

type Resolver = (
  spec: PathSpec,
  popts: MultiSourceOptions,
  rule: Rule,
  ctx: Context,
  jsonic: Jsonic,
) => Resolution

type Processor = (
  res: Resolution,
  popts: MultiSourceOptions,
  rule: Rule,
  ctx: Context,
  jsonic: Jsonic,
) => void
```
