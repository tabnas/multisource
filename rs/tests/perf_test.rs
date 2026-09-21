// The performance trap this plugin exposes, pinned the way
// ts/test/perf.test.ts and go/perf_test.go pin it: building the instance
// (the engine, the jsonic grammar, the directive and this plugin's own
// alts) dominates a small parse, so a caller must build ONE instance and
// reuse it.
//
// The check is machine-INDEPENDENT: it compares rebuild-per-call against
// instance reuse on the SAME machine in the SAME run, so a slow box
// cannot make it flaky (both sides scale together). There is
// deliberately NO wall-clock budget.
//
// It is also ONE test, deliberately. The harness runs a binary's tests in
// parallel unless told otherwise, and `ci/rust/run.sh` runs a plain
// `cargo test --all-targets` with no `--test-threads`, so splitting the
// two measurements would let one test's expensive rebuild loop overlap
// the other's cheap reuse loop and make the ratio a function of core
// count rather than of the code.

use std::time::{Duration, Instant};

use tabnas::Tabnas;
use tabnas_multisource::{make_with, MapResolver, MultiSourceOptions};

const SRC: &str = "{x:@a.jsonic,y:@b.jsonic,z:3}";
const N: usize = 120;

/// The ratio reuse has to beat. Rebuilding installs jsonic, the
/// directive and this plugin's grammar, so the real margin is far
/// larger; 4x is the floor a regression has to stay above.
const WANT: u32 = 4;

fn build() -> Tabnas {
    make_with(MultiSourceOptions::new(MapResolver::from([
        ("a.jsonic", "a:1"),
        ("b.jsonic", "b:2"),
    ])))
}

/// `N` parses through `parse_once`, timed. Every measurement goes
/// through here so the loops are identical apart from what they call.
fn time_parses(mut parse_once: impl FnMut()) -> Duration {
    let started = Instant::now();
    for _ in 0..N {
        parse_once();
    }
    started.elapsed()
}

#[test]
fn reusing_one_instance_is_far_cheaper_than_rebuilding_per_parse() {
    // Warm both paths so the comparison is steady-state.
    for _ in 0..20 {
        build().parse(SRC).expect("parses");
    }
    let shared = build();
    for _ in 0..50 {
        shared.parse(SRC).expect("parses");
    }

    // The anti-pattern: rebuild the instance on every parse.
    let rebuild = time_parses(|| {
        build().parse(SRC).expect("rebuild parse");
    });

    // The intended pattern: build once, reuse for every parse.
    let reuse = time_parses(|| {
        shared.parse(SRC).expect("reuse parse");
    });

    // Guard the baseline before dividing by it. If the rebuild loop
    // measured as no time at all then nothing was measured, and the
    // ratio below would be vacuously true.
    assert!(
        rebuild > Duration::ZERO,
        "{N} rebuild-per-call parses measured as zero elapsed time: this is \
         testing the clock, not the code"
    );

    let ratio = rebuild.as_secs_f64() / reuse.as_secs_f64().max(f64::EPSILON);
    eprintln!("perf: rebuild-per-call={rebuild:?} reuse={reuse:?} ratio={ratio:.2}x");

    assert!(
        rebuild >= WANT * reuse,
        "reusing one instance is not meaningfully faster than rebuilding per parse: \
         {N} rebuild-per-call parses took {rebuild:?} vs {reuse:?} reusing one instance \
         (ratio {ratio:.1}x, want >={WANT}x). Build one make_with instance and reuse it."
    );
}
