/* Copyright (c) 2025 Richard Rodger, MIT License */

package tabnasmultisource

import (
	"testing"
	"testing/fstest"
)

// TestColonChainImport guards the colon-chain (path-dive) import case: a bare
// `@"file"` value reached through a colon-chain key (`a: b: @"f"`) must resolve
// nested under the key, matching the canonical TypeScript multisource plugin.
//
// The regression it covers: the Go port shared a single `pk > 0` back-track
// condition for the val-open alt as well as the map/pair close alts. In a
// colon-chain the value-position val rule has pk > 0 but its parent is the pair
// for that key, so the val-open back-track must additionally require the parent
// not to be a pair. Without that clause the @ unwinds to depth 0, the key is
// finalised to null, and the import is silently dropped.
func TestColonChainImport(t *testing.T) {
	files := map[string]string{"minor": `{x:1}`}
	j := MakeJsonic(MultiSourceOptions{Resolver: MakeMemResolver(files)})

	cases := []struct {
		src  string
		want any
	}{
		// direct and braced nesting already worked; included as parity anchors.
		{`struct: @minor`, map[string]any{"struct": "{x:1}"}},
		{`struct: {minor: @minor}`, map[string]any{
			"struct": map[string]any{"minor": "{x:1}"}}},
		// colon-chain nesting: the cases that regressed.
		{`struct: minor: @minor`, map[string]any{
			"struct": map[string]any{"minor": "{x:1}"}}},
		{`a: b: c: @minor`, map[string]any{
			"a": map[string]any{"b": map[string]any{"c": "{x:1}"}}}},
	}

	for _, c := range cases {
		got, err := j.Parse(c.src)
		if err != nil {
			t.Fatalf("%q: %v", c.src, err)
		}
		assert(t, c.src, got, c.want)
	}
}

// TestColonChainImportFile is the file-resolver counterpart, mirroring the
// reproduction in the design note (jsonic-processed import via fs.FS).
func TestColonChainImportFile(t *testing.T) {
	fsys := fstest.MapFS{"minor.aon": {Data: []byte(`{x:1}`)}}
	j := MakeJsonic(MultiSourceOptions{Resolver: MakeFileResolver(), FS: fsys})

	got, err := j.Parse(`struct: minor: @"minor.aon"`)
	if err != nil {
		t.Fatal(err)
	}
	assert(t, "colon-chain-file", got, map[string]any{
		"struct": map[string]any{"minor": "{x:1}"},
	})
}
