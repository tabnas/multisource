# multisource

Load partial values from multiple sources (files, packages, memory) into a
single [Tabnas](https://github.com/tabnas/jsonic) parse result. A marked path
(`@a.jsonic`) is resolved, parsed, and spliced in place.


[![npm version](https://img.shields.io/npm/v/@tabnas/multisource.svg)](https://npmjs.com/package/@tabnas/multisource)
[![build](https://github.com/tabnas/multisource/actions/workflows/build.yml/badge.svg)](https://github.com/tabnas/multisource/actions/workflows/build.yml)
[![Coverage Status](https://coveralls.io/repos/github/tabnas/multisource/badge.svg?branch=main)](https://coveralls.io/github/tabnas/multisource?branch=main)
[![Known Vulnerabilities](https://snyk.io/test/github/tabnas/multisource/badge.svg)](https://snyk.io/test/github/tabnas/multisource)
[![DeepScan grade](https://deepscan.io/api/teams/5016/projects/22471/branches/663911/badge/grade.svg)](https://deepscan.io/dashboard#view=project&tid=5016&pid=22471&bid=663911)
[![Maintainability](https://api.codeclimate.com/v1/badges/eb0f99f5302e3bd37924/maintainability)](https://codeclimate.com/github/tabnas/multisource/maintainability)


| ![Voxgig](https://www.voxgig.com/res/img/vgt01r.png) | This open source module is sponsored and supported by [Voxgig](https://www.voxgig.com). |
| ---------------------------------------------------- | --------------------------------------------------------------------------------------- |


## Install

```sh
npm install @tabnas/multisource @tabnas/parser @tabnas/jsonic
```


## Tiny example

```js
import { Tabnas } from '@tabnas/parser'
import { jsonic } from '@tabnas/jsonic'
import { MultiSource } from '@tabnas/multisource'
import { makeMemResolver } from '@tabnas/multisource/resolver/mem'

const j = new Tabnas().use(jsonic).use(MultiSource, {
  resolver: makeMemResolver({ 'foo.jsonic': 'a:1' }),
})

j.parse('@"foo.jsonic" b:2')   // => { a: 1, b: 2 }
```


## Documentation

Four-quadrant [Diátaxis](https://diataxis.fr) docs:

- [Tutorial](doc/tutorial.md) — zero to a working multisource parse.
- [How-to guide](doc/guide.md) — recipes: files, custom kinds, merging,
  base paths, dependency tracking, preloading.
- [Reference](doc/reference.md) — every export, option and type.
- [Concepts](doc/concepts.md) — how it works and why; the engine relationship.

The Go port lives in [`../go`](../go/) with its own
[four-quadrant docs](../go/doc/).


## Grammar diagram

The installed grammar as a railroad/syntax diagram, generated from the live
grammar with [`@tabnas/railroad`](https://github.com/tabnas/railroad):

![multisource grammar railroad diagram](doc/grammar.svg)

A vertical ASCII version is in [`doc/grammar.txt`](doc/grammar.txt).

## License

MIT © Richard Rodger and contributors.
