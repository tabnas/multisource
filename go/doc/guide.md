# How-to guide (Go)

Focused recipes for real tasks. Each assumes you know the basics from the
[tutorial](./tutorial.md). The package identifier is `tabnasmultisource`.

## Merge a referenced map into its surroundings

When a reference is the only thing in a map, its keys are merged into the
parent map instead of nesting under a key. Compare:

```go
files := map[string]string{"a.jsonic": "{a:1, b:2}"}
j := tabnasmultisource.MakeJsonic(tabnasmultisource.MultiSourceOptions{
    Resolver: tabnasmultisource.MakeMemResolver(files),
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
j := tabnasmultisource.MakeJsonic(tabnasmultisource.MultiSourceOptions{
    Resolver: tabnasmultisource.MakeMemResolver(files),
})

j.Parse(`{@base.jsonic, @override.jsonic}`)
// => map[string]any{"name": "svc", "port": float64(9090)}
```

`name` survives from `base.jsonic`; `port` is overwritten by `override.jsonic`.

## Register a processor for a new file kind

A *processor* turns the resolved source string (`res.Src`) into a value
(`res.Val`). The package picks one by *kind* — the file extension without the
dot. Register your own to teach multisource a new format:

```go
import (
    "strings"

    tabnasmultisource "github.com/tabnas/multisource/go"
    jsonic "github.com/tabnas/jsonic/go"
)

csvProc := func(res *tabnasmultisource.Resolution,
    opts *tabnasmultisource.MultiSourceOptions,
    ctx *jsonic.Context, j *jsonic.Jsonic) {
    parts := make([]any, 0)
    for _, s := range strings.Split(res.Src, ",") {
        parts = append(parts, strings.TrimSpace(s))
    }
    res.Val = parts
}

j := tabnasmultisource.MakeJsonic(tabnasmultisource.MultiSourceOptions{
    Resolver: tabnasmultisource.MakeMemResolver(
        map[string]string{"data.csv": "a,b,c"}),
    Processor: map[string]tabnasmultisource.Processor{
        tabnasmultisource.NONE: tabnasmultisource.DefaultProcessor,
        "csv":                  csvProc,
    },
})

j.Parse(`{rows: @data.csv}`)
// => map[string]any{"rows": []any{"a", "b", "c"}}
```

When you supply a custom `Processor` map, include the `NONE` key (the default
fallback) so references whose kind you have not registered still resolve.

## Change the mark character

`@` is the default. If it collides with your data, pick another character with
`MarkChar`:

```go
j := tabnasmultisource.MakeJsonic(tabnasmultisource.MultiSourceOptions{
    Resolver: tabnasmultisource.MakeMemResolver(
        map[string]string{"a.jsonic": "{a:1}"}),
    MarkChar: "$",
})

j.Parse(`{x: $a.jsonic}`)
// => map[string]any{"x": map[string]any{"a": float64(1)}}
```

## Set a base path for relative references

`Path` prefixes every relative reference. With the memory resolver this is
string concatenation against the map keys:

```go
files := map[string]string{"data/a.jsonic": "{a:1}"}
j := tabnasmultisource.MakeJsonic(tabnasmultisource.MultiSourceOptions{
    Resolver: tabnasmultisource.MakeMemResolver(files),
    Path:     "data",
})

j.Parse(`{x: @a.jsonic}`)
// => map[string]any{"x": map[string]any{"a": float64(1)}}
```

Absolute references (starting with `/`) ignore the base path:

```go
files := map[string]string{"/etc/config.jsonic": `{env:"prod"}`}
j := tabnasmultisource.MakeJsonic(tabnasmultisource.MultiSourceOptions{
    Resolver: tabnasmultisource.MakeMemResolver(files),
    Path:     "ignored",
})

j.Parse(`{cfg: @/etc/config.jsonic}`)
// => map[string]any{"cfg": map[string]any{"env": "prod"}}
```

## Load JSON sources

`.json` references are parsed by the built-in `JSONProcessor` (Go's stdlib
`encoding/json`):

```go
files := map[string]string{
    "config.json": `{"host":"localhost","port":8080}`,
}
j := tabnasmultisource.MakeJsonic(tabnasmultisource.MultiSourceOptions{
    Resolver: tabnasmultisource.MakeMemResolver(files),
})

j.Parse(`{config: @config.json}`)
// => map[string]any{"config": map[string]any{
//      "host": "localhost", "port": float64(8080)}}
```

## Supply a custom resolver

A `Resolver` is a function
`func(spec PathSpec, opts *MultiSourceOptions, ctx *jsonic.Context) Resolution`.
It must set `Found` and, when found, `Src` and `Full`. Use `ResolvePathSpec`
to do the shared path normalisation:

```go
httpResolver := func(spec tabnasmultisource.PathSpec,
    opts *tabnasmultisource.MultiSourceOptions,
    ctx *jsonic.Context) tabnasmultisource.Resolution {
    body := httpGet(spec.Full) // your own fetch
    return tabnasmultisource.Resolution{
        PathSpec: spec,
        Src:      body,
        Found:    body != "",
    }
}

j := tabnasmultisource.MakeJsonic(tabnasmultisource.MultiSourceOptions{
    Resolver: httpResolver,
})
```

The selected processor still runs on the resolution, picked from `spec.Kind`.

## Handle a missing source

A reference that resolves to nothing produces `nil` for that value rather than
an error:

```go
j := tabnasmultisource.MakeJsonic(tabnasmultisource.MultiSourceOptions{
    Resolver: tabnasmultisource.MakeMemResolver(map[string]string{}),
})

out, err := j.Parse(`{x: @missing}`)
// err == nil
// out == map[string]any{"x": nil}
```

## Track the dependency tree

Sources can reference other sources, forming a tree. Pass an empty
`DependencyMap` under the `deps` key of the `multisource` parse meta and the
plugin fills it with a flat map of `target → { source → Dependency }`,
recording which source pulled in which:

```go
j := tabnasmultisource.MakeJsonic(tabnasmultisource.MultiSourceOptions{
    Resolver: tabnasmultisource.MakeFileResolver(),
    Path:     baseDir,
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

j := tabnasmultisource.MakeJsonic(tabnasmultisource.MultiSourceOptions{
    Resolver: tabnasmultisource.MakeFileResolver(
        tabnasmultisource.FileResolverOptions{Preload: filemap}),
    Path: configDir,
})

out, err := j.Parse(`@"app.jsonic"`)
// served from memory, falls back to disk if missing
```

## Use the path plugin alongside multisource

multisource composes with `github.com/tabnas/path/go`. Install it on the same
instance to track key paths through references:

```go
import path "github.com/tabnas/path/go"

j := tabnasmultisource.MakeJsonic(tabnasmultisource.MultiSourceOptions{
    Resolver: tabnasmultisource.MakeMemResolver(files),
})
j.Use(path.Path, nil)
```
