# Tutorial: your first multisource parse

This tutorial takes you from nothing to a working multisource parse. By the
end you will have parsed a document that pulls a value in from a *second*
source — the whole point of the plugin.

You need Node.js 24+ and a folder where you can install npm packages.

## 1. Install

```sh
npm install @tabnas/multisource @tabnas/parser @tabnas/jsonic
```

`@tabnas/parser` is the parser engine, `@tabnas/jsonic` supplies the
relaxed-JSON grammar (so you can write `a:1` instead of `{"a":1}`), and
`@tabnas/multisource` is this plugin.

## 2. Parse a reference to an in-memory source

The simplest resolver is the *memory* resolver: you hand it a map of
`path → content`, and `@path` references look up content in that map. No
files, no disk — ideal for a first run.

```js
import { Tabnas } from '@tabnas/parser'
import { jsonic } from '@tabnas/jsonic'
import { MultiSource } from '@tabnas/multisource'
import { makeMemResolver } from '@tabnas/multisource/resolver/mem'

const j = new Tabnas().use(jsonic).use(MultiSource, {
  resolver: makeMemResolver({
    'a.jsonic': 'a:1',
  }),
})

j.parse('x:@a.jsonic, y:2')   // => { x: { a: 1 }, y: 2 }
```

Read that result carefully. The `@a.jsonic` reference was replaced by the
parsed contents of `a.jsonic` (which is `{ a: 1 }`), and the rest of the
document — `y:2` — parsed normally. You merged two sources into one result.

## 3. Reference at the top level

A reference does not have to be a pair value. If `@a.jsonic` appears on its
own, its keys become the keys of the surrounding result:

```js
import { Tabnas } from '@tabnas/parser'
import { jsonic } from '@tabnas/jsonic'
import { MultiSource } from '@tabnas/multisource'
import { makeMemResolver } from '@tabnas/multisource/resolver/mem'

const j = new Tabnas().use(jsonic).use(MultiSource, {
  resolver: makeMemResolver({ 'a.jsonic': 'a:1' }),
})

j.parse('@a.jsonic b:2')   // => { a: 1, b: 2 }
```

This is the headline form: `@a.jsonic` brings in `{ a: 1 }`, and `b:2` is
added alongside it.

## 4. Drop the extension

You rarely want to spell out `.jsonic` everywhere. By default the resolver
tries a list of implicit extensions (`.jsonic`, `.jsc`, `.json`, `.js`) and
also looks for an `index` file inside a folder of that name:

```js
import { Tabnas } from '@tabnas/parser'
import { jsonic } from '@tabnas/jsonic'
import { MultiSource } from '@tabnas/multisource'
import { makeMemResolver } from '@tabnas/multisource/resolver/mem'

const j = new Tabnas().use(jsonic).use(MultiSource, {
  resolver: makeMemResolver({ 'g/index.jsc': 'g:6' }),
})

j.parse('g:@g')   // => { g: { g: 6 } }
```

Here `@g` had no extension, so the resolver tried `g.jsonic`, `g.jsc`, … and
then `g/index.jsonic`, `g/index.jsc`, finding `g/index.jsc`.

## 5. Pull in several sources at once

The real value appears when one document composes several. References can sit
next to ordinary pairs and next to each other:

```js
import { Tabnas } from '@tabnas/parser'
import { jsonic } from '@tabnas/jsonic'
import { MultiSource } from '@tabnas/multisource'
import { makeMemResolver } from '@tabnas/multisource/resolver/mem'

const j = new Tabnas().use(jsonic).use(MultiSource, {
  resolver: makeMemResolver({
    'a.jsonic': 'a:1',
    'b.jsonic': 'b:2',
  }),
})

j.parse('x:@a.jsonic, y:@b.jsonic, z:3')   // => { x: { a: 1 }, y: { b: 2 }, z: 3 }
```

Two files and one inline value, merged into a single result.

## Where to go next

- [How-to guide](./guide.md) — focused recipes: reading from disk, custom
  source kinds, base paths, dependency tracking, preloading.
- [Reference](./reference.md) — every export, option and type.
- [Concepts](./concepts.md) — how resolution and processing actually work,
  and how the plugin relates to the parser engine.
