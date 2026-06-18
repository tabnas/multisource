/* Copyright (c) 2025 Richard Rodger, MIT License */

package multisource

import (
	"reflect"
	"strings"
	"testing"

	jsonic "github.com/tabnas/jsonic/go"
	path "github.com/tabnas/path/go"
)

// assert is a test helper that checks deep equality.
func assert(t *testing.T, name string, got, want any) {
	t.Helper()
	if !reflect.DeepEqual(got, want) {
		t.Errorf("%s:\n  got:  %#v\n  want: %#v", name, got, want)
	}
}

func TestHappy(t *testing.T) {
	files := map[string]string{
		"a.jsonic": `{a:1}`,
		"b.jsc":    `{b:2}`,
		"c.txt":    `CCC`,
		"d.json":   `{"d":3}`,
	}

	j := MakeJsonic(MultiSourceOptions{
		Resolver: MakeMemResolver(files),
	})

	r, err := j.Parse(`{a: @a.jsonic}`)
	if err != nil {
		t.Fatal(err)
	}
	m, _ := r.(map[string]any)
	assert(t, "jsonic-ref", m["a"], map[string]any{"a": float64(1)})

	r, err = j.Parse(`{c: @c.txt}`)
	if err != nil {
		t.Fatal(err)
	}
	m, _ = r.(map[string]any)
	assert(t, "txt-ref", m["c"], "CCC")

	r, err = j.Parse(`{d: @d.json}`)
	if err != nil {
		t.Fatal(err)
	}
	m, _ = r.(map[string]any)
	assert(t, "json-ref", m["d"], map[string]any{"d": float64(3)})
}

func TestImplicitExt(t *testing.T) {
	files := map[string]string{
		"a.jsonic": `{a:1}`,
		"b.jsc":    `{b:2}`,
		"c.json":   `{"c":3}`,
	}

	j := MakeJsonic(MultiSourceOptions{
		Resolver: MakeMemResolver(files),
	})

	r, err := j.Parse(`{x: @a}`)
	if err != nil {
		t.Fatal(err)
	}
	m, _ := r.(map[string]any)
	assert(t, "implicit-jsonic", m["x"], map[string]any{"a": float64(1)})

	r, err = j.Parse(`{x: @b}`)
	if err != nil {
		t.Fatal(err)
	}
	m, _ = r.(map[string]any)
	assert(t, "implicit-jsc", m["x"], map[string]any{"b": float64(2)})

	r, err = j.Parse(`{x: @c}`)
	if err != nil {
		t.Fatal(err)
	}
	m, _ = r.(map[string]any)
	assert(t, "implicit-json", m["x"], map[string]any{"c": float64(3)})
}

func TestMultipleSources(t *testing.T) {
	files := map[string]string{
		"a.jsonic": `{a:1}`,
		"b.jsonic": `{b:2}`,
	}

	j := MakeJsonic(MultiSourceOptions{
		Resolver: MakeMemResolver(files),
	})

	r, err := j.Parse(`{x: @a.jsonic, y: @b.jsonic}`)
	if err != nil {
		t.Fatal(err)
	}
	m, _ := r.(map[string]any)
	assert(t, "multi-a", m["x"], map[string]any{"a": float64(1)})
	assert(t, "multi-b", m["y"], map[string]any{"b": float64(2)})
}

func TestNotFound(t *testing.T) {
	files := map[string]string{}

	j := MakeJsonic(MultiSourceOptions{
		Resolver: MakeMemResolver(files),
	})

	r, err := j.Parse(`{x: @missing}`)
	if err != nil {
		t.Fatal(err)
	}
	m, _ := r.(map[string]any)
	assert(t, "not-found", m["x"], nil)
}

func TestBasePath(t *testing.T) {
	files := map[string]string{
		"data/a.jsonic": `{a:1}`,
	}

	j := MakeJsonic(MultiSourceOptions{
		Resolver: MakeMemResolver(files),
		Path:     "data",
	})

	r, err := j.Parse(`{x: @a.jsonic}`)
	if err != nil {
		t.Fatal(err)
	}
	m, _ := r.(map[string]any)
	assert(t, "base-path", m["x"], map[string]any{"a": float64(1)})
}

func TestJSONSource(t *testing.T) {
	files := map[string]string{
		"config.json": `{"host":"localhost","port":8080}`,
	}

	j := MakeJsonic(MultiSourceOptions{
		Resolver: MakeMemResolver(files),
	})

	r, err := j.Parse(`{config: @config.json}`)
	if err != nil {
		t.Fatal(err)
	}
	m, _ := r.(map[string]any)
	cfg, _ := m["config"].(map[string]any)
	assert(t, "json-host", cfg["host"], "localhost")
	assert(t, "json-port", cfg["port"], float64(8080))
}

func TestIndexFile(t *testing.T) {
	files := map[string]string{
		"mymod/index.jsonic": `{x:1}`,
	}

	j := MakeJsonic(MultiSourceOptions{
		Resolver: MakeMemResolver(files),
	})

	r, err := j.Parse(`{mod: @mymod}`)
	if err != nil {
		t.Fatal(err)
	}
	m, _ := r.(map[string]any)
	assert(t, "index-file", m["mod"], map[string]any{"x": float64(1)})
}

func TestMixedValues(t *testing.T) {
	files := map[string]string{
		"a.jsonic": `{a:1}`,
	}

	j := MakeJsonic(MultiSourceOptions{
		Resolver: MakeMemResolver(files),
	})

	r, err := j.Parse(`{x: @a.jsonic, y: 2, z: "hello"}`)
	if err != nil {
		t.Fatal(err)
	}
	m, _ := r.(map[string]any)
	assert(t, "ref-val", m["x"], map[string]any{"a": float64(1)})
	assert(t, "num-val", m["y"], float64(2))
	assert(t, "str-val", m["z"], "hello")
}

func TestEmptyInput(t *testing.T) {
	j := MakeJsonic(MultiSourceOptions{
		Resolver: MakeMemResolver(map[string]string{}),
	})

	r, err := j.Parse(`{}`)
	if err != nil {
		t.Fatal(err)
	}
	assert(t, "empty", r, map[string]any{})
}

func TestResolvePathSpec(t *testing.T) {
	ps := ResolvePathSpec("a.jsonic", "base")
	assert(t, "full", ps.Full, "base/a.jsonic")
	assert(t, "kind", ps.Kind, "jsonic")
	assert(t, "abs", ps.Abs, false)

	ps = ResolvePathSpec("/abs/a.json", "base")
	assert(t, "abs-full", ps.Full, "/abs/a.json")
	assert(t, "abs-kind", ps.Kind, "json")
	assert(t, "abs-abs", ps.Abs, true)

	ps = ResolvePathSpec("noext", "")
	assert(t, "noext-full", ps.Full, "noext")
	assert(t, "noext-kind", ps.Kind, "")
}

func TestBuildPotentials(t *testing.T) {
	exts := []string{".jsonic", ".jsc", ".json"}

	p := buildPotentials("foo", exts)
	assert(t, "pot-0", p[0], "foo")
	assert(t, "pot-1", p[1], "foo.jsonic")
	assert(t, "pot-2", p[2], "foo.jsc")
	assert(t, "pot-3", p[3], "foo.json")
	assert(t, "pot-idx-1", p[4], "foo/index.jsonic")

	p = buildPotentials("bar.json", exts)
	assert(t, "has-ext", len(p), 1)
	assert(t, "has-ext-0", p[0], "bar.json")
}

func TestCustomProcessor(t *testing.T) {
	files := map[string]string{
		"data.csv": "a,b,c",
	}

	csvProc := func(res *Resolution, opts *MultiSourceOptions, j *jsonic.Jsonic) {
		parts := make([]any, 0)
		for _, s := range splitCSV(res.Src) {
			parts = append(parts, s)
		}
		res.Val = parts
	}

	procs := map[string]Processor{
		NONE:  DefaultProcessor,
		"csv": csvProc,
	}

	j := MakeJsonic(MultiSourceOptions{
		Resolver:  MakeMemResolver(files),
		Processor: procs,
	})

	r, err := j.Parse(`{data: @data.csv}`)
	if err != nil {
		t.Fatal(err)
	}
	m, _ := r.(map[string]any)
	assert(t, "csv", m["data"], []any{"a", "b", "c"})
}

// splitCSV is a simple CSV field splitter for testing.
func splitCSV(s string) []string {
	var result []string
	for _, field := range strings.Split(s, ",") {
		result = append(result, strings.TrimSpace(field))
	}
	return result
}

func TestParse(t *testing.T) {
	files := map[string]string{
		"a.jsonic": `{a:1}`,
	}

	r, err := Parse(`{x: @a.jsonic}`, MultiSourceOptions{
		Resolver: MakeMemResolver(files),
	})
	if err != nil {
		t.Fatal(err)
	}
	m, _ := r.(map[string]any)
	assert(t, "parse", m["x"], map[string]any{"a": float64(1)})
}

func TestAbsolutePath(t *testing.T) {
	files := map[string]string{
		"/etc/config.jsonic": `{env:"prod"}`,
	}

	j := MakeJsonic(MultiSourceOptions{
		Resolver: MakeMemResolver(files),
		Path:     "ignored",
	})

	r, err := j.Parse(`{cfg: @/etc/config.jsonic}`)
	if err != nil {
		t.Fatal(err)
	}
	m, _ := r.(map[string]any)
	assert(t, "abs-path", m["cfg"], map[string]any{"env": "prod"})
}

func TestPathPlugin(t *testing.T) {
	files := map[string]string{
		"a.jsonic": `{a:1}`,
		"b.jsonic": `{b:2}`,
	}

	j := MakeJsonic(MultiSourceOptions{
		Resolver: MakeMemResolver(files),
	})
	j.Use(path.Path, nil)

	r, err := j.Parse(`{x: @a.jsonic, y: @b.jsonic}`)
	if err != nil {
		t.Fatal(err)
	}
	m, _ := r.(map[string]any)
	assert(t, "path-a", m["x"], map[string]any{"a": float64(1)})
	assert(t, "path-b", m["y"], map[string]any{"b": float64(2)})
}

func TestMergeIntoMap(t *testing.T) {
	files := map[string]string{
		"a.jsonic": `{a:1}`,
	}

	j := MakeJsonic(MultiSourceOptions{
		Resolver: MakeMemResolver(files),
	})

	r, err := j.Parse(`{x:2, @a.jsonic}`)
	if err != nil {
		t.Fatal(err)
	}
	m, _ := r.(map[string]any)
	assert(t, "merge-x", m["x"], float64(2))
	assert(t, "merge-a", m["a"], float64(1))
}

func TestTopLevelRef(t *testing.T) {
	files := map[string]string{
		"a.jsonic": `{a:1}`,
	}

	j := MakeJsonic(MultiSourceOptions{
		Resolver: MakeMemResolver(files),
	})

	r, err := j.Parse(`@a.jsonic`)
	if err != nil {
		t.Fatal(err)
	}
	assert(t, "top-level", r, map[string]any{"a": float64(1)})
}

// TestDirectiveThenPair covers the README headline form `@"foo.jsonic" b:2`
// and the surrounding implicit-container cases — a top-level `@` directive
// followed by, preceded by, or interleaved with bare pairs. This is the case
// the new builtin pair-close (pairval, which reads r.Node[key] directly) made
// fragile: the implicit top-level map must be allocated so a following pair has
// a seeded node, and the {@foo} merge must deep-merge into the grandparent map
// in place so existing nested values survive and following pairs share the
// node. (Previously latently uncovered by the Go suite.)
func TestDirectiveThenPair(t *testing.T) {
	files := map[string]string{
		"a.jsonic": `a:1`,
		"b.jsonic": `a:{b:1,c:2}`,
		"d.jsonic": `d:3`,
	}
	j := MakeJsonic(MultiSourceOptions{
		Resolver: MakeMemResolver(files),
	})

	cases := []struct {
		name string
		src  string
		want map[string]any
	}{
		// README headline: directive first, then a pair.
		{"directive-then-pair", `@a.jsonic b:2`,
			map[string]any{"a": float64(1), "b": float64(2)}},
		{"pair-then-directive", `b:2 @a.jsonic`,
			map[string]any{"a": float64(1), "b": float64(2)}},
		{"pair-directive-pair", `b:2 @a.jsonic c:3`,
			map[string]any{"a": float64(1), "b": float64(2), "c": float64(3)}},
		{"directive-only", `@a.jsonic`,
			map[string]any{"a": float64(1)}},

		// Two directives, bare and interleaved with pairs.
		{"two-directives", `@a.jsonic @d.jsonic`,
			map[string]any{"a": float64(1), "d": float64(3)}},
		{"directive-pair-directive", `@a.jsonic x:11 @d.jsonic`,
			map[string]any{"a": float64(1), "x": float64(11), "d": float64(3)}},

		// Deep-merge into the grandparent map: the existing `d:3` must
		// survive (in-place deep merge), not be overwritten.
		{"merge-keep-existing", `a:{d:3} @b.jsonic`,
			map[string]any{"a": map[string]any{"b": float64(1), "c": float64(2), "d": float64(3)}}},
		{"merge-then-override", `a:{d:3} @b.jsonic a:{d:4,f:5}`,
			map[string]any{"a": map[string]any{"b": float64(1), "c": float64(2), "d": float64(4), "f": float64(5)}}},
		{"directive-then-override", `@b.jsonic a:{d:4,f:5}`,
			map[string]any{"a": map[string]any{"b": float64(1), "c": float64(2), "d": float64(4), "f": float64(5)}}},
		{"merge-then-pair", `@b.jsonic y:2`,
			map[string]any{"a": map[string]any{"b": float64(1), "c": float64(2)}, "y": float64(2)}},
	}

	for _, tc := range cases {
		r, err := j.Parse(tc.src)
		if err != nil {
			t.Fatalf("%s (%q): %v", tc.name, tc.src, err)
		}
		assert(t, tc.name, r, tc.want)
	}
}
