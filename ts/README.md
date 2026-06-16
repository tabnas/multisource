# multisource

Load partial values from multiple sources (files, packages, memory) into a
single [Jsonic](https://jsonic.senecajs.org) parse result.


[![npm version](https://img.shields.io/npm/v/@tabnas/multisource.svg)](https://npmjs.com/package/@tabnas/multisource)
[![build](https://github.com/tabnas/multisource/actions/workflows/build.yml/badge.svg)](https://github.com/tabnas/multisource/actions/workflows/build.yml)
[![Coverage Status](https://coveralls.io/repos/github/tabnas/multisource/badge.svg?branch=main)](https://coveralls.io/github/tabnas/multisource?branch=main)
[![Known Vulnerabilities](https://snyk.io/test/github/tabnas/multisource/badge.svg)](https://snyk.io/test/github/tabnas/multisource)
[![DeepScan grade](https://deepscan.io/api/teams/5016/projects/22471/branches/663911/badge/grade.svg)](https://deepscan.io/dashboard#view=project&tid=5016&pid=22471&bid=663911)
[![Maintainability](https://api.codeclimate.com/v1/badges/eb0f99f5302e3bd37924/maintainability)](https://codeclimate.com/github/tabnas/multisource/maintainability)


| ![Voxgig](https://www.voxgig.com/res/img/vgt01r.png) | This open source module is sponsored and supported by [Voxgig](https://www.voxgig.com). |
| ---------------------------------------------------- | --------------------------------------------------------------------------------------- |


## Documentation

Documentation for both language implementations follows the
[Diátaxis](https://diataxis.fr) framework (Tutorials, How-to guides,
Explanation, Reference).

- TypeScript: [`doc/multisource-ts.md`](doc/multisource-ts.md)
- Go: [`doc/multisource-go.md`](doc/multisource-go.md)


## Quick Example

```ts
// file: foo.jsonic
//   a:1

import { Tabnas } from '@tabnas/parser'
import { jsonic } from '@tabnas/jsonic'
import { MultiSource } from '@tabnas/multisource'
import { makeFileResolver } from '@tabnas/multisource/resolver/file'

const j = new Tabnas().use(jsonic).use(MultiSource, {
  resolver: makeFileResolver(),
})

j.parse('@"foo.jsonic" b:2')
// => { a: 1, b: 2 }
```

```go
import (
    tabnas "github.com/tabnas/jsonic/go"
    multisource "github.com/tabnas/multisource/go"
)

files := map[string]string{"foo.jsonic": "a:1"}
j := multisource.MakeJsonic(multisource.MultiSourceOptions{
    Resolver: multisource.MakeMemResolver(files),
})
out, _ := j.Parse(`{@foo.jsonic, b:2}`)
// => map[a:1 b:2]
```



## Grammar diagram

The installed grammar as a railroad/syntax diagram, generated from the live
grammar with [`@tabnas/railroad`](https://github.com/tabnas/railroad):

![multisource grammar railroad diagram](doc/grammar.svg)

A vertical ASCII version is in [`doc/grammar.txt`](doc/grammar.txt).

## License

MIT © Richard Rodger and contributors.
