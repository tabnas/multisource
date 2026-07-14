# multisource (Go)

The Go port of [`@tabnas/multisource`](../ts/). Merges multiple sources into a
single jsonic parse result: a marked path (`@a.jsonic`) is resolved, parsed,
and spliced in place. The TypeScript implementation is canonical; this package
tracks it.

Package identifier: `tabnasmultisource`.


## Install

```sh
go get github.com/tabnas/multisource/go
```


## Tiny example

```go
import (
    tabnasmultisource "github.com/tabnas/multisource/go"
)

files := map[string]string{"foo.jsonic": "{a:1}"}
j := tabnasmultisource.MakeJsonic(tabnasmultisource.MultiSourceOptions{
    Resolver: tabnasmultisource.MakeMemResolver(files),
})
out, _ := j.Parse(`@foo.jsonic b:2`)
// out == map[string]any{"a": float64(1), "b": float64(2)}
```


## Documentation

Four-quadrant [Diátaxis](https://diataxis.fr) docs:

- [Tutorial](doc/tutorial.md) — zero to a working multisource parse.
- [How-to guide](doc/guide.md) — recipes: custom kinds, merging, base paths,
  custom resolvers.
- [Reference](doc/reference.md) — every exported symbol, option and type.
- [Concepts](doc/concepts.md) — how it works, and how the Go port differs from
  the TypeScript version.


## Differences from the TypeScript implementation

The TypeScript package is canonical and the Go port tracks it, with one
deliberate gap: **`.js` sources are not supported in Go**. In TypeScript,
`makeJavaScriptProcessor` loads a `.js` reference by executing the JavaScript
module (`require(...)`) and splicing in its exports — Go has no JavaScript
runtime, so this cannot be ported. A `@foo.js` reference in Go falls through to
the default raw-string processor, and `.js` is not among the default implicit
extensions. (`.jsc` files are jsonic content, not JavaScript, and are fully
supported.) Use `.jsonic`/`.jsc`/`.json` sources, or register a custom
`Processor` if you embed your own interpreter.

The full list of behavioural and idiom differences is in
[Concepts — Differences from the TypeScript implementation](doc/concepts.md#differences-from-the-typescript-implementation).


## Grammar diagram

The grammar as a railroad/syntax diagram (shared with the TS implementation):

![multisource grammar railroad diagram](../ts/doc/grammar.svg)

A vertical ASCII version is in [`../ts/doc/grammar.txt`](../ts/doc/grammar.txt).

## License

MIT © Richard Rodger and contributors.
