# Tutorial: your first multisource parse (Go)

This tutorial takes you from nothing to a working multisource parse in Go. By
the end you will have parsed a document that pulls a value in from a *second*
source, the whole point of the package.

You need Go 1.24+ and the `github.com/tabnas/multisource/go` module available.

## 1. Import the packages

```go
import (
    jsonic "github.com/tabnas/jsonic/go"
    tabnasmultisource "github.com/tabnas/multisource/go"
)
```

`jsonic` builds the host parser (the tabnas engine with the relaxed-JSON
grammar); `tabnasmultisource` is this package, a plugin you install on that
parser. The exported package identifier is `tabnasmultisource`.

## 2. Parse a reference to an in-memory source

The simplest resolver is the *memory* resolver: you hand it a map of
`path → content`, and `@path` references look up content in that map. No
files, no disk: ideal for a first run.

```go
files := map[string]string{
    "a.jsonic": "{a:1}",
}

j := jsonic.Make()
j.Use(tabnasmultisource.MultiSource, map[string]any{
    "resolver": tabnasmultisource.MakeMemResolver(files),
})

out, _ := j.Parse(`{x: @a.jsonic}`)
// out == map[string]any{"x": map[string]any{"a": float64(1)}}
```

`jsonic.Make()` builds the parser, and `j.Use` installs the plugin on it with
its options. The `@a.jsonic` reference was replaced by the parsed contents of
`a.jsonic`. Numbers come back as `float64`, the jsonic engine's default
numeric type.

## 3. Reference at the top level

A reference does not have to be a pair value. On its own, its keys become the
keys of the surrounding result:

```go
files := map[string]string{"a.jsonic": "{a:1}"}

j := jsonic.Make()
j.Use(tabnasmultisource.MultiSource, map[string]any{
    "resolver": tabnasmultisource.MakeMemResolver(files),
})

out, _ := j.Parse(`@a.jsonic b:2`)
// out == map[string]any{"a": float64(1), "b": float64(2)}
```

`@a.jsonic` brings in `{a:1}`, and `b:2` is added alongside it.

## 4. Drop the extension

You rarely want to spell out `.jsonic` everywhere. By default the resolver
tries implicit extensions (`.jsonic`, `.jsc`, `.json`) and also looks for an
`index` file inside a folder of that name:

```go
files := map[string]string{"g/index.jsonic": "{g:6}"}

j := jsonic.Make()
j.Use(tabnasmultisource.MultiSource, map[string]any{
    "resolver": tabnasmultisource.MakeMemResolver(files),
})

out, _ := j.Parse(`{x: @g}`)
// out == map[string]any{"x": map[string]any{"g": float64(6)}}
```

`@g` had no extension, so the resolver tried `g.jsonic`, `g.jsc`, `g.json`,
then `g/index.jsonic`, finding the index file.

A `.json` file found this way comes back as raw text unless you register a
processor for the `json` kind. The package has none built in; the
[how-to guide](./guide.md#load-json-sources) shows how to add one.

## 5. Pull in several sources at once

References can sit next to ordinary pairs and next to each other:

```go
files := map[string]string{
    "a.jsonic": "{a:1}",
    "b.jsonic": "{b:2}",
}

j := jsonic.Make()
j.Use(tabnasmultisource.MultiSource, map[string]any{
    "resolver": tabnasmultisource.MakeMemResolver(files),
})

out, _ := j.Parse(`{x: @a.jsonic, y: @b.jsonic, z: 3}`)
// out == map[string]any{
//   "x": map[string]any{"a": float64(1)},
//   "y": map[string]any{"b": float64(2)},
//   "z": float64(3),
// }
```

Two files and one inline value, merged into a single result.

## Where to go next

- [How-to guide](./guide.md). Recipes for custom kinds, base paths, merging,
  and custom resolvers.
- [Reference](./reference.md). Every exported symbol, option, and type.
- [Concepts](./concepts.md). How resolution and processing work, and how the
  Go port differs from the TypeScript original.
