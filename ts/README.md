# multisource

Load partial values from multiple sources (files, packages, memory) into a
single [Jsonic](https://jsonic.senecajs.org) parse result.


[![npm version](https://img.shields.io/npm/v/@jsonic/multisource.svg)](https://npmjs.com/package/@jsonic/multisource)
[![build](https://github.com/jsonicjs/multisource/actions/workflows/build.yml/badge.svg)](https://github.com/jsonicjs/multisource/actions/workflows/build.yml)
[![Coverage Status](https://coveralls.io/repos/github/jsonicjs/multisource/badge.svg?branch=main)](https://coveralls.io/github/jsonicjs/multisource?branch=main)
[![Known Vulnerabilities](https://snyk.io/test/github/jsonicjs/multisource/badge.svg)](https://snyk.io/test/github/jsonicjs/multisource)
[![DeepScan grade](https://deepscan.io/api/teams/5016/projects/22471/branches/663911/badge/grade.svg)](https://deepscan.io/dashboard#view=project&tid=5016&pid=22471&bid=663911)
[![Maintainability](https://api.codeclimate.com/v1/badges/eb0f99f5302e3bd37924/maintainability)](https://codeclimate.com/github/jsonicjs/multisource/maintainability)


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

import { Jsonic } from 'jsonic'
import MultiSource from '@jsonic/multisource'
import { makeFileResolver } from '@jsonic/multisource/resolver/file'

const j = Jsonic.make().use(MultiSource, {
  resolver: makeFileResolver(),
})

j('@"foo.jsonic" b:2')
// => { a: 1, b: 2 }
```

```go
import (
    jsonic "github.com/jsonicjs/jsonic/go"
    multisource "github.com/jsonicjs/multisource/go"
)

files := map[string]string{"foo.jsonic": "a:1"}
j := multisource.MakeJsonic(multisource.MultiSourceOptions{
    Resolver: multisource.MakeMemResolver(files),
})
out, _ := j.Parse(`{@foo.jsonic, b:2}`)
// => map[a:1 b:2]
```


## License

MIT © Richard Rodger and contributors.
