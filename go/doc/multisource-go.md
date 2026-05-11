# multisource plugin for Jsonic (Go)

The Go `multisource` package loads partial values from external sources
(files, in-memory maps, ...) while parsing Jsonic input. A directive
character (`@` by default) marks a reference; the package resolves it,
processes the resolved text, and splices the result into the parse output.


## Installation

```sh
go get github.com/jsonicjs/multisource/go
```

Imports:

```go
import (
    multisource "github.com/jsonicjs/multisource/go"
    jsonic "github.com/jsonicjs/jsonic/go"
)
```


## Tutorials

### Parse a reference to an in-memory source

`MakeMemResolver` builds a resolver over a `path → content` map:

```go
files := map[string]string{"a.jsonic": "{a:1}"}
j := multisource.MakeJsonic(multisource.MultiSourceOptions{
    Resolver: multisource.MakeMemResolver(files),
})
out, _ := j.Parse(`{x: @a.jsonic}`)
// out == map[string]any{"x": map[string]any{"a": float64(1)}}
```

### Parse and merge multiple references

References can sit beside regular pairs and are merged when they appear
alone in a map:

```go
files := map[string]string{
    "a.jsonic": "{a:1}",
    "b.jsonic": "{b:2}",
}
j := multisource.MakeJsonic(multisource.MultiSourceOptions{
    Resolver: multisource.MakeMemResolver(files),
})
out, _ := j.Parse(`{@a.jsonic, @b.jsonic, c:3}`)
// out == map[string]any{"a": float64(1), "b": float64(2), "c": float64(3)}
```

### Omit the extension (implicit extensions)

By default, `@foo` is tried as `foo.jsonic`, `foo.jsc`, `foo.json`, and
`foo/index.<ext>`. The first match wins:

```go
files := map[string]string{"a.jsonic": "{a:1}"}
j := multisource.MakeJsonic(multisource.MultiSourceOptions{
    Resolver: multisource.MakeMemResolver(files),
})
out, _ := j.Parse(`{x: @a}`)
// out == map[string]any{"x": map[string]any{"a": float64(1)}}
```


## How-to guides

### Supply a custom resolver

Implement the `Resolver` function type. It must populate `Resolution.Found`
and — if found — `Src` and `Full`:

```go
httpResolver := func(spec multisource.PathSpec, _ *multisource.MultiSourceOptions) multisource.Resolution {
    body := httpGet(spec.Full)
    return multisource.Resolution{
        PathSpec: spec,
        Src:      body,
        Found:    body != "",
    }
}
j := multisource.MakeJsonic(multisource.MultiSourceOptions{Resolver: httpResolver})
```

### Register a processor for a new file kind

Processors fill in `res.Val` from `res.Src`. Register them under the kind
(extension without the dot):

```go
j := multisource.MakeJsonic(multisource.MultiSourceOptions{
    Resolver: multisource.MakeMemResolver(files),
    Processor: map[string]multisource.Processor{
        "yaml": func(res *multisource.Resolution, _ *multisource.MultiSourceOptions, _ *jsonic.Jsonic) {
            res.Val = parseYAML(res.Src)
        },
    },
})
```

### Use a different mark character

```go
j := multisource.MakeJsonic(multisource.MultiSourceOptions{
    Resolver: multisource.MakeMemResolver(files),
    MarkChar: "$",
})
```

### Set a base path for relative references

```go
j := multisource.MakeJsonic(multisource.MultiSourceOptions{
    Resolver: multisource.MakeMemResolver(files),
    Path:     "configs",
})
// @a.jsonic now resolves against "configs/a.jsonic"
```


## Explanation

### How the plugin works

`MakeJsonic` creates a Jsonic instance, applies default options, and
installs the `MultiSource` plugin. `MultiSource` in turn uses the
`directive` package to register `@` as a directive open token, and adds
three grammar rules through `j.Grammar(...)`:

- `val`: recognise the mark and, at depth 0, push into a map.
- `map`: open a new pair when a mark appears inside a map.
- `pair`: close the current pair when a mark follows.

All added alternates share the `multisource` group tag, supplied via the
`GrammarSetting.Rule.Alt.G` option — not per-alt.

### Resolution pipeline

1. The directive action reads the reference — a string, or a map with a
   `path` key.
2. `ResolvePathSpec` normalises the string into a `PathSpec` (kind, base,
   full, abs).
3. The configured `Resolver` attempts to load the source, optionally
   trying implicit extensions and `index.<ext>` variants.
4. A `Processor` is selected from `Processor[kind]` (or the default
   processor for unknown kinds) and converts the source string to a
   Go value.
5. The value is spliced into the surrounding parse tree; at pair level,
   a map value is merged into the parent.


## Reference

### `Version`

```go
const Version = "0.1.0"
```

Go module release version.

### `MakeJsonic`

```go
func MakeJsonic(opts ...MultiSourceOptions) *jsonic.Jsonic
```

Creates a `*jsonic.Jsonic` with the `MultiSource` plugin installed and
sensible defaults applied.

### `Parse`

```go
func Parse(src string, opts ...MultiSourceOptions) (any, error)
```

Convenience wrapper around `MakeJsonic().Parse(src)`.

### `MultiSourceOptions`

| Field          | Type                       | Default                                        | Purpose                         |
| -------------- | -------------------------- | ---------------------------------------------- | ------------------------------- |
| `Resolver`     | `Resolver`                 | empty mem resolver                             | Resolves a `PathSpec` to source.|
| `Path`         | `string`                   | `""`                                           | Base path prefix.               |
| `MarkChar`     | `string`                   | `"@"`                                          | Directive open character.       |
| `Processor`    | `map[string]Processor`     | `json`, `jsonic`, `jsc`, default               | Per-kind source transformers.   |
| `ImplicitExt`  | `[]string`                 | `[".jsonic", ".jsc", ".json"]`                 | Extensions tried when omitted.  |

### Resolvers and processors

| Name                   | Kind                  |
| ---------------------- | --------------------- |
| `MakeMemResolver`      | Resolver factory      |
| `DefaultProcessor`     | Raw passthrough       |
| `JSONProcessor`        | `.json` via stdlib    |
| `JsonicProcessor`      | `.jsonic`, `.jsc`     |

### Types

```go
type PathSpec struct {
    Kind string
    Path string
    Full string
    Base string
    Abs  bool
}

type Resolution struct {
    PathSpec
    Src    string
    Val    any
    Found  bool
    Search []string
}

type Resolver func(spec PathSpec, opts *MultiSourceOptions) Resolution

type Processor func(res *Resolution, opts *MultiSourceOptions, j *jsonic.Jsonic)
```
