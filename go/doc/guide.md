# How-to guide (Go)

Focused recipes for real tasks. Each assumes you know the basics from the
[tutorial](./tutorial.md). The package identifier is `tabnasmultisource`.

The examples build the host parser with `jsonic.Make()`, from the
`github.com/tabnas/jsonic/go` package, and install the plugin on it with
`j.Use(tabnasmultisource.MultiSource, options)`. The
[reference](./reference.md#plugin-options) lists the option keys.

## Merge a referenced map into its surroundings

When a reference is the only thing in a map, its keys are merged into the
parent map instead of nesting under a key. Compare:

```go
files := map[string]string{"a.jsonic": "{a:1, b:2}"}
j := jsonic.Make()
j.Use(tabnasmultisource.MultiSource, map[string]any{
    "resolver": tabnasmultisource.MakeMemResolver(files),
})

j.Parse(`{x: @a.jsonic}`)
// => map[string]any{"x": map[string]any{"a": float64(1), "b": float64(2)}}

j.Parse(`{@a.jsonic, c:3}`)
// => map[string]any{"a": float64(1), "b": float64(2), "c": float64(3)}
```

The merge is a deep merge, so layering one source over another keeps keys the
later source does not mention:

```go
files := map[string]string{
    "base.jsonic":     `{name:"svc", port:8080}`,
    "override.jsonic": `{port:9090}`,
}
j := jsonic.Make()
j.Use(tabnasmultisource.MultiSource, map[string]any{
    "resolver": tabnasmultisource.MakeMemResolver(files),
})

j.Parse(`{@base.jsonic, @override.jsonic}`)
// => map[string]any{"name": "svc", "port": float64(9090)}
```

`name` survives from `base.jsonic`; `port` is overwritten by `override.jsonic`.

## Register a processor for a new file kind

A *processor* turns the resolved source string (`res.Src`) into a value
(`res.Val`). The package picks one by *kind*, the file extension without the
dot. Register your own to teach multisource a new format:

```go
import (
    "strings"

    jsonic "github.com/tabnas/jsonic/go"
    tabnasmultisource "github.com/tabnas/multisource/go"
    tabnas "github.com/tabnas/parser/go"
)

csvProc := func(res *tabnasmultisource.Resolution,
    opts *tabnasmultisource.MultiSourceOptions,
    ctx *tabnas.Context, j *tabnas.Tabnas) {
    parts := make([]any, 0)
    for _, s := range strings.Split(res.Src, ",") {
        parts = append(parts, strings.TrimSpace(s))
    }
    res.Val = parts
}

j := jsonic.Make()
j.Use(tabnasmultisource.MultiSource, map[string]any{
    "resolver": tabnasmultisource.MakeMemResolver(
        map[string]string{"data.csv": "a,b,c"}),
    "processor": map[string]tabnasmultisource.Processor{
        "csv": csvProc,
    },
})

j.Parse(`{rows: @data.csv}`)
// => map[string]any{"rows": []any{"a", "b", "c"}}
```

The `processor` option adds its entries to the default processors, so kinds
you do not register keep theirs. A typed `MultiSourceOptions` passed under
`"_opts"` is different: its `Processor` map replaces the defaults, so include
the `NONE` key there, the fallback for references whose kind you have not
registered.

## Change the mark character

`@` is the default. If it collides with your data, pick another character with
`markchar`:

```go
j := jsonic.Make()
j.Use(tabnasmultisource.MultiSource, map[string]any{
    "resolver": tabnasmultisource.MakeMemResolver(
        map[string]string{"a.jsonic": "{a:1}"}),
    "markchar": "$",
})

j.Parse(`{x: $a.jsonic}`)
// => map[string]any{"x": map[string]any{"a": float64(1)}}
```

## Set a base path for relative references

`path` prefixes every relative reference. With the memory resolver this is
string concatenation against the map keys:

```go
files := map[string]string{"data/a.jsonic": "{a:1}"}
j := jsonic.Make()
j.Use(tabnasmultisource.MultiSource, map[string]any{
    "resolver": tabnasmultisource.MakeMemResolver(files),
    "path":     "data",
})

j.Parse(`{x: @a.jsonic}`)
// => map[string]any{"x": map[string]any{"a": float64(1)}}
```

Absolute references (starting with `/`) ignore the base path:

```go
files := map[string]string{"/etc/config.jsonic": `{env:"prod"}`}
j := jsonic.Make()
j.Use(tabnasmultisource.MultiSource, map[string]any{
    "resolver": tabnasmultisource.MakeMemResolver(files),
    "path":     "ignored",
})

j.Parse(`{cfg: @/etc/config.jsonic}`)
// => map[string]any{"cfg": map[string]any{"env": "prod"}}
```

## Load JSON sources

The package has no built-in processor for the `json` kind. Without one, the
value of a `.json` source is its raw text, as for any kind with no processor.
To parse it, register a processor for `json` built with the JSON parser of
your choice. This one uses Go's stdlib `encoding/json`:

```go
import (
    "encoding/json"

    jsonic "github.com/tabnas/jsonic/go"
    tabnasmultisource "github.com/tabnas/multisource/go"
    tabnas "github.com/tabnas/parser/go"
)

jsonProc := func(res *tabnasmultisource.Resolution,
    opts *tabnasmultisource.MultiSourceOptions,
    ctx *tabnas.Context, j *tabnas.Tabnas) {
    if res.Src == "" {
        res.Val = nil
        return
    }
    var val any
    if err := json.Unmarshal([]byte(res.Src), &val); err != nil {
        res.Err = err // malformed JSON fails the parse
        return
    }
    res.Val = val
}

files := map[string]string{
    "config.json": `{"host":"localhost","port":8080}`,
}
j := jsonic.Make()
j.Use(tabnasmultisource.MultiSource, map[string]any{
    "resolver": tabnasmultisource.MakeMemResolver(files),
    "processor": map[string]tabnasmultisource.Processor{
        "json": jsonProc,
    },
})

j.Parse(`{config: @config.json}`)
// => map[string]any{"config": map[string]any{
//      "host": "localhost", "port": float64(8080)}}
```

Without the `json` entry, `config` is the raw text
`{"host":"localhost","port":8080}` as a string. `.json` is still an implicit
extension, so `@config` finds `config.json` too, and the processor you
registered handles it.

## Supply a custom resolver

A `Resolver` is a function
`func(spec PathSpec, opts *MultiSourceOptions, ctx *tabnas.Context) Resolution`.
It must set `Found` and, when found, `Src` and `Full`. Use `ResolvePathSpec`
to do the shared path normalisation:

```go
var httpResolver tabnasmultisource.Resolver = func(spec tabnasmultisource.PathSpec,
    opts *tabnasmultisource.MultiSourceOptions,
    ctx *tabnas.Context) tabnasmultisource.Resolution {
    body := httpGet(spec.Full) // your own fetch
    return tabnasmultisource.Resolution{
        PathSpec: spec,
        Src:      body,
        Found:    body != "",
    }
}

j := jsonic.Make()
j.Use(tabnasmultisource.MultiSource, map[string]any{
    "resolver": httpResolver,
})
```

Declare the function with the `tabnasmultisource.Resolver` type, as here. The
plugin reads the `resolver` option as a `Resolver`, and a plain function value
of the same signature is not one, so the plugin ignores it.

The selected processor still runs on the resolution, picked from `spec.Kind`.

## Handle a missing source

A reference that resolves to nothing fails the parse with
`multisource_not_found`, listing the paths it searched:

```go
j := jsonic.Make()
j.Use(tabnasmultisource.MultiSource, map[string]any{
    "resolver": tabnasmultisource.MakeMemResolver(map[string]string{}),
})

out, err := j.Parse(`{x: @missing}`)
// out == nil
// err reports multisource_not_found ("source not found: missing")
```

## Track the dependency tree

Sources can reference other sources, forming a tree. Pass an empty
`DependencyMap` under the `deps` key of the `multisource` parse meta and the
plugin fills it with a flat map of `target → { source → Dependency }`,
recording which source pulled in which:

```go
j := jsonic.Make()
j.Use(tabnasmultisource.MultiSource, map[string]any{
    "resolver": tabnasmultisource.MakeFileResolver(),
    "path":     baseDir,
})

deps := tabnasmultisource.DependencyMap{}
out, err := j.ParseMeta(`@"app.jsonic"`, map[string]any{
    "multisource": map[string]any{"deps": deps},
})
// deps now maps each source's full path to the sources it pulled in.
// Sources referenced by the top-level parse are keyed by
// tabnasmultisource.TOP.
```

This is how you build a watch list or invalidate caches when an upstream file
changes.

## Preload files to avoid per-reference disk I/O

For large trees, scan folders into memory once with `PreloadFiles` and hand
the map to the file resolver. The resolver checks preloaded content before
touching disk:

```go
filemap := tabnasmultisource.PreloadFiles(tabnasmultisource.PreloadOptions{
    Folders:   []string{configDir},
    Ext:       []string{".jsonic", ".json"},
    Recursive: true,
})

j := jsonic.Make()
j.Use(tabnasmultisource.MultiSource, map[string]any{
    "resolver": tabnasmultisource.MakeFileResolver(
        tabnasmultisource.FileResolverOptions{Preload: filemap}),
    "path": configDir,
})

out, err := j.Parse(`@"app.jsonic"`)
// served from memory, falls back to disk if missing
```

## Use the path plugin alongside multisource

multisource composes with `github.com/tabnas/path/go`. Install it on the same
instance to track key paths through references:

```go
import path "github.com/tabnas/path/go"

j := jsonic.Make()
j.Use(tabnasmultisource.MultiSource, map[string]any{
    "resolver": tabnasmultisource.MakeMemResolver(files),
})
j.Use(path.Path, nil)
```
