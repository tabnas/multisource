// The real-filesystem cases, over the same `ts/test/*` source files the
// canonical TypeScript suite loads. A shared fixture cannot carry a
// filesystem, so `ts/test/multisource.test.ts` and `go/resolver_test.go`
// cover this ground in their own languages and this file is the Rust
// half.
//
// The files are read, never executed: see tests/divergence_test.rs for
// the one kind that changes, and ../DIVERGENCE.md for why.

mod common;

use std::path::Path;

use tabnas_multisource::{make_with, FileResolver, MultiSourceOptions};

use common::to_json;

fn ts_test_dir() -> String {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("rs/ has a parent")
        .join("ts")
        .join("test")
        .to_string_lossy()
        .into_owned()
}

fn file_parser() -> tabnas::Tabnas {
    make_with(MultiSourceOptions::new(FileResolver::new()).with_path(ts_test_dir()))
}

/// Ports the TypeScript `basic-file` test: a reference beside the base
/// path, at the top level, before a pair, after one, and twice over.
#[test]
fn a_file_reference_splices_wherever_it_stands() {
    let parser = file_parser();
    let cases: [(&str, serde_json::Value); 6] = [
        (
            r#"a:1,b:@"t01.jsonic""#,
            serde_json::json!({"a":1.0,"b":{"c":2.0}}),
        ),
        (r#"@"t01.jsonic""#, serde_json::json!({"c":2.0})),
        (r#"a:1,@"t01.jsonic""#, serde_json::json!({"a":1.0,"c":2.0})),
        (r#"@"t01.jsonic",a:1"#, serde_json::json!({"c":2.0,"a":1.0})),
        (
            r#"a:1,@"t01.jsonic",b:2"#,
            serde_json::json!({"a":1.0,"c":2.0,"b":2.0}),
        ),
        (
            r#"a:1,@"t01.jsonic",b:2,@"t01.jsonic","#,
            serde_json::json!({"a":1.0,"c":2.0,"b":2.0}),
        ),
    ];
    for (src, want) in cases {
        assert_eq!(
            to_json(
                &parser
                    .parse(src)
                    .unwrap_or_else(|error| panic!("{src}: {error}"))
            ),
            want,
            "{src}"
        );
    }
}

/// `t02.jsonic` pulls in a subfolder source and merges another file's
/// keys at its top level, all through paths relative to itself.
#[test]
fn a_loaded_file_resolves_its_own_relative_references() {
    let parser = file_parser();
    assert_eq!(
        to_json(
            &parser
                .parse(r#"a:1,b:@"t02.jsonic",c:3"#)
                .expect("the tree of files loads")
        ),
        serde_json::json!({"a":1.0,"b":{"d":2.0,"e":{"f":4.0},"g":9.0},"c":3.0})
    );
}

/// Ports the TypeScript `file-implicit` test: a reference with no
/// extension finds the `.jsonic` file.
#[test]
fn an_extensionless_file_reference_finds_the_jsonic_file() {
    let parser = file_parser();
    assert_eq!(
        to_json(&parser.parse(r#"a:1,b:@"t01""#).expect("the file loads")),
        serde_json::json!({"a":1.0,"b":{"c":2.0}})
    );
}

/// Ports the TypeScript `file-kind` test for the kinds Rust has: a
/// `.jsonic` source re-parses and a `.json` source reads as JSON.
#[test]
fn the_file_kinds_select_their_processors() {
    let parser = file_parser();
    assert_eq!(
        to_json(
            &parser
                .parse(r#"a:1,b:@"k01.jsonic",f:@"k03.json""#)
                .expect("both files load")
        ),
        serde_json::json!({"a":1.0,"b":{"c":2.0},"f":{"g":4.0}})
    );
}

/// Ports the TypeScript `file-pathfinder` test: the pathfinder rewrites
/// the reference before it is resolved.
#[test]
fn a_pathfinder_rewrites_a_file_reference() {
    let dir = ts_test_dir();
    let parser = make_with(
        MultiSourceOptions::new(
            FileResolver::new().with_pathfinder(move |reference| format!("f01/{reference}")),
        )
        .with_path(dir),
    );
    assert_eq!(
        to_json(
            &parser
                .parse(r#"b:@"f01t01.jsonic""#)
                .expect("the file loads")
        ),
        serde_json::json!({"b":{"f":4.0}})
    );
}

/// Ports the TypeScript `error-file` test: a syntax error inside a
/// loaded file fails the parse, and a syntax error inside a file loaded
/// BY a loaded file fails it too.
#[test]
fn a_syntax_error_inside_a_loaded_file_fails_the_parse() {
    let parser = file_parser();
    for src in [r#"@"e02.jsonic""#, r#"@"e01.jsonic""#] {
        let error = parser.parse(src).expect_err("the loaded file is malformed");
        assert!(!error.code.is_empty(), "{src}: {error}");
    }
}

/// Ports the TypeScript `pkg-relative-ref` test's shape without the
/// package part: a relative reference inside a loaded file resolves
/// against that file's own directory, here one folder down.
#[test]
fn a_relative_reference_inside_a_subfolder_file_resolves_there() {
    let parser = file_parser();
    assert_eq!(
        to_json(
            &parser
                .parse(r#"@"./rel/outer.jsonic""#)
                .expect("the pair of files loads")
        ),
        serde_json::json!({"o":1.0,"inner":{"v":7.0}})
    );
}

/// The preload scan reads the real `ts/test` folder: the extensions
/// asked for, recursively. Ports the TypeScript `preload-basic`,
/// `preload-extensions` and `preload-recursive` tests.
#[test]
fn a_preload_reads_the_real_folder() {
    use tabnas_multisource::{preload_files, PreloadOptions};

    let dir = ts_test_dir();
    let filemap = preload_files(
        &PreloadOptions::new([dir.clone()])
            .with_ext([".jsonic"])
            .with_recursive(true),
    );
    assert!(
        filemap.keys().any(|key| key.ends_with("t01.jsonic")),
        "the scan found the .jsonic files: {:?}",
        filemap.keys().collect::<Vec<_>>()
    );
    assert!(
        filemap.keys().any(|key| key.contains("f01")),
        "a recursive scan descends"
    );
    assert!(
        !filemap.keys().any(|key| key.ends_with(".js")),
        "and loads only the extensions asked for"
    );

    let flat = preload_files(&PreloadOptions::new([dir.clone()]).with_ext([".jsonic"]));
    assert!(
        !flat.keys().any(|key| key.contains("f01")),
        "a non-recursive scan stays at the root: {:?}",
        flat.keys().collect::<Vec<_>>()
    );

    let defaults = preload_files(&PreloadOptions::new([dir]));
    assert!(defaults.keys().any(|key| key.ends_with(".jsonic")));
    assert!(defaults.keys().any(|key| key.ends_with(".json")));
    assert!(!defaults.keys().any(|key| key.ends_with(".js")));
}

/// A preloaded map serves the parse with the files gone from the disk,
/// which is what proves the parse read no file. Ports the TypeScript
/// and Go `preload-file-resolver` tests.
#[test]
fn a_preload_serves_the_parse_after_the_files_are_gone() {
    use tabnas_multisource::{preload_files, PreloadOptions};

    let dir = std::env::temp_dir().join(format!(
        "tabnas-multisource-preload-{}-{:?}",
        std::process::id(),
        std::thread::current().id()
    ));
    std::fs::create_dir_all(dir.join("f01")).expect("a scratch folder");
    std::fs::write(dir.join("t01.jsonic"), "c:2").expect("a scratch file");
    std::fs::write(dir.join("f01").join("f01t01.jsonic"), "f:4").expect("a scratch file");

    let folder = dir.to_string_lossy().into_owned();
    let filemap = preload_files(
        &PreloadOptions::new([folder.clone()])
            .with_ext([".jsonic"])
            .with_recursive(true),
    );
    assert_eq!(filemap.len(), 2, "{filemap:?}");

    std::fs::remove_dir_all(&dir).expect("the scratch folder is removed");

    let parser = make_with(
        MultiSourceOptions::new(FileResolver::new().with_preload(filemap))
            .with_path(folder)
            .with_preload(
                PreloadOptions::new([dir.to_string_lossy().into_owned()])
                    .with_ext([".jsonic"])
                    .with_recursive(true),
            ),
    );
    assert_eq!(
        to_json(
            &parser
                .parse(r#"@"t01.jsonic""#)
                .expect("the preloaded file loads")
        ),
        serde_json::json!({"c":2.0})
    );
}

/// The package resolver over the real filesystem: a sub-path reference,
/// a folder index, and a package's `package.json` `main`, each found by
/// walking up from a nested directory. Ports `go/resolver_test.go`
/// `TestPkgResolver*`.
#[test]
fn the_package_resolver_reads_real_node_modules() {
    use tabnas_multisource::PkgResolver;

    let dir = std::env::temp_dir().join(format!(
        "tabnas-multisource-pkg-{}-{:?}",
        std::process::id(),
        std::thread::current().id()
    ));
    let modules = dir.join("node_modules");
    let write = |path: std::path::PathBuf, content: &str| {
        std::fs::create_dir_all(path.parent().expect("a parent")).expect("a scratch folder");
        std::fs::write(path, content).expect("a scratch file");
    };
    write(modules.join("mypkg").join("zed.jsonic"), "{zed:99}");
    write(modules.join("idxpkg").join("index.jsonic"), "{i:5}");
    write(
        modules.join("mainpkg").join("package.json"),
        r#"{"main":"main.jsonic"}"#,
    );
    write(modules.join("mainpkg").join("main.jsonic"), "{z:11}");
    let nested = dir.join("a").join("b").join("c");
    std::fs::create_dir_all(&nested).expect("a nested scratch folder");

    let parser = make_with(MultiSourceOptions::new(
        PkgResolver::new().with_paths([nested.to_string_lossy().into_owned()]),
    ));

    let cases: [(&str, serde_json::Value); 3] = [
        (
            r#"{c:@"mypkg/zed.jsonic"}"#,
            serde_json::json!({"zed":99.0}),
        ),
        (r#"{c:@"idxpkg"}"#, serde_json::json!({"i":5.0})),
        (r#"{c:@"mainpkg"}"#, serde_json::json!({"z":11.0})),
    ];
    for (src, want) in cases {
        assert_eq!(
            to_json(
                &parser
                    .parse(src)
                    .unwrap_or_else(|error| panic!("{src}: {error}"))
            )["c"],
            want,
            "{src}"
        );
    }

    assert_eq!(
        parser
            .parse(r#"{x:@"nopkg/zed.jsonic"}"#)
            .expect_err("nothing installs nopkg")
            .code,
        "multisource_not_found"
    );

    std::fs::remove_dir_all(&dir).expect("the scratch folder is removed");
}
