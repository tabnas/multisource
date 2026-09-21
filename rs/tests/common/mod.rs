// Shared test helpers. Cargo compiles this module into EVERY integration
// test binary, so an item only one binary uses is dead code in the
// others; the allow keeps that from being a warning rather than hiding
// anything real.
#![allow(dead_code)]
// The engine's error is large by design (it carries the whole report),
// and the engine allows this lint at its own crate root for the same
// reason.
#![allow(clippy::result_large_err)]

use std::collections::BTreeMap;
use std::path::Path;

use serde_json::Value as Json;
use tabnas::{Tabnas, Value};
use tabnas_multisource::{make_with, MapResolver, MultiSourceOptions};
use tabnas_support::Failure;

/// The repository root: the parent of `rs/`.
pub fn repo_root() -> &'static Path {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("rs/ has a parent")
}

/// A fresh, empty scratch directory under the system temporary folder,
/// for a test that needs a REAL filesystem rather than a map. The name
/// carries the process, the thread and a counter, so two tests running
/// at once never share one.
pub fn scratch_dir(label: &str) -> std::path::PathBuf {
    use std::sync::atomic::{AtomicUsize, Ordering};
    static NEXT: AtomicUsize = AtomicUsize::new(0);
    let dir = std::env::temp_dir().join(format!(
        "tabnas-multisource-{label}-{}-{:?}-{}",
        std::process::id(),
        std::thread::current().id(),
        NEXT.fetch_add(1, Ordering::Relaxed)
    ));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("a scratch folder");
    dir
}

/// A fresh scratch directory BELOW the working directory, returned both
/// absolutely and as the relative path that names it from there. A test
/// of a relative root needs one: the working directory of a test binary
/// is the crate root, and moving it is process-global, which a suite
/// running tests in parallel cannot do.
pub fn cwd_scratch_dir(label: &str) -> (std::path::PathBuf, String) {
    use std::sync::atomic::{AtomicUsize, Ordering};
    static NEXT: AtomicUsize = AtomicUsize::new(0);
    let relative = format!(
        "target/scratch/{label}-{}-{:?}-{}",
        std::process::id(),
        std::thread::current().id(),
        NEXT.fetch_add(1, Ordering::Relaxed)
    );
    let absolute = std::env::current_dir()
        .expect("a working directory")
        .join(&relative);
    let _ = std::fs::remove_dir_all(&absolute);
    std::fs::create_dir_all(&absolute).expect("a scratch folder");
    (absolute, relative)
}

/// A fresh parser over the in-memory source set a fixture row names,
/// plus the small set of engine options a row may carry.
pub fn parser_for(options: &Json) -> Result<Tabnas, Failure> {
    let mut sources: BTreeMap<String, String> = BTreeMap::new();
    if let Some(entries) = options.get("mem").and_then(Json::as_object) {
        for (path, content) in entries {
            if let Some(content) = content.as_str() {
                sources.insert(path.clone(), content.to_string());
            }
        }
    }

    let mut parser = make_with(MultiSourceOptions::new(MapResolver::from_map(sources)));

    if let Some(engine) = options.get("options") {
        apply_spec_options(&mut parser, engine)?;
    }

    Ok(parser)
}

/// Apply the small set of engine options a fixture may carry. Kept
/// explicit rather than generic: only options a case actually needs
/// belong here, and each must have a TypeScript counterpart.
fn apply_spec_options(parser: &mut Tabnas, engine: &Json) -> Result<(), Failure> {
    if let Some(extend) = engine.pointer("/map/extend").and_then(Json::as_bool) {
        parser
            .set_options(|options| options.map.extend = extend)
            .map_err(|error| Failure::message(format!("spec options: {error}")))?;
        return Ok(());
    }
    Err(Failure::message(format!(
        "spec options not supported by the runner: {engine}"
    )))
}

/// An engine value flattened through JSON, as the Go runner's
/// `jsonFlatten` does: the `MapRef` / `ListRef` / `Text` wrappers become
/// their plain values and key order stops mattering.
pub fn to_json(value: &Value) -> Json {
    canon(value.to_json())
}

/// Every number as a float.
///
/// The engine holds numbers as `f64`, so a parse result carries `1.0`
/// where a `json!` literal carries the integer `1`, and `serde_json`
/// compares those two as different. JSON itself does not distinguish
/// them, and neither do the TypeScript and Go suites, so both sides are
/// read the same way here rather than written differently.
pub fn canon(value: Json) -> Json {
    match value {
        Json::Number(number) => match number.as_f64() {
            Some(number) => serde_json::Number::from_f64(number)
                .map(Json::Number)
                .unwrap_or(Json::Null),
            None => Json::Null,
        },
        Json::Array(items) => Json::Array(items.into_iter().map(canon).collect()),
        Json::Object(entries) => Json::Object(
            entries
                .into_iter()
                .map(|(key, item)| (key, canon(item)))
                .collect(),
        ),
        other => other,
    }
}

/// A parse error as the runner's failure: the code the fixture pins,
/// and the rendered report for the failure message.
pub fn to_failure(error: tabnas::TabnasError) -> Failure {
    Failure::new(error.code.clone())
        .at(error.row, error.col)
        .with_message(error.to_string())
}
