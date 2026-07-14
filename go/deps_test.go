/* Copyright (c) 2025 Richard Rodger, MIT License */

package tabnasmultisource

import (
	"encoding/json"
	"testing"
)

// TestDeps ports the TypeScript 'deps' test: nested sources (a -> b -> c)
// parse to the same value regardless of the parse meta passed, and, when an
// empty DependencyMap is supplied under multisource.deps, the plugin fills it
// with one Dependency record per resolved (target, source) edge, keyed by TOP
// at the top level.
func TestDeps(t *testing.T) {
	files := map[string]string{
		"a.jsc":       `a:1,b:@b.jsc,x:99`,
		"b.jsc":       `b:2,c:@c`,
		"c/index.jsc": `c:3`,
	}

	j := MakeJsonic(MultiSourceOptions{Resolver: MakeMemResolver(files)})

	want := map[string]any{
		"a": float64(1),
		"b": map[string]any{
			"b": float64(2),
			"c": map[string]any{"c": float64(3)},
		},
		"x": float64(99),
	}

	// Mirrors: j.parse('@a'), j.parse('@a', {}), j.parse('@a', {x:1}),
	// j.parse('@a', {multisource:{path:undefined}}).
	metas := []map[string]any{
		nil,
		{},
		{"x": 1},
		{"multisource": map[string]any{}},
	}
	for i, meta := range metas {
		r, err := j.ParseMeta(`@a`, meta)
		if err != nil {
			t.Fatalf("meta %d: %v", i, err)
		}
		assert(t, "deps-parse", r, want)
	}

	// Pass an empty DependencyMap to be filled during the parse.
	deps := DependencyMap{}
	r, err := j.ParseMeta(`@a`, map[string]any{
		"multisource": map[string]any{"deps": deps},
	})
	if err != nil {
		t.Fatal(err)
	}
	assert(t, "deps-parse-tracked", r, want)

	if len(deps) != 3 {
		t.Fatalf("deps targets: want 3, got %d: %#v", len(deps), deps)
	}

	checkDep := func(tar, src string) {
		t.Helper()
		dep, ok := deps[tar][src]
		if !ok {
			t.Fatalf("missing dep %q -> %q in %#v", tar, src, deps)
		}
		if dep.Tar != tar || dep.Src != src {
			t.Fatalf("dep fields: want {%q %q}, got %#v", tar, src, dep)
		}
		if dep.Wen <= 0 {
			t.Fatalf("dep %q -> %q: Wen not set: %#v", tar, src, dep)
		}
	}

	// The top-level parse pulled in a.jsc, a.jsc pulled in b.jsc, and b.jsc
	// pulled in c/index.jsc (via the folder index file).
	checkDep(TOP, "a.jsc")
	checkDep("a.jsc", "b.jsc")
	checkDep("b.jsc", "c/index.jsc")

	if len(deps[TOP]) != 1 || len(deps["a.jsc"]) != 1 || len(deps["b.jsc"]) != 1 {
		t.Fatalf("unexpected extra deps: %#v", deps)
	}
}

// TestDepsNested checks the dependency tree when a single source is referenced
// from more than one target, and that repeated parses into the same map merge.
func TestDepsNested(t *testing.T) {
	files := map[string]string{
		"main.jsonic":   `{p:@one.jsonic, q:@two.jsonic}`,
		"one.jsonic":    `{o:@shared.jsonic}`,
		"two.jsonic":    `{t:@shared.jsonic}`,
		"shared.jsonic": `{s:1}`,
	}

	j := MakeJsonic(MultiSourceOptions{Resolver: MakeMemResolver(files)})

	deps := DependencyMap{}
	_, err := j.ParseMeta(`@main.jsonic`, map[string]any{
		"multisource": map[string]any{"deps": deps},
	})
	if err != nil {
		t.Fatal(err)
	}

	assert(t, "deps-top-src", deps[TOP]["main.jsonic"].Src, "main.jsonic")
	assert(t, "deps-one", deps["main.jsonic"]["one.jsonic"].Tar, "main.jsonic")
	assert(t, "deps-two", deps["main.jsonic"]["two.jsonic"].Tar, "main.jsonic")
	assert(t, "deps-shared-one", deps["one.jsonic"]["shared.jsonic"].Src, "shared.jsonic")
	assert(t, "deps-shared-two", deps["two.jsonic"]["shared.jsonic"].Src, "shared.jsonic")
}

// TestDependencyJSONShape checks the JSON-compatible shape of a Dependency
// record (tar/src/wen), matching the TypeScript field names.
func TestDependencyJSONShape(t *testing.T) {
	b, err := json.Marshal(Dependency{Tar: "a", Src: "b", Wen: 123})
	if err != nil {
		t.Fatal(err)
	}
	assert(t, "dep-json", string(b), `{"tar":"a","src":"b","wen":123}`)
}
