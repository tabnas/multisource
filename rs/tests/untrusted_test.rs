// Untrusted input: a document, and every source it names, is hostile
// text. None of it may panic, hang, overflow the stack or take
// super-linear time, and none of it may reach outside the resolver the
// caller configured.
//
// See "Untrusted input" in ../AGENTS.md and section 7 of the porting
// playbook.

mod common;

use std::collections::BTreeMap;
use std::time::{Duration, Instant};

use tabnas_multisource::{
    make_with, FileResolver, MapFs, MapResolver, MultiSourceOptions, PkgResolver,
};

fn sources<const N: usize>(entries: [(&str, &str); N]) -> MapResolver {
    MapResolver::from(entries)
}

/// Whatever the outcome, the process survives it and says something.
fn survives(parser: &tabnas::Tabnas, src: &str) {
    match parser.parse(src) {
        Ok(_) => {}
        Err(error) => assert!(!error.code.is_empty(), "an error carries a code: {src:?}"),
    }
}

#[test]
fn odd_documents_do_not_panic() {
    let parser = make_with(MultiSourceOptions::new(sources([("a.jsonic", "a:1")])));
    let cases = [
        "",
        " ",
        "@",
        "@@",
        "@@@@@@@@",
        "x:@",
        "@:",
        "@,",
        "{@",
        "[@",
        "@\"",
        "@\"unterminated",
        "@'",
        "@`",
        "x:@\0",
        "@\u{0007}\u{0008}",
        "@\u{feff}a.jsonic",
        "@ñ.jsonic",
        "@🙂",
        "@\\ud800",
        "a:b:c:d:e:@a.jsonic",
        "@a.jsonic@a.jsonic",
        "@../../../../../../etc/passwd",
        "@//////",
        "@.",
        "@..",
        "@./",
        "@a.",
        "@.jsonic",
    ];
    for src in cases {
        survives(&parser, src);
    }
}

#[test]
fn odd_source_content_does_not_panic() {
    for content in [
        "",
        "@",
        "@\"",
        "\0",
        "\u{feff}",
        "{{{{{{{{",
        "[[[[[[[[",
        "a:",
        ":",
        "\"unterminated",
    ] {
        let parser = make_with(MultiSourceOptions::new(MapResolver::from([(
            "a.jsonic", content,
        )])));
        survives(&parser, "@a.jsonic");
        survives(&parser, "x:@a.jsonic");
        survives(&parser, "@a.jsonic y:1");
    }
}

#[test]
fn a_very_long_reference_is_reported_not_crashed() {
    let parser = make_with(MultiSourceOptions::new(MapResolver::new()));
    let long = "a".repeat(100_000);
    let error = parser
        .parse(&format!("x:@\"{long}\""))
        .expect_err("nothing resolves it");
    assert_eq!(error.code, "multisource_not_found");
}

#[test]
fn a_very_long_source_is_read_not_crashed() {
    let items: String = (0..20_000)
        .map(|index| format!("k{index}:{index},"))
        .collect();
    let parser = make_with(MultiSourceOptions::new(MapResolver::from([(
        "big.jsonic",
        items.as_str(),
    )])));
    let value = parser.parse("@big.jsonic").expect("a large source loads");
    assert_eq!(
        common::to_json(&value)
            .as_object()
            .map(serde_json::Map::len),
        Some(20_000)
    );
}

/// Deeply nested VALUE content inside a source is the host grammar's
/// problem, and the host grammar bounds it. What matters here is that it
/// comes back as an error rather than as an aborted process.
#[test]
fn deeply_nested_source_content_is_rejected_not_crashed() {
    let deep = format!("{}{}", "[".repeat(5_000), "]".repeat(5_000));
    let parser = make_with(MultiSourceOptions::new(MapResolver::from([(
        "deep.jsonic",
        deep.as_str(),
    )])));
    let error = parser
        .parse("@deep.jsonic")
        .expect_err("the host grammar bounds the nesting");
    assert!(!error.code.is_empty());
}

/// A chain of sources with no cycle in it is bounded by `max_depth`, so
/// neither the caller's stack nor the process is at risk however long
/// the chain the document asks for.
#[test]
fn an_unbounded_chain_stops_at_the_cap() {
    let depth = 5_000;
    let mut files: BTreeMap<String, String> = BTreeMap::new();
    for level in 0..depth {
        files.insert(
            format!("s{level}.jsonic"),
            format!(r#"@"s{}.jsonic""#, level + 1),
        );
    }
    files.insert(format!("s{depth}.jsonic"), "end:1".to_string());

    let parser = make_with(MultiSourceOptions::new(MapResolver::from_map(files)));
    let error = parser
        .parse(r#"@"s0.jsonic""#)
        .expect_err("the chain is capped");
    assert_eq!(error.code, "multisource_depth");
}

/// A source that includes itself many times over, from many branches,
/// is reuse at each branch and a cycle down each one. Neither the work
/// nor the report may explode.
#[test]
fn a_wide_diamond_is_reuse_and_stays_linear() {
    let width = 200;
    let mut files: BTreeMap<String, String> = BTreeMap::new();
    files.insert("base.jsonic".to_string(), "x:1".to_string());
    let body: String = (0..width)
        .map(|index| format!(r#"k{index}:@"base.jsonic","#))
        .collect();
    files.insert("wide.jsonic".to_string(), body);

    let parser = make_with(MultiSourceOptions::new(MapResolver::from_map(files)));
    let started = Instant::now();
    let value = parser.parse(r#"@"wide.jsonic""#).expect("a diamond loads");
    let elapsed = started.elapsed();

    assert_eq!(
        common::to_json(&value)
            .as_object()
            .map(serde_json::Map::len),
        Some(width)
    );
    assert!(
        elapsed < Duration::from_secs(30),
        "a wide diamond should not take {elapsed:?}"
    );
}

/// The resolver is the sandbox, and it holds for a document that tries
/// every shape of escape, including one written into a loaded source.
#[test]
fn nothing_escapes_the_root() {
    let disk = MapFs::from([
        ("srv/app/main.jsonic", r#"{ok:@"./leaf.jsonic"}"#),
        ("srv/app/leaf.jsonic", "{leaf:1}"),
        ("srv/app/climb.jsonic", r#"{x:@"../../secret.jsonic"}"#),
        ("srv/secret.jsonic", "{leak:1}"),
        ("secret.jsonic", "{leak:1}"),
    ]);
    let parser = make_with(
        MultiSourceOptions::new(FileResolver::new().with_fs(disk).with_root("srv/app"))
            .with_path("srv/app"),
    );

    assert_eq!(
        common::to_json(
            &parser
                .parse(r#"@"main.jsonic""#)
                .expect("the root is reachable")
        ),
        serde_json::json!({ "ok": { "leaf": 1.0 } })
    );

    for escape in [
        r#"x:@"../secret.jsonic""#,
        r#"x:@"../../secret.jsonic""#,
        r#"x:@"/secret.jsonic""#,
        r#"x:@"./sub/../../secret.jsonic""#,
        r#"x:@"climb.jsonic""#,
        r#"x:@"..""#,
        r#"x:@"../""#,
    ] {
        let error = parser.parse(escape).expect_err("the root holds");
        assert_eq!(
            error.code, "multisource_not_found",
            "{escape} must not escape the root"
        );
    }
}

/// The package resolver is confined the same way, so a reference cannot
/// climb out of a vendored tree.
#[test]
fn the_package_resolver_is_confined_too() {
    let disk = MapFs::from([
        ("srv/node_modules/inside/index.jsonic", "{in:1}"),
        ("node_modules/outside/index.jsonic", "{out:1}"),
    ]);
    let parser = make_with(
        MultiSourceOptions::new(PkgResolver::new().with_paths(["srv"]).with_root("srv"))
            .with_fs(disk),
    );
    assert_eq!(
        common::to_json(
            &parser
                .parse(r#"x:@"inside""#)
                .expect("the vendored tree is reachable")
        )["x"],
        serde_json::json!({ "in": 1.0 })
    );
    assert_eq!(
        parser
            .parse(r#"x:@"outside""#)
            .expect_err("nothing outside the root")
            .code,
        "multisource_not_found"
    );
}

/// A key named `__proto__` in a loaded source is a key, not a way to
/// reach the merge target's shape. The engine's own merge drops the
/// dangerous names; this pins that the plugin's splice goes through it.
#[test]
fn prototype_keys_do_not_escape_the_value() {
    let parser = make_with(MultiSourceOptions::new(sources([(
        "a.jsonic",
        r#"{"__proto__":{"polluted":1},"ok":2}"#,
    )])));
    let value = parser.parse("x:1 @a.jsonic").expect("the source loads");
    let json = common::to_json(&value);
    assert_eq!(json["ok"], serde_json::json!(2.0));
    assert_eq!(json["x"], serde_json::json!(1.0));
    assert!(
        json.get("polluted").is_none(),
        "no key leaked out of __proto__: {json}"
    );
}

/// A reference that resolves to an empty source is a value, not a
/// failure, and a source of nothing but whitespace is the same.
#[test]
fn an_empty_source_resolves() {
    let parser = make_with(MultiSourceOptions::new(sources([
        ("empty.jsonic", ""),
        ("space.jsonic", "   \n\t  "),
        ("empty.txt", ""),
    ])));
    for src in ["x:@empty.jsonic", "x:@space.jsonic", "x:@empty.txt"] {
        parser.parse(src).unwrap_or_else(|error| {
            panic!("{src}: an empty source is a value, not a failure: {error}")
        });
    }
}

/// The root confinement has to hold against a REAL filesystem, not only
/// against the in-memory map: a map has no symbolic links, so it cannot
/// exercise the guarantee at all. Here a link inside the root points at a
/// directory outside it, and a second link points straight at a file
/// outside it. Neither may be readable through the root.
#[cfg(unix)]
#[test]
fn a_symlink_cannot_leave_the_root_on_a_real_filesystem() {
    let base = common::scratch_dir("symlink-root");
    let root = base.join("root");
    let outside = base.join("outside");
    std::fs::create_dir_all(&root).expect("a scratch root");
    std::fs::create_dir_all(&outside).expect("a scratch outside");
    std::fs::write(root.join("main.jsonic"), "{ok:1}").expect("a scratch file");
    std::fs::write(outside.join("secret.jsonic"), "{leak:1}").expect("a scratch file");
    std::os::unix::fs::symlink(&outside, root.join("link")).expect("a directory link");
    std::os::unix::fs::symlink(outside.join("secret.jsonic"), root.join("leak.jsonic"))
        .expect("a file link");
    std::os::unix::fs::symlink(outside.join("gone.jsonic"), root.join("dangle.jsonic"))
        .expect("a dangling link");

    let root_path = root.to_string_lossy().into_owned();
    let parser = make_with(
        MultiSourceOptions::new(FileResolver::new().with_root(root_path.clone()))
            .with_path(root_path),
    );

    assert_eq!(
        common::to_json(
            &parser
                .parse(r#"@"main.jsonic""#)
                .expect("a real file inside the root still loads")
        ),
        serde_json::json!({ "ok": 1.0 })
    );

    for escape in [
        r#"x:@"link/secret.jsonic""#,
        r#"x:@"leak.jsonic""#,
        r#"x:@"./link/secret.jsonic""#,
        r#"x:@"dangle.jsonic""#,
    ] {
        let error = parser.parse(escape).expect_err("the root holds");
        assert_eq!(
            error.code, "multisource_not_found",
            "{escape} must not escape the root through a link"
        );
    }

    std::fs::remove_dir_all(&base).expect("the scratch folder is removed");
}

/// A root may be spelled relatively, and then it has to name the same
/// directory a candidate does. The file resolver makes every candidate
/// absolute through the filesystem, so a root left as written matched
/// nothing at all and every reference INSIDE the root was reported
/// missing, which reads as a sandbox that works and is a sandbox that
/// has locked the door on the house.
#[test]
fn a_relative_root_confines_a_real_filesystem_without_hiding_it() {
    let (base, relative) = common::cwd_scratch_dir("relative-root");
    let app = base.join("srv").join("app");
    std::fs::create_dir_all(&app).expect("a scratch root");
    std::fs::write(app.join("main.jsonic"), r#"{ok:@"./leaf.jsonic"}"#).expect("a scratch file");
    std::fs::write(app.join("leaf.jsonic"), "{leaf:1}").expect("a scratch file");
    std::fs::write(base.join("secret.jsonic"), "{leak:1}").expect("a scratch file");

    let root = format!("{relative}/srv/app");
    let parser = make_with(
        MultiSourceOptions::new(FileResolver::new().with_root(root.clone())).with_path(root),
    );

    assert_eq!(
        common::to_json(
            &parser
                .parse(r#"@"main.jsonic""#)
                .expect("a relative root still reaches its own files")
        ),
        serde_json::json!({ "ok": { "leaf": 1.0 } })
    );

    for escape in [
        r#"x:@"../../secret.jsonic""#,
        r#"x:@"/etc/passwd""#,
        r#"x:@"./sub/../../../secret.jsonic""#,
    ] {
        assert_eq!(
            parser.parse(escape).expect_err("the root holds").code,
            "multisource_not_found",
            "{escape} must not escape the root"
        );
    }

    std::fs::remove_dir_all(&base).expect("the scratch folder is removed");
}

/// The package resolver advertises the same confinement, so it gets the
/// same real filesystem: a vendored `node_modules` entry that is a link
/// to a package outside the root is not reachable through it.
#[cfg(unix)]
#[test]
fn a_symlink_cannot_leave_the_package_root_either() {
    let base = common::scratch_dir("symlink-pkg");
    let root = base.join("root");
    let modules = root.join("node_modules");
    let elsewhere = base.join("elsewhere");
    std::fs::create_dir_all(modules.join("inside")).expect("a vendored package");
    std::fs::create_dir_all(&elsewhere).expect("a package outside the root");
    std::fs::write(modules.join("inside").join("index.jsonic"), "{in:1}").expect("a scratch file");
    std::fs::write(elsewhere.join("index.jsonic"), "{out:1}").expect("a scratch file");
    std::os::unix::fs::symlink(&elsewhere, modules.join("outside")).expect("a package link");

    let root_path = root.to_string_lossy().into_owned();
    let parser = make_with(MultiSourceOptions::new(
        PkgResolver::new()
            .with_paths([root_path.clone()])
            .with_root(root_path),
    ));

    assert_eq!(
        common::to_json(
            &parser
                .parse(r#"x:@"inside""#)
                .expect("the vendored package is reachable")
        )["x"],
        serde_json::json!({ "in": 1.0 })
    );
    assert_eq!(
        parser
            .parse(r#"x:@"outside""#)
            .expect_err("the link does not reach out of the root")
            .code,
        "multisource_not_found"
    );

    std::fs::remove_dir_all(&base).expect("the scratch folder is removed");
}
