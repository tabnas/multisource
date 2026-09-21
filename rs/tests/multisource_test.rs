// The in-language suite: the port of `go/multisource_test.go`,
// `go/{cycle,deps,nested,colon_chain}_test.go` and the parts of
// `ts/test/multisource.test.ts` a shared fixture cannot express, which
// is everything needing a filesystem or a second plugin.

mod common;

use std::collections::BTreeMap;
use std::sync::{Arc, Mutex};

use tabnas::Value;
use tabnas_multisource::{
    build_potentials, default_processor, ext_kind, make_with, parse, plugin_with,
    preload_files_with, resolve_path_spec, source_dir, DependencyMap, FileResolver, MapFs,
    MapResolver, MultiSourceOptions, PkgResolver, PreloadOptions, ProcessorInput, Resolution,
    ResolverInput, TOP,
};

use common::to_json;

/// A JSON literal read the way a parse result is: see `common::canon`.
macro_rules! j {
    ($($tokens:tt)*) => {
        common::canon(serde_json::json!($($tokens)*))
    };
}

fn sources<const N: usize>(entries: [(&str, &str); N]) -> MapResolver {
    MapResolver::from(entries)
}

fn parse_json(parser: &tabnas::Tabnas, src: &str) -> serde_json::Value {
    to_json(
        &parser
            .parse(src)
            .unwrap_or_else(|error| panic!("{src}: {error}")),
    )
}

fn code_of(parser: &tabnas::Tabnas, src: &str) -> String {
    match parser.parse(src) {
        Ok(value) => panic!("{src}: expected a failure, got {value}"),
        Err(error) => error.code,
    }
}

// ---------------------------------------------------------------------
// The happy paths: go/multisource_test.go
// ---------------------------------------------------------------------

#[test]
fn happy() {
    let parser = make_with(MultiSourceOptions::new(sources([
        ("a.jsonic", "a:1"),
        ("b.jsc", "b:2"),
        ("c.txt", "CCC"),
        ("d.json", r#"{"d":3}"#),
        ("f.jsc", "f:5"),
        ("g/index.jsc", "g:6"),
        ("h/index.h.jsc", "h:7"),
    ])));

    assert_eq!(
        parse_json(&parser, "a:@a.jsonic,x:1"),
        j!({"a":{"a":1},"x":1})
    );
    assert_eq!(parse_json(&parser, "b:@b.jsc,x:1"), j!({"b":{"b":2},"x":1}));
    assert_eq!(parse_json(&parser, "c:@c.txt,x:1"), j!({"c":"CCC","x":1}));
    assert_eq!(
        parse_json(&parser, "d:@d.json,x:1"),
        j!({"d":{"d":3},"x":1})
    );
    assert_eq!(parse_json(&parser, "f:@f,x:1"), j!({"f":{"f":5},"x":1}));
    assert_eq!(parse_json(&parser, "g:@g,x:1"), j!({"g":{"g":6},"x":1}));
    assert_eq!(parse_json(&parser, "h:@h,x:1"), j!({"h":{"h":7},"x":1}));

    assert_eq!(
        parse_json(
            &parser,
            "\n  x:a:@a.jsonic \n  x:b:@b.jsc \n  x:c:@c.txt \n  x:d:@d.json \n  y:1\n  "
        ),
        j!({"x":{"a":{"a":1},"b":{"b":2},"c":"CCC","d":{"d":3}},"y":1})
    );
}

#[test]
fn multiple_sources() {
    let parser = make_with(MultiSourceOptions::new(sources([
        ("a.jsonic", "{a:1}"),
        ("b.jsonic", "{b:2}"),
    ])));
    assert_eq!(
        parse_json(&parser, "{x: @a.jsonic, y: @b.jsonic}"),
        j!({"x":{"a":1},"y":{"b":2}})
    );
}

#[test]
fn base_path() {
    let parser =
        make_with(MultiSourceOptions::new(sources([("data/a.jsonic", "{a:1}")])).with_path("data"));
    assert_eq!(parse_json(&parser, "{x: @a.jsonic}"), j!({"x":{"a":1}}));
}

#[test]
fn absolute_path_ignores_the_base() {
    let parser = make_with(
        MultiSourceOptions::new(sources([("/etc/config.jsonic", r#"{env:"prod"}"#)]))
            .with_path("ignored"),
    );
    assert_eq!(
        parse_json(&parser, "{cfg: @/etc/config.jsonic}"),
        j!({"cfg":{"env":"prod"}})
    );
}

#[test]
fn index_file() {
    let parser = make_with(MultiSourceOptions::new(sources([(
        "mymod/index.jsonic",
        "{x:1}",
    )])));
    assert_eq!(parse_json(&parser, "{mod: @mymod}"), j!({"mod":{"x":1}}));
}

#[test]
fn empty_input() {
    let parser = make_with(MultiSourceOptions::default());
    assert_eq!(parse_json(&parser, "{}"), j!({}));
}

#[test]
fn a_missing_source_is_an_error_not_a_null() {
    let parser = make_with(MultiSourceOptions::default());
    assert_eq!(code_of(&parser, "{x: @missing}"), "multisource_not_found");
}

#[test]
fn the_not_found_report_names_every_path_tried() {
    let parser = make_with(MultiSourceOptions::default());
    let error = parser.parse("x:@a").expect_err("a missing source fails");
    assert_eq!(error.code, "multisource_not_found");
    assert_eq!((error.row, error.col), (1, 3));
    assert!(
        error.hint.contains("a.jsonic") && error.hint.contains("a/index.jsonic"),
        "the hint lists the search paths: {}",
        error.hint
    );
}

#[test]
fn spec_object() {
    let parser = make_with(MultiSourceOptions::new(sources([("a.jsonic", "a:1")])));
    assert_eq!(
        parse_json(&parser, r#"x:@{path:"a.jsonic"}"#),
        j!({"x":{"a":1}})
    );
}

#[test]
fn merge_into_the_enclosing_map() {
    let parser = make_with(MultiSourceOptions::new(sources([("a.jsonic", "{a:1}")])));
    assert_eq!(parse_json(&parser, "{x:2, @a.jsonic}"), j!({"x":2,"a":1}));
}

#[test]
fn top_level_reference() {
    let parser = make_with(MultiSourceOptions::new(sources([("a.jsonic", "{a:1}")])));
    assert_eq!(parse_json(&parser, "@a.jsonic"), j!({"a":1}));
}

/// The README headline form and the implicit-container cases around it:
/// a top-level directive followed by, preceded by, or interleaved with
/// bare pairs. Ports `TestDirectiveThenPair`.
#[test]
fn directive_then_pair() {
    let parser = make_with(MultiSourceOptions::new(sources([
        ("a.jsonic", "a:1"),
        ("b.jsonic", "a:{b:1,c:2}"),
        ("d.jsonic", "d:3"),
    ])));

    let cases: [(&str, serde_json::Value); 10] = [
        ("@a.jsonic b:2", j!({"a":1,"b":2})),
        ("b:2 @a.jsonic", j!({"b":2,"a":1})),
        ("b:2 @a.jsonic c:3", j!({"b":2,"a":1,"c":3})),
        ("@a.jsonic", j!({"a":1})),
        ("@a.jsonic @d.jsonic", j!({"a":1,"d":3})),
        ("@a.jsonic x:11 @d.jsonic", j!({"a":1,"x":11,"d":3})),
        ("a:{d:3} @b.jsonic", j!({"a":{"d":3,"b":1,"c":2}})),
        (
            "a:{d:3} @b.jsonic a:{d:4,f:5}",
            j!({"a":{"d":4,"b":1,"c":2,"f":5}}),
        ),
        ("@b.jsonic a:{d:4,f:5}", j!({"a":{"b":1,"c":2,"d":4,"f":5}})),
        ("@b.jsonic y:2", j!({"a":{"b":1,"c":2},"y":2})),
    ];

    for (src, want) in cases {
        assert_eq!(parse_json(&parser, src), want, "{src}");
    }
}

// ---------------------------------------------------------------------
// Kinds, processors and options
// ---------------------------------------------------------------------

#[test]
fn a_custom_processor_handles_its_own_kind() {
    let csv = |resolution: &mut Resolution, _input: &ProcessorInput<'_>| {
        let src = resolution.src.clone().unwrap_or_default();
        resolution.val = Value::array(
            src.split(',')
                .map(|field| Value::String(field.trim().to_string()))
                .collect(),
        );
    };

    let parser = make_with(
        MultiSourceOptions::new(sources([("data.csv", "a,b,c")])).with_processor("csv", csv),
    );

    assert_eq!(
        parse_json(&parser, "{data: @data.csv}"),
        j!({"data":["a","b","c"]})
    );
}

/// A processor entry may name another kind, one level deep. The Go port
/// cannot express this (its processor map holds functions only); Rust
/// can, so it matches the canonical TypeScript. Ports the TS
/// `custom-ext` test.
#[test]
fn a_processor_entry_may_alias_another_kind() {
    let parser = make_with(
        MultiSourceOptions::new(sources([("t04.foo", "a:1")]))
            .with_processor_alias("foo", "jsonic"),
    );
    assert_eq!(parse_json(&parser, r#"@"t04.foo""#), j!({"a":1}));
}

/// One level only, exactly as in TypeScript, where the second lookup
/// yields a string and nothing callable comes back: the fallback is the
/// raw-text processor.
#[test]
fn an_alias_to_an_alias_falls_back_to_raw_text() {
    let parser = make_with(
        MultiSourceOptions::new(sources([("t.one", "a:1")]))
            .with_processor_alias("one", "two")
            .with_processor_alias("two", "jsonic"),
    );
    assert_eq!(parse_json(&parser, r#"x:@"t.one""#), j!({"x":"a:1"}));
}

#[test]
fn an_unknown_extension_is_raw_text() {
    let parser = make_with(MultiSourceOptions::new(sources([("c.txt", "CCC")])));
    assert_eq!(parse_json(&parser, "c:@c.txt"), j!({"c":"CCC"}));
}

#[test]
fn malformed_json_fails_the_parse() {
    let parser = make_with(MultiSourceOptions::new(sources([("a.json", "not json")])));
    assert_eq!(code_of(&parser, r#"k:@"a.json""#), "unexpected");
}

/// A nested reference that cannot be resolved fails the whole parse,
/// with the nested code, rather than substituting the raw source text.
#[test]
fn a_nested_failure_propagates_with_its_own_code() {
    let parser = make_with(MultiSourceOptions::new(sources([(
        "a.jsonic",
        r#"x:@"gone.jsonic""#,
    )])));
    assert_eq!(code_of(&parser, r#"@"a.jsonic""#), "multisource_not_found");
}

/// `map.merge` wins over `map.extend`, and is handed the whole node, as
/// in TypeScript. Ports the TS `merge` test.
#[test]
fn map_merge_is_used_for_the_splice() {
    let mut parser = make_with(MultiSourceOptions::new(sources([("a.jsonic", "a:1")])));
    parser
        .set_options(|options| {
            options.map.merge = Some(Arc::new(|previous, current, _rule, _context| {
                tabnas_jsonic::deep_merge(previous, current)
            }));
        })
        .expect("the merge option applies");
    assert_eq!(parse_json(&parser, "x:2 @a.jsonic"), j!({"x":2,"a":1}));
}

/// With `map.extend` off and no merge callback the splice is a shallow
/// assignment. Ports the TS `assign` test and `test/spec/options.tsv`.
#[test]
fn map_extend_off_assigns() {
    let mut parser = make_with(MultiSourceOptions::new(sources([("a.jsonic", "a:1")])));
    parser
        .set_options(|options| options.map.extend = false)
        .expect("the extend option applies");
    assert_eq!(parse_json(&parser, "x:2 @a.jsonic"), j!({"x":2,"a":1}));
}

#[test]
fn a_custom_markchar_replaces_the_default() {
    let parser =
        make_with(MultiSourceOptions::new(sources([("a.jsonic", "a:1")])).with_markchar("%"));
    assert_eq!(parse_json(&parser, r#"x:%"a.jsonic""#), j!({"x":{"a":1}}));
}

// ---------------------------------------------------------------------
// Cycles and depth
// ---------------------------------------------------------------------

#[test]
fn a_cycle_is_an_error_not_a_stack_overflow() {
    let cases: [(&str, Vec<(&str, &str)>); 3] = [
        ("direct", vec![("a.jsonic", r#"@"a.jsonic""#)]),
        (
            "two-step",
            vec![
                ("a.jsonic", r#"@"b.jsonic""#),
                ("b.jsonic", r#"@"a.jsonic""#),
            ],
        ),
        (
            "three-step",
            vec![
                ("a.jsonic", r#"@"b.jsonic""#),
                ("b.jsonic", r#"@"c.jsonic""#),
                ("c.jsonic", r#"@"a.jsonic""#),
            ],
        ),
    ];
    for (name, files) in cases {
        let parser = make_with(MultiSourceOptions::new(MapResolver::from_iter(files)));
        assert_eq!(
            code_of(&parser, r#"@"a.jsonic""#),
            "multisource_cycle",
            "{name}"
        );
    }
}

#[test]
fn the_cycle_report_names_the_loop() {
    let parser = make_with(MultiSourceOptions::new(sources([
        ("a.jsonic", r#"@"b.jsonic""#),
        ("b.jsonic", r#"@"a.jsonic""#),
    ])));
    let error = parser
        .parse(r#"@"a.jsonic""#)
        .expect_err("a loop fails the parse");
    assert!(
        error.hint.contains("a.jsonic -> b.jsonic -> a.jsonic"),
        "the hint prints the loop: {}",
        error.hint
    );
}

/// Reuse is not a cycle: a source included from two branches, or twice
/// from one, is fine. The check is against the ancestor chain, not
/// against everything already visited.
#[test]
fn reuse_is_not_a_cycle() {
    let diamond = make_with(MultiSourceOptions::new(sources([
        ("base.jsonic", "x:1"),
        ("l.jsonic", r#"@"base.jsonic""#),
        ("r.jsonic", r#"@"base.jsonic""#),
    ])));
    assert_eq!(
        parse_json(&diamond, r#"l: @"l.jsonic", r: @"r.jsonic""#),
        j!({"l":{"x":1},"r":{"x":1}})
    );

    let twice = make_with(MultiSourceOptions::new(sources([("base.jsonic", "x:1")])));
    assert_eq!(
        parse_json(&twice, r#"a: @"base.jsonic", b: @"base.jsonic""#),
        j!({"a":{"x":1},"b":{"x":1}})
    );

    let chain = make_with(MultiSourceOptions::new(sources([
        ("a.jsonic", r#"@"b.jsonic""#),
        ("b.jsonic", r#"@"c.jsonic""#),
        ("c.jsonic", "z:1"),
    ])));
    assert_eq!(parse_json(&chain, r#"@"a.jsonic""#), j!({"z":1}));
}

/// An acyclic chain is still bounded: every link is parsed inside the
/// parse that referenced it, so an unbounded one would run out of
/// stack. Rust caps it where TypeScript and Go do not; see
/// `DIVERGENCE.md`.
#[test]
fn a_chain_deeper_than_the_cap_is_an_error_not_a_stack_overflow() {
    let depth = 4000;
    let mut files: BTreeMap<String, String> = BTreeMap::new();
    for level in 0..depth {
        files.insert(
            format!("s{level}.jsonic"),
            format!(r#"@"s{}.jsonic""#, level + 1),
        );
    }
    files.insert(format!("s{depth}.jsonic"), "end:1".to_string());

    let parser = make_with(MultiSourceOptions::new(MapResolver::from_map(files)));
    assert_eq!(code_of(&parser, r#"@"s0.jsonic""#), "multisource_depth");
}

#[test]
fn the_depth_cap_is_configurable() {
    let parser = make_with(
        MultiSourceOptions::new(sources([
            ("a.jsonic", r#"@"b.jsonic""#),
            ("b.jsonic", r#"@"c.jsonic""#),
            ("c.jsonic", "z:1"),
        ]))
        .with_max_depth(2),
    );
    assert_eq!(code_of(&parser, r#"@"a.jsonic""#), "multisource_depth");
}

// ---------------------------------------------------------------------
// Nested relative resolution
// ---------------------------------------------------------------------

/// A relative reference inside a loaded source resolves against that
/// source's own directory, across directories. Ports
/// `TestNestedRelativeLoad` and the TS `nested-relative-dirs`.
#[test]
fn a_relative_reference_resolves_against_its_own_source() {
    let disk = MapFs::from([
        ("main.jsonic", r#"{top:1, child:@"./sub/child.jsonic"}"#),
        ("sub/child.jsonic", r#"{mid:2, grand:@"./grand.jsonic"}"#),
        ("sub/grand.jsonic", "{v:99}"),
    ]);
    let parser = make_with(MultiSourceOptions::new(FileResolver::new()).with_fs(disk));
    assert_eq!(
        parse_json(&parser, r#"@"./main.jsonic""#),
        j!({"top":1,"child":{"mid":2,"grand":{"v":99}}})
    );
}

/// Two sources loaded from one parent each resolve their own relative
/// reference against their own directory: the base is tracked per
/// source, and resolving one does not leak into a sibling.
#[test]
fn sibling_sources_keep_their_own_base() {
    let disk = MapFs::from([
        ("main.jsonic", r#"{a:@"./aa/a.jsonic", b:@"./bb/b.jsonic"}"#),
        ("aa/a.jsonic", r#"{x:@"./inner.jsonic"}"#),
        ("aa/inner.jsonic", "{n:11}"),
        ("bb/b.jsonic", r#"{y:@"./inner.jsonic"}"#),
        ("bb/inner.jsonic", "{n:22}"),
    ]);
    let parser = make_with(MultiSourceOptions::new(FileResolver::new()).with_fs(disk));
    assert_eq!(
        parse_json(&parser, r#"@"./main.jsonic""#),
        j!({"a":{"x":{"n":11}},"b":{"y":{"n":22}}})
    );
}

/// Nested references through flat in-memory keys stay bare keys: a
/// parent key such as `a.jsc` must yield an EMPTY base, not `.`, or a
/// bare nested `@b.jsc` no longer matches.
#[test]
fn flat_in_memory_keys_nest() {
    let parser = make_with(MultiSourceOptions::new(sources([
        ("a.jsc", "a:1,b:@b.jsc,x:99"),
        ("b.jsc", "b:2,c:@c"),
        ("c/index.jsc", "c:3"),
    ])));
    assert_eq!(
        parse_json(&parser, "@a"),
        j!({"a":1,"b":{"b":2,"c":{"c":3}},"x":99})
    );
}

/// The full path of the source being processed, and the chain of
/// enclosing parents, are threaded through the parse metadata. A custom
/// processor reads what it is handed.
#[test]
fn the_source_path_and_parents_are_threaded_through_the_metadata() {
    type Sighting = (String, Vec<String>);
    let seen: Arc<Mutex<Vec<Sighting>>> = Arc::new(Mutex::new(Vec::new()));
    let recorder = Arc::clone(&seen);

    let probe = move |resolution: &mut Resolution, input: &ProcessorInput<'_>| {
        let meta = to_json(input.meta);
        let path = meta
            .pointer("/multisource/path")
            .and_then(serde_json::Value::as_str)
            .unwrap_or_default()
            .to_string();
        let parents = meta
            .pointer("/multisource/parents")
            .and_then(serde_json::Value::as_array)
            .map(|items| {
                items
                    .iter()
                    .filter_map(|item| item.as_str().map(str::to_string))
                    .collect()
            })
            .unwrap_or_default();
        recorder
            .lock()
            .expect("the recorder is not poisoned")
            .push((path, parents));
        default_processor(resolution, input);
    };

    let disk = MapFs::from([
        ("main.jsonic", r#"{child:@"./sub/c.probe"}"#),
        ("sub/c.probe", "probe-content"),
    ]);
    let parser = make_with(
        MultiSourceOptions::new(FileResolver::new())
            .with_fs(disk)
            .with_processor("probe", probe),
    );
    parser
        .parse(r#"@"./main.jsonic""#)
        .expect("the probe source loads");

    let seen = seen.lock().expect("the recorder is not poisoned");
    let (path, parents) = seen.last().expect("the probe ran");
    assert!(path.ends_with("sub/c.probe"), "threaded path: {path}");
    assert_eq!(parents.len(), 1, "threaded parents: {parents:?}");
    assert!(parents[0].ends_with("main.jsonic"), "{parents:?}");
}

// ---------------------------------------------------------------------
// The colon chain
// ---------------------------------------------------------------------

/// A bare `@"file"` reached through a colon chain (`a: b: @"f"`) must
/// resolve NESTED under the key. The back-track condition on `val` open
/// has to require that the rule's parent is not already the pair for
/// that key, or the mark unwinds to depth zero, the key finalises to
/// null, and the load is silently dropped.
#[test]
fn a_colon_chain_keeps_the_reference_under_its_key() {
    let parser = make_with(MultiSourceOptions::new(sources([("minor", "{x:1}")])));

    let cases: [(&str, serde_json::Value); 4] = [
        ("struct: @minor", j!({"struct":"{x:1}"})),
        ("struct: {minor: @minor}", j!({"struct":{"minor":"{x:1}"}})),
        ("struct: minor: @minor", j!({"struct":{"minor":"{x:1}"}})),
        ("a: b: c: @minor", j!({"a":{"b":{"c":"{x:1}"}}})),
    ];
    for (src, want) in cases {
        assert_eq!(parse_json(&parser, src), want, "{src}");
    }
}

#[test]
fn a_colon_chain_works_over_a_filesystem_too() {
    let disk = MapFs::from([("minor.aon", "{x:1}")]);
    let parser = make_with(MultiSourceOptions::new(FileResolver::new()).with_fs(disk));
    assert_eq!(
        parse_json(&parser, r#"struct: minor: @"minor.aon""#),
        j!({"struct":{"minor":"{x:1}"}})
    );
}

// ---------------------------------------------------------------------
// The file resolver
// ---------------------------------------------------------------------

#[test]
fn the_file_resolver_reads_an_injected_filesystem() {
    let disk = MapFs::from([
        ("a.jsonic", "{a:1}"),
        ("b.jsonic", "{b:2}"),
        ("mod/index.jsonic", "{m:3}"),
        ("h/index.h.jsonic", "{h:7}"),
        ("data/cfg.json", r#"{"k":4}"#),
    ]);
    let parser = make_with(MultiSourceOptions::new(FileResolver::new()).with_fs(disk));

    let cases: [(&str, serde_json::Value); 5] = [
        ("{x:@a.jsonic}", j!({"a":1})),
        ("{x:@b}", j!({"b":2})),
        ("{x:@mod}", j!({"m":3})),
        ("{x:@h}", j!({"h":7})),
        (r#"{x:@"data/cfg.json"}"#, j!({"k":4})),
    ];
    for (src, want) in cases {
        assert_eq!(parse_json(&parser, src)["x"], want, "{src}");
    }
}

/// A dot in a FOLDER name must not suppress the implicit-extension or
/// index search for an extensionless reference: `my.app/conf` has no
/// extension, the folder does.
#[test]
fn a_dotted_folder_does_not_invent_a_kind() {
    let disk = MapFs::from([
        ("my.app/conf.jsonic", "c:1"),
        ("my.app/mod/index.jsonic", "m:2"),
        ("my.app/plain.txt", "RAW"),
        ("my.app/nest.jsonic", r#"n:@"conf.jsonic""#),
    ]);
    let parser = make_with(MultiSourceOptions::new(FileResolver::new()).with_fs(disk));

    let cases: [(&str, serde_json::Value); 4] = [
        (r#"{x:@"my.app/conf"}"#, j!({"c":1})),
        (r#"{x:@"my.app/mod"}"#, j!({"m":2})),
        (r#"{x:@"my.app/plain.txt"}"#, j!("RAW")),
        (r#"{x:@"my.app/nest"}"#, j!({"n":{"c":1}})),
    ];
    for (src, want) in cases {
        assert_eq!(parse_json(&parser, src)["x"], want, "{src}");
    }
}

#[test]
fn the_file_resolver_takes_a_pathfinder() {
    let disk = MapFs::from([("sub/a.jsonic", "{a:1}")]);
    let parser = make_with(
        MultiSourceOptions::new(
            FileResolver::new().with_pathfinder(|reference| format!("sub/{reference}")),
        )
        .with_fs(disk),
    );
    assert_eq!(parse_json(&parser, "{x: @a.jsonic}")["x"], j!({"a":1}));
}

#[test]
fn the_file_resolver_serves_preloaded_content_first() {
    // Nothing on the filesystem at all: resolution must come from the
    // preloaded map.
    let preload = BTreeMap::from([("p.jsonic".to_string(), "{p:4}".to_string())]);
    let parser = make_with(MultiSourceOptions::new(
        FileResolver::new()
            .with_fs(MapFs::new())
            .with_preload(preload),
    ));
    assert_eq!(parse_json(&parser, "{x: @p.jsonic}")["x"], j!({"p":4}));
}

#[test]
fn the_file_resolver_reports_a_missing_source() {
    let parser = make_with(MultiSourceOptions::new(
        FileResolver::new().with_fs(MapFs::new()),
    ));
    assert_eq!(
        code_of(&parser, "{x: @missing.jsonic}"),
        "multisource_not_found"
    );
}

// ---------------------------------------------------------------------
// The root sandbox
// ---------------------------------------------------------------------

/// A reference must not escape the root a caller gives it, however many
/// `..` segments it carries, and whether it is relative or absolute.
#[test]
fn a_root_confines_every_reference() {
    let disk = MapFs::from([
        ("srv/app/main.jsonic", r#"{ok:1}"#),
        ("srv/secret.jsonic", "{leak:1}"),
        ("srv/appdata/other.jsonic", "{sibling:1}"),
    ]);
    let parser = make_with(
        MultiSourceOptions::new(FileResolver::new().with_fs(disk).with_root("srv/app"))
            .with_path("srv/app"),
    );

    assert_eq!(
        parse_json(&parser, r#"x:@"main.jsonic""#)["x"],
        j!({"ok":1})
    );
    for escape in [
        r#"x:@"../secret.jsonic""#,
        r#"x:@"../appdata/other.jsonic""#,
        r#"x:@"/srv/secret.jsonic""#,
        r#"x:@"./../secret.jsonic""#,
    ] {
        assert_eq!(
            code_of(&parser, escape),
            "multisource_not_found",
            "{escape} must not escape the root"
        );
    }
}

/// A reference inside a loaded source is confined too: the root is
/// checked on every candidate, not only on the first reference.
#[test]
fn a_root_confines_a_nested_reference() {
    let disk = MapFs::from([
        ("srv/app/main.jsonic", r#"{leak:@"../secret.jsonic"}"#),
        ("srv/secret.jsonic", "{leak:1}"),
    ]);
    let parser = make_with(
        MultiSourceOptions::new(FileResolver::new().with_fs(disk).with_root("srv/app"))
            .with_path("srv/app"),
    );
    assert_eq!(
        code_of(&parser, r#"@"main.jsonic""#),
        "multisource_not_found"
    );
}

/// Without a root the resolver behaves exactly as TypeScript and Go do:
/// it resolves wherever the reference points. The sandbox is opt-in, so
/// this is what makes it additive rather than a divergence.
#[test]
fn without_a_root_a_relative_reference_still_climbs() {
    let disk = MapFs::from([("srv/secret.jsonic", "{leak:1}")]);
    let parser =
        make_with(MultiSourceOptions::new(FileResolver::new().with_fs(disk)).with_path("srv/app"));
    assert_eq!(
        parse_json(&parser, r#"x:@"../secret.jsonic""#)["x"],
        j!({"leak":1})
    );
}

// ---------------------------------------------------------------------
// The package resolver
// ---------------------------------------------------------------------

#[test]
fn the_pkg_resolver_finds_a_subpath_an_index_and_a_main() {
    let disk = MapFs::from([
        ("node_modules/mypkg/zed.jsonic", "{zed:99}"),
        ("node_modules/idxpkg/index.jsonic", "{i:5}"),
        (
            "node_modules/mainpkg/package.json",
            r#"{"main":"main.jsonic"}"#,
        ),
        ("node_modules/mainpkg/main.jsonic", "{z:11}"),
    ]);
    let parser =
        make_with(MultiSourceOptions::new(PkgResolver::new().with_paths(["."])).with_fs(disk));

    let cases: [(&str, serde_json::Value); 3] = [
        (r#"{c:@"mypkg/zed.jsonic"}"#, j!({"zed":99})),
        (r#"{c:@"idxpkg"}"#, j!({"i":5})),
        (r#"{c:@"mainpkg"}"#, j!({"z":11})),
    ];
    for (src, want) in cases {
        assert_eq!(parse_json(&parser, src)["c"], want, "{src}");
    }
}

#[test]
fn the_pkg_resolver_walks_up_to_find_node_modules() {
    let disk = MapFs::from([
        ("node_modules/mypkg/zed.jsonic", "{zed:99}"),
        ("a/b/c/.keep", ""),
    ]);
    let parser =
        make_with(MultiSourceOptions::new(PkgResolver::new().with_paths(["a/b/c"])).with_fs(disk));
    assert_eq!(
        parse_json(&parser, r#"{c:@"mypkg/zed.jsonic"}"#)["c"],
        j!({"zed":99})
    );
}

/// A relative reference found inside a source loaded from a package is
/// not a package name: it resolves against that source's own directory.
#[test]
fn a_relative_reference_inside_a_package_is_not_a_package_name() {
    let disk = MapFs::from([
        (
            "node_modules/relpkg/index.jsonic",
            r#"{a:1, b:@"./child.jsonic", c:@"./leaf", d:@"./sub/deep.jsonic"}"#,
        ),
        ("node_modules/relpkg/child.jsonic", "{x:10}"),
        ("node_modules/relpkg/leaf.jsonic", "{y:20}"),
        ("node_modules/relpkg/sub/deep.jsonic", "{z:30}"),
    ]);
    let parser =
        make_with(MultiSourceOptions::new(PkgResolver::new().with_paths(["."])).with_fs(disk));
    assert_eq!(
        parse_json(&parser, r#"{r:@"relpkg"}"#)["r"],
        j!({"a":1,"b":{"x":10},"c":{"y":20},"d":{"z":30}})
    );
}

#[test]
fn the_pkg_resolver_reports_a_missing_package() {
    let parser = make_with(
        MultiSourceOptions::new(PkgResolver::new().with_paths(["."])).with_fs(MapFs::new()),
    );
    assert_eq!(
        code_of(&parser, r#"{x: @"nopkg/zed.jsonic"}"#),
        "multisource_not_found"
    );
}

#[test]
fn the_pkg_resolver_takes_a_root_too() {
    let disk = MapFs::from([
        ("srv/node_modules/mypkg/zed.jsonic", "{zed:99}"),
        ("node_modules/mypkg/zed.jsonic", "{zed:1}"),
    ]);
    let parser = make_with(
        MultiSourceOptions::new(PkgResolver::new().with_paths(["srv"]).with_root("srv"))
            .with_fs(disk),
    );
    // The walk up reaches the root's node_modules, but the root check
    // keeps the outer copy out of view.
    assert_eq!(
        parse_json(&parser, r#"{c:@"mypkg/zed.jsonic"}"#)["c"],
        j!({"zed":99})
    );
}

// ---------------------------------------------------------------------
// Dependencies
// ---------------------------------------------------------------------

#[test]
fn the_dependency_tree_records_every_edge() {
    let deps: Arc<Mutex<DependencyMap>> = Arc::new(Mutex::new(DependencyMap::new()));
    let parser = make_with(
        MultiSourceOptions::new(sources([
            ("a.jsc", "a:1,b:@b.jsc,x:99"),
            ("b.jsc", "b:2,c:@c"),
            ("c/index.jsc", "c:3"),
        ]))
        .with_deps(Arc::clone(&deps)),
    );

    assert_eq!(
        parse_json(&parser, "@a"),
        j!({"a":1,"b":{"b":2,"c":{"c":3}},"x":99})
    );

    let deps = deps.lock().expect("the dependency map is not poisoned");
    assert_eq!(deps.len(), 3, "{deps:?}");
    for (target, source) in [(TOP, "a.jsc"), ("a.jsc", "b.jsc"), ("b.jsc", "c/index.jsc")] {
        let record = deps
            .get(target)
            .and_then(|bucket| bucket.get(source))
            .unwrap_or_else(|| panic!("missing {target} -> {source} in {deps:?}"));
        assert_eq!(record.tar, target);
        assert_eq!(record.src, source);
        assert!(record.wen > 0, "{record:?}");
    }
}

#[test]
fn one_source_referenced_from_two_targets_is_recorded_under_both() {
    let deps: Arc<Mutex<DependencyMap>> = Arc::new(Mutex::new(DependencyMap::new()));
    let parser = make_with(
        MultiSourceOptions::new(sources([
            ("main.jsonic", "{p:@one.jsonic, q:@two.jsonic}"),
            ("one.jsonic", "{o:@shared.jsonic}"),
            ("two.jsonic", "{t:@shared.jsonic}"),
            ("shared.jsonic", "{s:1}"),
        ]))
        .with_deps(Arc::clone(&deps)),
    );
    parser
        .parse("@main.jsonic")
        .expect("the tree of sources loads");

    let deps = deps.lock().expect("the dependency map is not poisoned");
    assert_eq!(deps[TOP]["main.jsonic"].src, "main.jsonic");
    assert_eq!(deps["main.jsonic"]["one.jsonic"].tar, "main.jsonic");
    assert_eq!(deps["main.jsonic"]["two.jsonic"].tar, "main.jsonic");
    assert_eq!(deps["one.jsonic"]["shared.jsonic"].src, "shared.jsonic");
    assert_eq!(deps["two.jsonic"]["shared.jsonic"].src, "shared.jsonic");
}

#[test]
fn a_parse_without_a_sink_records_nothing_and_still_loads() {
    let parser = make_with(MultiSourceOptions::new(sources([
        ("a.jsc", "a:1,b:@b.jsc,x:99"),
        ("b.jsc", "b:2,c:@c"),
        ("c/index.jsc", "c:3"),
    ])));
    assert_eq!(
        parse_json(&parser, "@a"),
        j!({"a":1,"b":{"b":2,"c":{"c":3}},"x":99})
    );
}

// ---------------------------------------------------------------------
// Preload
// ---------------------------------------------------------------------

#[test]
fn preload_defaults_to_jsonic_and_json() {
    let disk = MapFs::from([
        ("t/t01.jsonic", "{c:2}"),
        ("t/k03.json", r#"{"g":4}"#),
        ("t/k02.js", "module.exports={e:3}"),
        ("t/f01/f01t01.jsonic", "{f:1}"),
    ]);

    let loaded = preload_files_with(&PreloadOptions::new(["t"]), &disk);
    assert_eq!(
        loaded.keys().cloned().collect::<Vec<_>>(),
        vec!["t/k03.json".to_string(), "t/t01.jsonic".to_string()]
    );
}

#[test]
fn preload_extensions_are_normalised_and_replace_the_default() {
    let disk = MapFs::from([("t/a.jsonic", "{a:1}"), ("t/b.js", "module.exports={}")]);

    let dotted = preload_files_with(&PreloadOptions::new(["t"]).with_ext([".js"]), &disk);
    let bare = preload_files_with(&PreloadOptions::new(["t"]).with_ext(["js"]), &disk);
    assert_eq!(dotted, bare);
    assert_eq!(dotted.keys().cloned().collect::<Vec<_>>(), vec!["t/b.js"]);
}

#[test]
fn preload_descends_only_when_asked() {
    let disk = MapFs::from([("t/a.jsonic", "{a:1}"), ("t/f01/b.jsonic", "{b:2}")]);

    let flat = preload_files_with(&PreloadOptions::new(["t"]).with_ext([".jsonic"]), &disk);
    assert!(!flat.keys().any(|key| key.contains("f01")), "{flat:?}");
    assert!(!flat.is_empty());

    let deep = preload_files_with(
        &PreloadOptions::new(["t"])
            .with_ext([".jsonic"])
            .with_recursive(true),
        &disk,
    );
    assert!(deep.keys().any(|key| key.contains("f01")), "{deep:?}");
}

#[test]
fn preload_combines_several_folders() {
    let disk = MapFs::from([("t/a.jsonic", "{a:1}"), ("t/f01/b.jsonic", "{b:2}")]);
    let combined = preload_files_with(
        &PreloadOptions::new(["t", "t/f01"]).with_ext([".jsonic"]),
        &disk,
    );
    assert_eq!(
        combined.keys().cloned().collect::<Vec<_>>(),
        vec!["t/a.jsonic".to_string(), "t/f01/b.jsonic".to_string()]
    );
}

#[test]
fn preload_skips_a_missing_folder() {
    let disk = MapFs::new();
    let loaded = preload_files_with(&PreloadOptions::new(["/nonexistent/folder/path"]), &disk);
    assert!(loaded.is_empty(), "{loaded:?}");
}

/// The preloaded map feeds the file resolver, so a parse reads nothing
/// from the filesystem. The filesystem is emptied after the scan to
/// prove it.
#[test]
fn preloaded_content_resolves_with_no_filesystem_left() {
    let disk = MapFs::from([("t/t01.jsonic", "{c:2}"), ("t/f01/x.jsonic", "{f:1}")]);
    let filemap = preload_files_with(
        &PreloadOptions::new(["t"])
            .with_ext([".jsonic"])
            .with_recursive(true),
        &disk,
    );

    let options = PreloadOptions::new(["t"])
        .with_ext([".jsonic"])
        .with_recursive(true);
    let parser = make_with(
        MultiSourceOptions::new(
            FileResolver::new()
                .with_fs(MapFs::new())
                .with_preload(filemap),
        )
        .with_path("t")
        .with_preload(options.clone()),
    );

    assert_eq!(parse_json(&parser, r#"@"t01.jsonic""#), j!({"c":2}));
}

// ---------------------------------------------------------------------
// Composition
// ---------------------------------------------------------------------

/// The path-diving plugin and this one compose: a reference that lands
/// at a dived key keeps its place, and the dive travels into the nested
/// parse through `meta.path.base`.
#[test]
fn it_composes_with_the_path_plugin() {
    let mut parser = tabnas_jsonic::make();
    tabnas_multisource::multisource(
        &mut parser,
        MultiSourceOptions::new(sources([("x.jsonic", "x:y:1")])),
    )
    .expect("the MultiSource plugin installs");
    tabnas_path::path(&mut parser).expect("the Path plugin installs");
    parser.define_rule("val".to_string(), |spec| {
        spec.add_ac_ref("@stamp-path");
    });
    parser.state_action_ref("@stamp-path", |rule, _context| {
        let stamp = match rule.k.get(tabnas_path::PATH) {
            Some(Value::Array(segments)) => segments
                .iter()
                .map(|segment| match segment {
                    Value::String(text) => text.clone(),
                    other => other.to_string(),
                })
                .collect::<Vec<_>>()
                .join(","),
            _ => String::new(),
        };
        let mut node = rule.node.borrow_mut();
        if let Value::Object(map) = &mut *node {
            std::sync::Arc::make_mut(map).insert("$".to_string(), Value::String(stamp));
        } else if let Value::MapRef(map) = &mut *node {
            std::sync::Arc::make_mut(map)
                .value
                .insert("$".to_string(), Value::String(stamp));
        }
        Ok(())
    });

    assert_eq!(
        parse_json(&parser, r#"a:b:@"x.jsonic""#),
        j!({
            "$": "",
            "a": { "$": "a", "b": { "$": "a,b", "x": { "$": "a,b,x", "y": 1 } } },
        })
    );
}

/// A plugin installed AFTER this one is still used for a nested parse,
/// which is what the canonical TypeScript gets from closing over a live
/// instance.
#[test]
fn a_plugin_installed_afterwards_reaches_the_nested_parse() {
    let mut parser = tabnas_jsonic::make();
    tabnas_multisource::multisource(
        &mut parser,
        MultiSourceOptions::new(sources([("a.jsonic", "@b.jsonic"), ("b.jsonic", "b:1")])),
    )
    .expect("the MultiSource plugin installs");
    tabnas_path::path(&mut parser).expect("the Path plugin installs");
    assert_eq!(parse_json(&parser, "@a.jsonic"), j!({"b":1}));
}

// ---------------------------------------------------------------------
// The API surface
// ---------------------------------------------------------------------

#[test]
fn ext_kind_reads_the_last_segment_only() {
    assert_eq!(ext_kind(Some("a.jsonic")), "jsonic");
    assert_eq!(ext_kind(Some("a.d/foo")), "");
    assert_eq!(ext_kind(Some("a.d/foo.txt")), "txt");
    assert_eq!(ext_kind(Some("noext")), "");
    assert_eq!(ext_kind(None), "");
}

#[test]
fn resolve_path_spec_normalises_a_reference() {
    let spec = resolve_path_spec(Some("a.jsonic"), "base");
    assert_eq!(spec.full.as_deref(), Some("base/a.jsonic"));
    assert_eq!(spec.kind, "jsonic");
    assert!(!spec.abs);

    let spec = resolve_path_spec(Some("/abs/a.json"), "base");
    assert_eq!(spec.full.as_deref(), Some("/abs/a.json"));
    assert_eq!(spec.kind, "json");
    assert!(spec.abs);

    let spec = resolve_path_spec(Some("noext"), "");
    assert_eq!(spec.full.as_deref(), Some("noext"));
    assert_eq!(spec.kind, "");

    let spec = resolve_path_spec(None, "base");
    assert_eq!(spec.full, None);
}

#[test]
fn build_potentials_covers_extensions_and_index_files() {
    let exts: Vec<String> = [".jsonic", ".jsc", ".json"]
        .into_iter()
        .map(str::to_string)
        .collect();

    let potentials = build_potentials("foo", &exts);
    assert_eq!(potentials[0], "foo");
    assert_eq!(potentials[1], "foo.jsonic");
    assert_eq!(potentials[2], "foo.jsc");
    assert_eq!(potentials[3], "foo.json");
    assert_eq!(potentials[4], "foo/index.jsonic");
    assert!(potentials.contains(&"foo/index.foo.jsonic".to_string()));

    assert_eq!(build_potentials("bar.json", &exts), vec!["bar.json"]);
    assert!(build_potentials("", &exts).is_empty());
}

#[test]
fn source_dir_keeps_a_bare_key_bare() {
    assert_eq!(source_dir("a.jsonic"), "");
    assert_eq!(source_dir("b/a.jsonic"), "b");
    assert_eq!(source_dir("/a.jsonic"), "/");
    assert_eq!(source_dir("/b/a.jsonic"), "/b");
}

#[test]
fn parse_builds_an_instance_for_one_document() {
    let value = parse(
        "{x: @a.jsonic}",
        MultiSourceOptions::new(sources([("a.jsonic", "{a:1}")])),
    )
    .expect("the document parses");
    assert_eq!(to_json(&value), j!({"x":{"a":1}}));
}

/// A closure is a resolver too, so a caller needs no type of their own
/// for a one-off source set.
#[test]
fn a_closure_is_a_resolver() {
    let resolver = |reference: Option<&str>, _input: &ResolverInput<'_>| match reference {
        Some("live") => {
            Resolution::found(resolve_path_spec(Some("live"), ""), "live.jsonic", "now:1")
        }
        _ => Resolution::not_found(resolve_path_spec(reference, "")),
    };
    let parser = make_with(MultiSourceOptions::new(resolver));
    assert_eq!(parse_json(&parser, "x:@live"), j!({"x":{"now":1}}));
    assert_eq!(code_of(&parser, "x:@other"), "multisource_not_found");
}

/// The plugin refuses a bare engine rather than installing a directive
/// that could never match.
#[test]
fn it_refuses_an_engine_with_no_host_grammar() {
    let mut parser = tabnas::Tabnas::new();
    let error = match parser.use_plugin(plugin_with(MultiSourceOptions::default()), None) {
        Ok(_) => panic!("a bare engine has no val rule"),
        Err(error) => error,
    };
    assert!(error.0.contains("host grammar"), "{}", error.0);
}

/// Every instance holds its own resolver, so two parsers configured
/// differently never see each other's sources: nothing lives in global
/// state.
#[test]
fn two_instances_do_not_share_sources() {
    let left = make_with(MultiSourceOptions::new(sources([(
        "s.jsonic",
        "side:\"left\"",
    )])));
    let right = make_with(MultiSourceOptions::new(sources([(
        "s.jsonic",
        "side:\"right\"",
    )])));
    assert_eq!(parse_json(&left, "@s.jsonic"), j!({"side":"left"}));
    assert_eq!(parse_json(&right, "@s.jsonic"), j!({"side":"right"}));
}

/// The instance is `Send + Sync` and parses through `&self`, so one
/// parser serves many threads. Per-parse state lives in the context,
/// never on the instance.
#[test]
fn one_instance_parses_from_many_threads() {
    fn assert_send_sync<T: Send + Sync>() {}
    assert_send_sync::<tabnas::Tabnas>();

    let parser = Arc::new(make_with(MultiSourceOptions::new(sources([
        ("a.jsonic", "a:1"),
        ("b.jsonic", "b:2"),
    ]))));

    let mut handles = Vec::new();
    for index in 0..8 {
        let parser = Arc::clone(&parser);
        handles.push(std::thread::spawn(move || {
            for _ in 0..50 {
                let src = if index % 2 == 0 {
                    "x:@a.jsonic"
                } else {
                    "x:@b.jsonic"
                };
                let value = parser.parse(src).expect("the source loads");
                let want = if index % 2 == 0 {
                    j!({"x":{"a":1}})
                } else {
                    j!({"x":{"b":2}})
                };
                assert_eq!(to_json(&value), want);
            }
        }));
    }
    for handle in handles {
        handle.join().expect("no thread panicked");
    }
}

/// The base path may be given per parse, in the metadata, as the
/// canonical TypeScript tests give it (`{ multisource: { path } }`).
/// That is a TOP-level parse, so the instance still has to be published
/// for the nested one.
#[test]
fn the_base_path_may_come_from_the_parse_metadata() {
    let parser = make_with(MultiSourceOptions::new(sources([
        ("data/a.jsonic", "a:@b.jsonic"),
        ("data/b.jsonic", "b:1"),
    ])));

    let mut meta = indexmap::IndexMap::new();
    let mut entry = indexmap::IndexMap::new();
    entry.insert("path".to_string(), Value::String("data".to_string()));
    meta.insert("multisource".to_string(), Value::object(entry));

    let value = parser
        .parse_with_meta("x:@a.jsonic", Value::object(meta))
        .expect("the metadata base resolves the reference");
    assert_eq!(to_json(&value), j!({"x":{"a":{"b":1}}}));
}
