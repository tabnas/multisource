// Copyright (c) 2025 Richard Rodger and other contributors, MIT License

package tabnasmultisource

// parity_test.go — cross-runtime conformance, driven by the shared
// `test/spec/*.tsv` fixtures at the repo root (see ../test/AGENTS.md).
//
// The fixture loader, the escape codec, the ERROR:<code> contract and the
// row loop all come from github.com/tabnas/support/go, whose TypeScript
// half ts/test/parity.test.ts uses to run the SAME files — so the two
// implementations cannot drift without one of them going red, and neither
// can the two loaders.
//
// What is left here is only what is specific to multisource: how to build
// the parser for a row's options, and how to flatten a result for
// comparison.

import (
	"encoding/json"
	"fmt"
	"testing"

	jsonic "github.com/tabnas/jsonic/go"
	support "github.com/tabnas/support/go"
)

// TestSpec runs every fixture in the spec directory. FindSpecDir walks up
// from the package directory, and Dir discovers the files by listing, so
// adding a .tsv runs it in both runtimes without touching either runner.
func TestSpec(t *testing.T) {
	dir, err := support.FindSpecDir("")
	if err != nil {
		t.Fatal(err)
	}

	support.Runner{
		ParseRow: func(input string, row *support.Row) (any, error) {
			opts := map[string]any{}
			if raw := row.Named("opts"); "" != raw {
				if err := json.Unmarshal([]byte(raw), &opts); err != nil {
					return nil, err
				}
			}

			// The opts column carries the in-memory source set the case
			// resolves against (`mem`), plus optional engine options
			// (`options`). A real filesystem cannot live in a fixture, so
			// the file/pkg resolvers stay covered by the in-language tests.
			mem := map[string]string{}
			if raw, ok := opts["mem"].(map[string]any); ok {
				for k, v := range raw {
					if s, ok := v.(string); ok {
						mem[k] = s
					}
				}
			}

			j := jsonic.Make()
			if err := j.Use(MultiSource, map[string]any{
				"resolver": MakeMemResolver(mem),
			}); err != nil {
				return nil, err
			}
			if engineOpts, ok := opts["options"].(map[string]any); ok {
				if err := applySpecOptions(j, engineOpts); err != nil {
					return nil, err
				}
			}

			return j.Parse(input)
		},

		// Flatten through JSON so the parser's own containers and numeric
		// types compare against the fixture's decoded shape. Normalize runs
		// outermost first, so the top node is enough — the plain values it
		// yields pass through unchanged.
		Normalize: jsonFlatten,
	}.Dir(t, dir)
}

// applySpecOptions applies the small set of engine options a fixture may
// carry. Kept explicit rather than generic: only options a case actually
// needs belong here, and each must have a TS counterpart.
func applySpecOptions(j *jsonic.Jsonic, opts map[string]any) error {
	if m, ok := opts["map"].(map[string]any); ok {
		if extend, ok := m["extend"].(bool); ok {
			j.SetOptions(jsonic.Options{Map: &jsonic.MapOptions{Extend: &extend}})
			return nil
		}
	}
	return fmt.Errorf("spec options not supported by the runner: %v", opts)
}

// jsonFlatten renders a value as JSON and reads it back as plain
// map/slice/float64/string/bool/nil. A value that will not marshal is
// returned as it is: the comparison then fails and prints it, which says
// more than a panic here would.
func jsonFlatten(v any) any {
	raw, err := json.Marshal(v)
	if err != nil {
		return v
	}
	var out any
	if err := json.Unmarshal(raw, &out); err != nil {
		return v
	}
	return out
}
