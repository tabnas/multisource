# How-to guide

Focused recipes for real tasks. Each assumes you already know the basics from
the [tutorial](./tutorial.md). Examples use the `@tabnas/parser` engine plus
the `jsonic` grammar; swap in whatever resolver the task needs.

## Read references from files on disk

Use the *file* resolver and give it a base `path` (the folder that relative
references are resolved against). Pass `multisource.path` in the parse meta to
set the base per parse, or set the `path` option once on the plugin.

```js ignore
import { Tabnas } from '@tabnas/parser'
import { jsonic } from '@tabnas/jsonic'
import { MultiSource } from '@tabnas/multisource'
import { makeFileResolver } from '@tabnas/multisource/resolver/file'

const j = new Tabnas().use(jsonic).use(MultiSource, {
  resolver: makeFileResolver(),
  path: '/path/to/configs',
})

j.parse('@"app.jsonic"')         // reads /path/to/configs/app.jsonic
j.parse('@"app"')                // tries app.jsonic, app.jsc, app.json, app.js
```

Quote the reference (`@"app.jsonic"`) when the path contains a dot or slash,
so the grammar treats it as one string.

If you do not set the `path` option, supply the base directory per parse:

```js ignore
j.parse('@"app.jsonic"', { multisource: { path: __dirname } })
```

The file resolver reads through `node:fs` by default. To read from a virtual
filesystem (for tests, or sandboxing), pass an `fs` object in the parse meta;
the resolver uses `ctx.meta.fs` when present:

```js ignore
import { memfs } from 'memfs'
const { fs } = memfs({ 'b.jsonic': '2' })
j.parse('a:1 b:@"/b.jsonic"', { fs })   // => { a: 1, b: 2 }
```

## Merge a referenced map into its surroundings

When a reference is the only thing in a map pair, its keys are merged into the
parent map instead of nesting under a key. Compare these two forms:

```js
import { Tabnas } from '@tabnas/parser'
import { jsonic } from '@tabnas/jsonic'
import { MultiSource } from '@tabnas/multisource'
import { makeMemResolver } from '@tabnas/multisource/resolver/mem'

const j = new Tabnas().use(jsonic).use(MultiSource, {
  resolver: makeMemResolver({ 'a.jsonic': 'a:1 b:2' }),
})

// Nested under the key x:
j.parse('{x:@a.jsonic}')    // => { x: { a: 1, b: 2 } }
// Merged in place into the parent map:
j.parse('{@a.jsonic, c:3}') // => { a: 1, b: 2, c: 3 }
```

The merge is a deep merge, so layering one source over another keeps keys the
later source does not mention:

```js
import { Tabnas } from '@tabnas/parser'
import { jsonic } from '@tabnas/jsonic'
import { MultiSource } from '@tabnas/multisource'
import { makeMemResolver } from '@tabnas/multisource/resolver/mem'

const j = new Tabnas().use(jsonic).use(MultiSource, {
  resolver: makeMemResolver({
    'base.jsonic': 'name:"svc" port:8080',
    'override.jsonic': 'port:9090',
  }),
})

j.parse('{@base.jsonic, @override.jsonic}')   // => { name: 'svc', port: 9090 }
```

`name` survives from `base.jsonic`; `port` is overwritten by `override.jsonic`.

## Register a processor for a new file kind

A *processor* turns the resolved source string into a value. The plugin picks
one by *kind* — the file extension without the dot. Register your own to teach
multisource a new format:

```js
import { Tabnas } from '@tabnas/parser'
import { jsonic } from '@tabnas/jsonic'
import { MultiSource } from '@tabnas/multisource'
import { makeMemResolver } from '@tabnas/multisource/resolver/mem'

const j = new Tabnas().use(jsonic).use(MultiSource, {
  resolver: makeMemResolver({ 'data.csv': 'a,b,c' }),
  processor: {
    csv: (res) => { res.val = res.src.split(',') },
  },
})

j.parse('rows:@data.csv')   // => { rows: ['a', 'b', 'c'] }
```

A processor receives the `Resolution` and assigns the parsed value to
`res.val`. The remaining arguments (`popts, rule, ctx, tn`) are available if
you need the parser engine — for example to recursively parse the source.

### Alias one kind to another

If you supply a string instead of a function, it aliases to another kind's
processor. This is the easy way to say "treat `.conf` files as jsonic":

```js
import { Tabnas } from '@tabnas/parser'
import { jsonic } from '@tabnas/jsonic'
import { MultiSource } from '@tabnas/multisource'
import { makeMemResolver } from '@tabnas/multisource/resolver/mem'

const j = new Tabnas().use(jsonic).use(MultiSource, {
  resolver: makeMemResolver({ 'app.conf': 'a:1' }),
  processor: { conf: 'jsonic' },
})

j.parse('cfg:@app.conf')   // => { cfg: { a: 1 } }
```

## Change the mark character

`@` is the default mark. If it collides with your data, pick another single
character with `markchar`:

```js
import { Tabnas } from '@tabnas/parser'
import { jsonic } from '@tabnas/jsonic'
import { MultiSource } from '@tabnas/multisource'
import { makeMemResolver } from '@tabnas/multisource/resolver/mem'

const j = new Tabnas().use(jsonic).use(MultiSource, {
  resolver: makeMemResolver({ 'a.jsonic': 'a:1' }),
  markchar: '$',
})

j.parse('x:$a.jsonic')   // => { x: { a: 1 } }
```

## Set a base path for relative references

The `path` option prefixes every relative reference. With the memory resolver
this is just string concatenation against the map keys:

```js
import { Tabnas } from '@tabnas/parser'
import { jsonic } from '@tabnas/jsonic'
import { MultiSource } from '@tabnas/multisource'
import { makeMemResolver } from '@tabnas/multisource/resolver/mem'

const j = new Tabnas().use(jsonic).use(MultiSource, {
  resolver: makeMemResolver({ 'configs/a.jsonic': 'a:1' }),
  path: 'configs',
})

j.parse('x:@a.jsonic')   // => { x: { a: 1 } }
```

Absolute references (starting with `/`) ignore the base path.

## Pass the reference as an object

A reference can be an object with a `path` key instead of a bare string. This
is useful when you generate references programmatically or want to attach more
fields in a custom resolver:

```js
import { Tabnas } from '@tabnas/parser'
import { jsonic } from '@tabnas/jsonic'
import { MultiSource } from '@tabnas/multisource'
import { makeMemResolver } from '@tabnas/multisource/resolver/mem'

const j = new Tabnas().use(jsonic).use(MultiSource, {
  resolver: makeMemResolver({ 'a.jsonic': 'a:1' }),
})

j.parse('x:@{path:"a.jsonic"}')   // => { x: { a: 1 } }
```

## Track the dependency tree

Sources can reference other sources, forming a tree. Pass an empty `deps`
object in `multisource` meta and the plugin fills it with a flat map of
`target → { source → dependency }`, recording which file pulled in which:

```js ignore
import { Tabnas } from '@tabnas/parser'
import { jsonic } from '@tabnas/jsonic'
import { MultiSource } from '@tabnas/multisource'
import { makeFileResolver } from '@tabnas/multisource/resolver/file'

const j = new Tabnas().use(jsonic).use(MultiSource, {
  resolver: makeFileResolver(),
})

const deps = {}
j.parse('@"app.jsonic"', { multisource: { path: __dirname, deps } })
// `deps` now maps each source's full path to the sources it pulled in.
```

This is how you build a watch list or invalidate caches when an upstream file
changes.

## Preload files to avoid per-reference disk I/O

For large trees, scan folders into memory once and hand the map to the file
resolver. The resolver checks preloaded content before touching disk:

```js ignore
import { Tabnas } from '@tabnas/parser'
import { jsonic } from '@tabnas/jsonic'
import { MultiSource, preloadFiles } from '@tabnas/multisource'
import { makeFileResolver } from '@tabnas/multisource/resolver/file'

const filemap = preloadFiles({
  folders: [__dirname + '/configs'],
  ext: ['.jsonic', '.json'],
  recursive: true,
})

const j = new Tabnas().use(jsonic).use(MultiSource, {
  resolver: makeFileResolver({ preload: filemap }),
  path: __dirname + '/configs',
})

j.parse('@"app.jsonic"')   // served from memory, falls back to disk if missing
```

## Resolve from an installed npm package

Use the *pkg* resolver to reference files inside installed packages. It tries
`require.resolve` first, then walks `node_modules`, then the filesystem (so it
works with virtual filesystems too):

```js ignore
import { Tabnas } from '@tabnas/parser'
import { jsonic } from '@tabnas/jsonic'
import { MultiSource } from '@tabnas/multisource'
import { makePkgResolver } from '@tabnas/multisource/resolver/pkg'

const j = new Tabnas().use(jsonic).use(MultiSource, {
  resolver: makePkgResolver({ require }),
})

j.parse('@"some-pkg/config.jsonic"')
```

Pass `require: ['/some/path']` or `require: '/some/path'` to constrain the
search to specific `node_modules` roots.
