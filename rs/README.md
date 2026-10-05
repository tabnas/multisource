# tabnas-multisource (Rust)

Load partial values from multiple external sources into one parse
result, as a plugin for the [`tabnas`](https://github.com/tabnas/parser)
parsing engine. It installs on a host grammar you build, such as the
[`tabnas-jsonic`](https://github.com/tabnas/jsonic) relaxed-JSON grammar.
Crate `tabnas_multisource`.

A directive character (`@` by default) marks a reference in the input.
The plugin **resolves** the reference to a source, **processes** that
source into a value, and splices the value into the output, recursively,
so a loaded source can reference more sources. `x:@a.jsonic` sets `x` to
whatever `a.jsonic` parses to; `{@foo}` merges the loaded map's keys into
the enclosing object.

This is the Rust port of the canonical TypeScript implementation in
[`../ts`](../ts); the TypeScript version is authoritative and this crate
tracks it. The Go port is in [`../go`](../go). Differences are recorded
in [`../DIVERGENCE.md`](../DIVERGENCE.md).

## Use

Build the host parser, then install the plugin on it. The plugin
modifies the `val`, `map` and `pair` rules rather than defining a value
grammar, so the host grammar must supply them; a jsonic parser does. On
a bare engine the plugin refuses to install, rather than registering a
directive that could never match.

```rust
use tabnas_multisource::{multisource, MapResolver, MultiSourceOptions};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut parser = tabnas_jsonic::make();
    let sources = MapResolver::from([("a.jsonic", "a:1")]);
    multisource(&mut parser, MultiSourceOptions::new(sources))?;

    assert_eq!(parser.parse(r#"@"a.jsonic" b:2"#)?.to_string(), r#"{"a":1,"b":2}"#);
    assert_eq!(parser.parse(r#"x:@"a.jsonic""#)?.to_string(), r#"{"x":{"a":1}}"#);
    Ok(())
}
```

Build the instance once and reuse it. Building the engine, the host
grammar and this plugin's own alternates dominates a small parse, and
`tests/perf_test.rs` measures the difference.

`plugin_with(options)` returns the same plugin as a `Plugin` value, for
a caller that installs it through `Tabnas::use_plugin` itself.

## Resolvers

A resolver is the only thing a reference can reach, so it is also the
sandbox. Three come with the crate.

`MapResolver` serves a map of path to content, and nothing else:

```rust
use tabnas_multisource::{multisource, MapResolver, MultiSourceOptions};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut parser = tabnas_jsonic::make();
    multisource(
        &mut parser,
        MultiSourceOptions::new(MapResolver::from([
            ("conf/base.jsonic", "port:8080"),
            ("conf/app.jsonic", r#"@"base.jsonic" name:"app""#),
        ])),
    )?;
    assert_eq!(
        parser.parse(r#"@"conf/app.jsonic""#)?.to_string(),
        r#"{"port":8080,"name":"app"}"#
    );
    Ok(())
}
```

`FileResolver` reads a filesystem. Which filesystem is a typed option, so
a test or a sandboxed caller can hand it a map and no disk is touched:

```rust
use tabnas_multisource::{multisource, FileResolver, MapFs, MultiSourceOptions};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let disk = MapFs::from([
        ("app/main.jsonic", r#"{host:"localhost", port:@"./port.jsonic"}"#),
        ("app/port.jsonic", "8080"),
    ]);
    let mut parser = tabnas_jsonic::make();
    multisource(
        &mut parser,
        MultiSourceOptions::new(FileResolver::new()).with_fs(disk),
    )?;
    assert_eq!(
        parser.parse(r#"@"./app/main.jsonic""#)?.to_string(),
        r#"{"host":"localhost","port":8080}"#
    );
    Ok(())
}
```

`PkgResolver` walks `node_modules` folders, honouring a package's
`package.json` `main` for a bare reference. Node's `require.resolve` has
no Rust counterpart, so this is the portable subset the Go port also
implements; conditional `exports` are not covered.

A closure is a resolver too, so a caller needs no type of their own:

```rust
use tabnas_multisource::{
    multisource, resolve_path_spec, MultiSourceOptions, Resolution, ResolverInput,
};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let resolver = |reference: Option<&str>, _input: &ResolverInput<'_>| match reference {
        Some("live") => Resolution::found(
            resolve_path_spec(Some("live"), ""),
            "live.jsonic",
            "now:1",
        ),
        other => Resolution::not_found(resolve_path_spec(other, "")),
    };
    let mut parser = tabnas_jsonic::make();
    multisource(&mut parser, MultiSourceOptions::new(resolver))?;
    assert_eq!(parser.parse("x:@live")?.to_string(), r#"{"x":{"now":1}}"#);
    Ok(())
}
```

## Kinds and processors

The extension of a reference's last path segment selects a processor.
The defaults are the TypeScript ones: `jsonic` and `jsc` re-parse with
the live engine, and anything else is the raw text. A reference with no
extension is tried against the implicit extensions (`.jsonic`, `.jsc`
and `.json`) and then against a folder index file.

There is no built-in `json` processor, so a `.json` source is raw text
until you register one, built with the JSON parser of your choice. Here
it is jsonic's strict-JSON parser; a processor that fails puts its error
in `resolution.err`, and the whole parse then fails with it:

```rust
use tabnas_multisource::{
    multisource, MapResolver, MultiSourceOptions, ProcessorInput, Resolution,
};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let strict = tabnas_jsonic::make_json();
    let json = move |resolution: &mut Resolution, _input: &ProcessorInput<'_>| {
        if let Some(src) = &resolution.src {
            match strict.parse(src) {
                Ok(value) => resolution.val = value,
                Err(error) => resolution.err = Some(Box::new(error)),
            }
        }
    };

    let mut parser = tabnas_jsonic::make();
    multisource(
        &mut parser,
        MultiSourceOptions::new(MapResolver::from([("d.json", r#"{"d":3}"#)]))
            .with_processor("json", json),
    )?;
    assert_eq!(parser.parse("x:@d.json")?.to_string(), r#"{"x":{"d":3}}"#);
    Ok(())
}
```

Register a processor for a kind of your own the same way, or point one
kind at another kind's processor:

```rust
use tabnas::Value;
use tabnas_multisource::{
    multisource, MapResolver, MultiSourceOptions, ProcessorInput, Resolution,
};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let lines = |resolution: &mut Resolution, _input: &ProcessorInput<'_>| {
        let src = resolution.src.clone().unwrap_or_default();
        resolution.val = Value::array(
            src.lines().map(|line| Value::String(line.to_string())).collect(),
        );
    };

    let mut parser = tabnas_jsonic::make();
    multisource(
        &mut parser,
        MultiSourceOptions::new(MapResolver::from([
            ("a.list", "one\ntwo"),
            ("b.conf", "b:1"),
        ]))
        .with_processor("list", lines)
        .with_processor_alias("conf", "jsonic"),
    )?;

    assert_eq!(parser.parse("x:@a.list")?.to_string(), r#"{"x":["one","two"]}"#);
    assert_eq!(parser.parse("y:@b.conf")?.to_string(), r#"{"y":{"b":1}}"#);
    Ok(())
}
```

## Errors

| Code | Raised when |
| --- | --- |
| `multisource_not_found` | a reference resolves to no source; the hint lists every path tried |
| `multisource_cycle` | a reference resolves to one of its own ancestors; the hint prints the loop |
| `multisource_depth` | a chain of sources nests deeper than `max_depth` |

A source included from two branches is reuse, not a cycle: the check is
against the ancestor chain, not against everything already visited. A
failure inside a loaded source, such as a nested reference that resolves
to nothing, fails the whole parse under the nested code rather than
substituting the raw source text.

`multisource_depth` has no counterpart in TypeScript or Go. It is
recorded in [`../DIVERGENCE.md`](../DIVERGENCE.md).

## Untrusted input

Following `@` references is this plugin's whole job, which is why the
boundary matters. Parsed content is data, never instructions, and a
loaded value is still hostile text.

- **A reference reaches only what the resolver can see.** Give the
  resolver a map, or a `MapFs`, and a document cannot read the disk
  whatever it asks for.
- **`FileResolver::with_root` and `PkgResolver::with_root` confine every
  candidate path to one directory.** The root and every candidate are
  resolved through the filesystem first, links and all, then compared by
  whole segment, so neither `../../etc/passwd` nor a symbolic link
  inside the root reaches outside it, and a link that resolves to
  nothing at all is refused rather than guessed at. One thing the check
  cannot cover is a link created in the instant between the check and
  the read: a root whose contents another process can write needs a
  filesystem that enforces the boundary. The confinement is additive:
  without a root the resolvers behave exactly as TypeScript and Go do.
- **A cycle ends in an error**, and a chain longer than `max_depth` ends
  in one too, so neither hangs nor exhausts the stack.
- **Provenance is available.** Pass `MultiSourceOptions::with_deps` a
  sink and the plugin records the target-to-source map of every source
  the parse touched, so a spliced value can be traced back.
- **Parsing is not sanitising.** Loaded values are spliced verbatim;
  escaping for SQL, HTML or a shell remains the caller's job.

There is no `js` kind. The TypeScript one calls `require`, which is to
say it executes the module, and Rust has no JavaScript runtime; a `.js`
reference falls through to the raw-text processor, as it does in Go.

## Dependencies

```rust
use std::sync::{Arc, Mutex};

use tabnas_multisource::{multisource, DependencyMap, MapResolver, MultiSourceOptions, TOP};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let deps: Arc<Mutex<DependencyMap>> = Arc::new(Mutex::new(DependencyMap::new()));
    let mut parser = tabnas_jsonic::make();
    multisource(
        &mut parser,
        MultiSourceOptions::new(MapResolver::from([
            ("a.jsonic", "a:1, b:@b.jsonic"),
            ("b.jsonic", "b:2"),
        ]))
        .with_deps(Arc::clone(&deps)),
    )?;

    parser.parse("@a.jsonic")?;

    let deps = deps.lock().expect("the sink is not poisoned");
    assert_eq!(deps[TOP]["a.jsonic"].src, "a.jsonic");
    assert_eq!(deps["a.jsonic"]["b.jsonic"].tar, "a.jsonic");
    Ok(())
}
```

## Install

Neither the engine nor the grammar plugins are published to a registry,
so all of them are consumed as **sibling checkouts**, the standard
tabnas development model. The plugin itself needs
`https://github.com/tabnas/parser` and
`https://github.com/tabnas/directive` next to this repository. It brings
no host grammar and no JSON reader, so add the grammar you install it
on as well; the examples here use `https://github.com/tabnas/jsonic`,
which in turn needs `https://github.com/tabnas/json` beside it:

```toml
[dependencies]
tabnas-multisource = { path = "../multisource/rs" }
tabnas = { package = "tabnas-parser", path = "../parser/rs" }
tabnas-jsonic = { path = "../jsonic/rs" }
```

A crate's dependencies are not passed on to its dependents, so
`tabnas-multisource` alone does not put `tabnas` or a host grammar in
the extern prelude, and the preceding examples that name them would not
resolve without these entries. The test suite takes jsonic as a
dev-dependency, and additionally needs
`https://github.com/tabnas/support`, `https://github.com/tabnas/path`
and `https://github.com/tabnas/debug` beside the repository.

## Differences from the canonical TypeScript

Every parse result is the TypeScript one, and the shared fixtures in
[`../test/spec`](../test/spec) hold all three runtimes to it. What
differs is the shape of the API and the points where the host language
has no way to say what JavaScript says. The measured list is in
[`../DIVERGENCE.md`](../DIVERGENCE.md); in outline:

- **Configuration is typed.** `MultiSourceOptions` is a struct with
  builder methods rather than an open object, and a resolver or a
  processor is a trait object rather than a function on a bag.
- **The filesystem is an option, not parse metadata.** A `Value` cannot
  carry a `node:fs` module, so `SourceFs` lives on the options or on a
  resolver. Go's `MultiSourceOptions.FS` makes the same move.
- **The dependency tree is collected into a sink** the caller supplies,
  because a Rust parse metadata value cannot be written back to. `TOP`
  is a sentinel string rather than a `Symbol`, as in Go.
- **There is no `js` kind**, for the reason given earlier.
- **A chain of sources is capped** at `max_depth`, and every second
  level of nesting moves to a fresh thread with a large stack, so a long
  chain cannot exhaust the caller's stack. A level that cannot be given
  a thread fails the parse, rather than resuming the chain on a stack
  too small to hold it.

## Build and test

The engine and the directive plugin, and for the tests the jsonic
grammar and the fixture runner, are path dependencies on sibling
checkouts, so there is nothing to fetch:

```bash
cargo test --all-targets && cargo test --doc
cargo clippy --all-targets --all-features -- -D warnings
```

Or, from the repository root, `make test-rs`. For what CI would say,
including formatting and the lockfile check, run `ci/rust/run.sh`.

The suite runs every shared `../test/spec/*.tsv` fixture, the same files
the TypeScript and Go suites run. Beside them are the in-language tests
for what a fixture cannot express: the file and package resolvers, the
preload scan, cycles and depth, the dependency tree, composition with
the path-diving and debug plugins, the sandbox root, and a hostile-input
suite.

## License

MIT.
