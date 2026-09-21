// Composition: the multisource plugin layered with the official
// tabnas-debug plugin, asserting the structured grammar model. Ports
// ts/test/debug-model.test.ts.
//
// The Rust port takes tabnas-debug as a dev-dependency on a sibling
// checkout rather than loading it dynamically, so this test never skips:
// a skip that nothing notices is how a composition stops being checked.

use tabnas_debug::{model, DebugOptions};
use tabnas_multisource::{multisource, MapResolver, MultiSourceOptions};

fn composed() -> tabnas::Tabnas {
    let mut parser = tabnas_jsonic::make();
    multisource(
        &mut parser,
        MultiSourceOptions::new(MapResolver::from([("a.jsonic", "a:1")])),
    )
    .expect("the MultiSource plugin installs");
    tabnas_debug::apply(&mut parser, DebugOptions::quiet()).expect("the Debug plugin installs");
    parser
}

#[test]
fn it_parses_normally_with_the_debug_plugin_installed() {
    let parser = composed();
    assert_eq!(
        parser.parse(r#"{"a":[1,2]}"#).expect("parses").to_string(),
        r#"{"a":[1,2]}"#
    );
    assert_eq!(
        parser
            .parse(r#"x:@"a.jsonic""#)
            .expect("parses")
            .to_string(),
        r#"{"x":{"a":1}}"#
    );
}

#[test]
fn the_model_reports_the_grammar() {
    let parser = composed();
    let model = model(&parser);

    // The rule set: the shared jsonic rules plus this plugin's own.
    let mut names: Vec<String> = model.rules.iter().map(|rule| rule.name.clone()).collect();
    names.sort();
    assert_eq!(names, ["elem", "list", "map", "multisource", "pair", "val"]);

    assert_eq!(model.config.start, "val");
    assert!(
        model
            .plugins
            .iter()
            .any(|plugin| plugin.name == "MultiSource"),
        "the plugins list names MultiSource: {:?}",
        model
            .plugins
            .iter()
            .map(|plugin| &plugin.name)
            .collect::<Vec<_>>()
    );

    // The structural facts specific to this grammar: the marked
    // directive wires `val` into a `multisource` rule, which in turn
    // parses a `val` for the resolved source.
    let val = model
        .rules
        .iter()
        .find(|rule| rule.name == "val")
        .expect("a val rule");
    assert!(
        val.open
            .iter()
            .any(|alt| alt.push.as_deref() == Some("multisource")),
        "val pushes multisource"
    );

    let rule = model
        .rules
        .iter()
        .find(|rule| rule.name == "multisource")
        .expect("a multisource rule");
    assert!(
        rule.open
            .iter()
            .any(|alt| alt.push.as_deref() == Some("val")),
        "multisource pushes val"
    );

    // The model is serializable and round-trips.
    let encoded = serde_json::to_string(&model).expect("the model serializes");
    let decoded: serde_json::Value = serde_json::from_str(&encoded).expect("and reads back");
    assert!(decoded.get("rules").is_some());
}
