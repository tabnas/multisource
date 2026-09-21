// The Rust side of every recorded divergence, pinned so a repair fails
// here and names the row in ../DIVERGENCE.md to delete.
//
// Neither divergence is expressible as a shared fixture: one needs a
// real filesystem AND a JavaScript runtime, the other an input far
// larger than a cell. ../DIVERGENCE.md says so, and these tests are what
// it points at instead.

mod common;

use std::collections::BTreeMap;
use std::path::Path;

use tabnas_multisource::{make_with, FileResolver, MapResolver, MultiSourceOptions};

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

/// Divergence 1: there is no `js` kind.
///
/// `ts/test/multisource.test.ts` `file-kind` asserts that TypeScript
/// reads `ts/test/k02.js` as `{e: 3}`, because its `js` processor calls
/// `require` on the file, which is to say executes it. Rust has no
/// JavaScript runtime, so the reference falls through to the raw-text
/// processor, exactly as it does in Go.
#[test]
fn a_js_source_is_raw_text_here() {
    let parser = make_with(MultiSourceOptions::new(FileResolver::new()).with_path(ts_test_dir()));
    let value = parser
        .parse(r#"d:@"k02.js""#)
        .expect("the file is read, just not executed");
    let json = to_json(&value);
    assert_eq!(
        json["d"].as_str().map(str::trim),
        Some("module.exports = {\n  e: 3\n}"),
        "the module text is the value, not the module's exports"
    );
}

/// Divergence 1, second half: `.js` is not an implicit extension.
///
/// TypeScript's `implictExt` is `.jsonic .jsc .json .js`, so an
/// extensionless `@k02` finds `k02.js` there. The Rust list ends at
/// `.json`, as the Go list does, because finding the file would only
/// lead to a kind that cannot be processed.
#[test]
fn a_js_file_is_not_found_by_an_extensionless_reference() {
    let parser = make_with(MultiSourceOptions::new(FileResolver::new()).with_path(ts_test_dir()));
    let error = parser
        .parse(r#"d:@"k02""#)
        .expect_err("nothing but a .js file is there");
    assert_eq!(error.code, "multisource_not_found");
    assert!(
        !error.hint.lines().any(|line| line.ends_with(".js")),
        "the search does not consider .js: {}",
        error.hint
    );
}

/// Divergence 2: a chain of sources is capped.
///
/// Go resolves a 4000-deep acyclic chain to `{"end":1}`: a goroutine
/// stack grows on demand. A Rust thread's stack does not, and a nested
/// source is parsed inside the parse that referenced it, so the same
/// input aborted the process. The cap turns that into an error the
/// caller can handle.
#[test]
fn a_chain_deeper_than_the_cap_is_an_error() {
    let depth = 4_000;
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

/// The same chain, well inside the cap, still resolves. A cap that
/// rejected ordinary composition would be a defect, not a guard.
#[test]
fn a_chain_inside_the_cap_still_resolves() {
    let depth = 32;
    let mut files: BTreeMap<String, String> = BTreeMap::new();
    for level in 0..depth {
        files.insert(
            format!("s{level}.jsonic"),
            format!(r#"@"s{}.jsonic""#, level + 1),
        );
    }
    files.insert(format!("s{depth}.jsonic"), "end:1".to_string());

    let parser = make_with(MultiSourceOptions::new(MapResolver::from_map(files)));
    let value = parser
        .parse(r#"@"s0.jsonic""#)
        .expect("a chain inside the cap resolves");
    assert_eq!(to_json(&value), serde_json::json!({ "end": 1.0 }));
}

/// Not a divergence, and here to keep it from becoming one: a processor
/// entry may name another kind. TypeScript allows one level of that
/// aliasing and Go cannot express it, so Go reads `ts/test/t04.foo` as
/// raw text where TypeScript reads it as `{a: 1}`. Rust matches
/// TypeScript.
#[test]
fn a_processor_alias_matches_typescript_where_go_cannot() {
    let parser = make_with(
        MultiSourceOptions::new(FileResolver::new())
            .with_path(ts_test_dir())
            .with_processor_alias("foo", "jsonic"),
    );
    let value = parser.parse(r#"@"t04.foo""#).expect("the alias resolves");
    assert_eq!(to_json(&value), serde_json::json!({ "a": 1.0 }));
}
