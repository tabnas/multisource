/* Copyright (c) 2025 Richard Rodger, MIT License */

package tabnasmultisource

import (
	"os"
	"path/filepath"
	"strings"
	"testing"

	jsonic "github.com/tabnas/jsonic/go"
)

// TestNestedRelativeLoad checks that a relative reference *inside* a loaded
// file resolves against that file's own directory (a -> b -> c, across
// directories), mirroring the canonical TypeScript behaviour. Uses an in-memory
// fs.FS so the test is hermetic (cf. the memfs-based TS test).
func TestNestedRelativeLoad(t *testing.T) {
	fsys := mapFS(map[string]string{
		"main.jsonic":      `{top:1, child:@"./sub/child.jsonic"}`,
		"sub/child.jsonic": `{mid:2, grand:@"./grand.jsonic"}`,
		"sub/grand.jsonic": `{v:99}`,
	})

	j := MakeJsonic(MultiSourceOptions{Resolver: MakeFileResolver(), FS: fsys})
	r, err := j.Parse(`@"./main.jsonic"`)
	if err != nil {
		t.Fatal(err)
	}
	assert(t, "nested-relative", r, map[string]any{
		"top": float64(1),
		"child": map[string]any{
			"mid": float64(2),
			"grand": map[string]any{
				"v": float64(99),
			},
		},
	})
}

// TestNestedRelativeSiblingDirs checks that two references loaded from the same
// parent each resolve their *own* relative references against their own
// directory. Both children load "./inner.jsonic", but from different
// directories, so they must pick up different files. This proves the base path
// is tracked per-source and that resolving one reference does not leak into a
// sibling (the parent context is copied, not mutated).
func TestNestedRelativeSiblingDirs(t *testing.T) {
	fsys := mapFS(map[string]string{
		"main.jsonic":     `{a:@"./aa/a.jsonic", b:@"./bb/b.jsonic"}`,
		"aa/a.jsonic":     `{x:@"./inner.jsonic"}`,
		"aa/inner.jsonic": `{n:11}`,
		"bb/b.jsonic":     `{y:@"./inner.jsonic"}`,
		"bb/inner.jsonic": `{n:22}`,
	})

	j := MakeJsonic(MultiSourceOptions{Resolver: MakeFileResolver(), FS: fsys})
	r, err := j.Parse(`@"./main.jsonic"`)
	if err != nil {
		t.Fatal(err)
	}
	assert(t, "sibling-dirs", r, map[string]any{
		"a": map[string]any{"x": map[string]any{"n": float64(11)}},
		"b": map[string]any{"y": map[string]any{"n": float64(22)}},
	})
}

// TestNestedRelativeMemFlat checks that nested references through flat (no
// directory) in-memory keys keep resolving as bare keys (a -> b -> c). Mirrors
// the TypeScript "deps" test. This guards the in-memory resolver against a
// directory-stripping regression: a parent key like "a.jsc" must yield an
// empty base (not "."), so a bare nested reference like "@b.jsc" still matches.
func TestNestedRelativeMemFlat(t *testing.T) {
	files := map[string]string{
		"a.jsc":       `a:1,b:@b.jsc,x:99`,
		"b.jsc":       `b:2,c:@c`,
		"c/index.jsc": `c:3`,
	}

	j := MakeJsonic(MultiSourceOptions{Resolver: MakeMemResolver(files)})
	r, err := j.Parse(`@a`)
	if err != nil {
		t.Fatal(err)
	}
	assert(t, "mem-flat-nested", r, map[string]any{
		"a": float64(1),
		"b": map[string]any{
			"b": float64(2),
			"c": map[string]any{"c": float64(3)},
		},
		"x": float64(99),
	})
}

// TestNestedSourcePathMeta checks that the full path of the source being
// processed, and the chain of enclosing parents, are threaded through ctx.Meta
// (under the "multisource" entry), mirroring the canonical TypeScript
// ctx.meta.multisource.{path,parents}. A custom processor inspects the meta it
// receives.
func TestNestedSourcePathMeta(t *testing.T) {
	fsys := mapFS(map[string]string{
		"main.jsonic": `{child:@"./sub/c.probe"}`,
		"sub/c.probe": `probe-content`,
	})

	var gotPath string
	var gotParents []string
	probe := func(res *Resolution, _ *MultiSourceOptions, ctx *jsonic.Context, _ *jsonic.Jsonic) {
		if ms, ok := ctx.Meta["multisource"].(map[string]any); ok {
			gotPath, _ = ms["path"].(string)
			gotParents, _ = ms["parents"].([]string)
		}
		res.Val = res.Src
	}

	j := MakeJsonic(MultiSourceOptions{
		Resolver: MakeFileResolver(),
		FS:       fsys,
		Processor: map[string]Processor{
			NONE:     DefaultProcessor,
			"jsonic": JsonicProcessor,
			"probe":  probe,
		},
	})
	if _, err := j.Parse(`@"./main.jsonic"`); err != nil {
		t.Fatal(err)
	}

	if !strings.HasSuffix(filepath.ToSlash(gotPath), "sub/c.probe") {
		t.Fatalf("threaded path: want suffix sub/c.probe, got %q", gotPath)
	}
	if len(gotParents) != 1 || !strings.HasSuffix(filepath.ToSlash(gotParents[0]), "main.jsonic") {
		t.Fatalf("threaded parents: want [.../main.jsonic], got %#v", gotParents)
	}
}

// TestNestedRelativeLoadOSFiles is the on-disk counterpart of
// TestNestedRelativeLoad: it exercises nested relative resolution against real
// OS files (absolute paths from filepath.Abs), confirming the directory
// tracking works for the default OS filesystem as well as an injected fs.FS.
func TestNestedRelativeLoadOSFiles(t *testing.T) {
	dir := t.TempDir()
	sub := filepath.Join(dir, "sub")
	if err := os.MkdirAll(sub, 0o755); err != nil {
		t.Fatal(err)
	}
	write := func(p, s string) {
		if err := os.WriteFile(p, []byte(s), 0o644); err != nil {
			t.Fatal(err)
		}
	}
	write(filepath.Join(dir, "main.jsonic"), `{top:1, child:@"./sub/child.jsonic"}`)
	write(filepath.Join(sub, "child.jsonic"), `{mid:2, grand:@"./grand.jsonic"}`)
	write(filepath.Join(sub, "grand.jsonic"), `{v:99}`)

	j := MakeJsonic(MultiSourceOptions{Resolver: MakeFileResolver(), Path: dir})
	r, err := j.Parse(`@"./main.jsonic"`)
	if err != nil {
		t.Fatal(err)
	}
	assert(t, "nested-relative-os", r, map[string]any{
		"top": float64(1),
		"child": map[string]any{
			"mid": float64(2),
			"grand": map[string]any{
				"v": float64(99),
			},
		},
	})
}
