package tabnasmultisource

import (
	"strings"
	"testing"

	jsonic "github.com/tabnas/jsonic/go"
)

// A source cannot be an ancestor of itself. Without this check a -> b -> a
// recursed until the stack overflowed, with no source position and no
// indication of which files were at fault. Mirrors the TS cycle test.
func mkCycleParser(t *testing.T, files map[string]string) *jsonic.Jsonic {
	t.Helper()
	j := jsonic.Make(jsonic.Options{})
	if err := j.Use(MultiSource, map[string]any{
		"resolver": MakeMemResolver(files),
	}); err != nil {
		t.Fatalf("Use: %v", err)
	}
	return j
}

func TestCycleDetected(t *testing.T) {
	cases := []struct {
		name  string
		files map[string]string
	}{
		{"direct", map[string]string{"a.jsonic": `@"a.jsonic"`}},
		{"two-step", map[string]string{
			"a.jsonic": `@"b.jsonic"`, "b.jsonic": `@"a.jsonic"`}},
		{"three-step", map[string]string{
			"a.jsonic": `@"b.jsonic"`,
			"b.jsonic": `@"c.jsonic"`,
			"c.jsonic": `@"a.jsonic"`}},
	}
	for _, tc := range cases {
		t.Run(tc.name, func(t *testing.T) {
			j := mkCycleParser(t, tc.files)
			_, err := j.Parse(`@"a.jsonic"`)
			if err == nil {
				t.Fatal("expected a cycle error, got none")
			}
			if !strings.Contains(err.Error(), "multisource_cycle") &&
				!strings.Contains(err.Error(), "includes itself") {
				t.Errorf("unexpected error: %v", err)
			}
		})
	}
}

// Reuse is not a cycle: a file included from two branches, or twice from
// one, is fine — the check is against the ancestor chain, not against
// everything already visited.
func TestReuseIsNotACycle(t *testing.T) {
	j := mkCycleParser(t, map[string]string{
		"base.jsonic": "x:1",
		"l.jsonic":    `@"base.jsonic"`,
		"r.jsonic":    `@"base.jsonic"`,
	})
	if _, err := j.Parse(`l: @"l.jsonic", r: @"r.jsonic"`); err != nil {
		t.Fatalf("diamond should parse: %v", err)
	}

	j2 := mkCycleParser(t, map[string]string{"base.jsonic": "x:1"})
	if _, err := j2.Parse(`a: @"base.jsonic", b: @"base.jsonic"`); err != nil {
		t.Fatalf("repeated include should parse: %v", err)
	}

	j3 := mkCycleParser(t, map[string]string{
		"a.jsonic": `@"b.jsonic"`,
		"b.jsonic": `@"c.jsonic"`,
		"c.jsonic": "z:1",
	})
	if _, err := j3.Parse(`@"a.jsonic"`); err != nil {
		t.Fatalf("acyclic chain should parse: %v", err)
	}
}
