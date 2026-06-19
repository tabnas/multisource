/* Copyright (c) 2025 Richard Rodger, MIT License */

package tabnasmultisource

import (
	"testing"
	"testing/fstest"
)

// mapFS builds an in-memory fs.FS (testing/fstest.MapFS) from a path -> content
// map. It is the Go counterpart to the memfs used by the TypeScript tests.
func mapFS(files map[string]string) fstest.MapFS {
	m := make(fstest.MapFS, len(files))
	for k, v := range files {
		m[k] = &fstest.MapFile{Data: []byte(v)}
	}
	return m
}

// TestFileResolverFS checks that the file resolver reads from an injected
// io/fs.FS (via MultiSourceOptions.FS) instead of the OS, covering explicit
// extensions, implicit extensions, index files, JSON, and a sub-directory base.
func TestFileResolverFS(t *testing.T) {
	fsys := mapFS(map[string]string{
		"a.jsonic":         `{a:1}`,
		"b.jsonic":         `{b:2}`,
		"mod/index.jsonic": `{m:3}`,
		"data/cfg.json":    `{"k":4}`,
	})

	j := MakeJsonic(MultiSourceOptions{Resolver: MakeFileResolver(), FS: fsys})

	cases := []struct {
		src  string
		want any
	}{
		{`{x:@a.jsonic}`, map[string]any{"a": float64(1)}},        // explicit ext
		{`{x:@b}`, map[string]any{"b": float64(2)}},               // implicit ext
		{`{x:@mod}`, map[string]any{"m": float64(3)}},             // index file
		{`{x:@"data/cfg.json"}`, map[string]any{"k": float64(4)}}, // json, sub-dir
	}
	for _, c := range cases {
		r, err := j.Parse(c.src)
		if err != nil {
			t.Fatalf("%s: %v", c.src, err)
		}
		m, _ := r.(map[string]any)
		assert(t, c.src, m["x"], c.want)
	}
}

// TestFileResolverFSViaMeta checks the per-parse filesystem override passed as
// ctx.Meta["fs"], mirroring the TypeScript j('...', { fs }). The fs must also
// propagate to nested loads (threaded through the copied parse meta).
func TestFileResolverFSViaMeta(t *testing.T) {
	fsys := mapFS(map[string]string{
		"main.jsonic":  `{child:@"./sub/c.jsonic"}`,
		"sub/c.jsonic": `{v:7}`,
	})

	// No instance-level FS: the filesystem comes from the parse meta only.
	j := MakeJsonic(MultiSourceOptions{Resolver: MakeFileResolver()})

	r, err := j.ParseMeta(`@"./main.jsonic"`, map[string]any{"fs": fsys})
	if err != nil {
		t.Fatal(err)
	}
	assert(t, "fs-via-meta", r, map[string]any{
		"child": map[string]any{"v": float64(7)},
	})
}

// TestPkgResolverFS checks that the pkg resolver reads from an injected
// io/fs.FS, covering a sub-path reference, an index file, and package.json
// "main".
func TestPkgResolverFS(t *testing.T) {
	fsys := mapFS(map[string]string{
		"node_modules/mypkg/zed.jsonic":     `{zed:99}`,
		"node_modules/idxpkg/index.jsonic":  `{i:5}`,
		"node_modules/mainpkg/package.json": `{"main":"main.jsonic"}`,
		"node_modules/mainpkg/main.jsonic":  `{z:11}`,
	})

	j := MakeJsonic(MultiSourceOptions{
		Resolver: MakePkgResolver(PkgResolverOptions{Paths: []string{"."}}),
		FS:       fsys,
	})

	cases := []struct {
		src  string
		want any
	}{
		{`{c:@"mypkg/zed.jsonic"}`, map[string]any{"zed": float64(99)}}, // sub-path
		{`{c:@"idxpkg"}`, map[string]any{"i": float64(5)}},              // index
		{`{c:@"mainpkg"}`, map[string]any{"z": float64(11)}},            // "main"
	}
	for _, c := range cases {
		r, err := j.Parse(c.src)
		if err != nil {
			t.Fatalf("%s: %v", c.src, err)
		}
		m, _ := r.(map[string]any)
		assert(t, c.src, m["c"], c.want)
	}
}

// TestPkgResolverFSWalkUp checks that, with an injected filesystem, the pkg
// resolver still walks up parent directories to find node_modules.
func TestPkgResolverFSWalkUp(t *testing.T) {
	fsys := mapFS(map[string]string{
		"node_modules/mypkg/zed.jsonic": `{zed:99}`,
		// The reference is resolved from a nested starting directory.
		"a/b/c/.keep": ``,
	})

	j := MakeJsonic(MultiSourceOptions{
		Resolver: MakePkgResolver(PkgResolverOptions{Paths: []string{"a/b/c"}}),
		FS:       fsys,
	})

	r, err := j.Parse(`{c:@"mypkg/zed.jsonic"}`)
	if err != nil {
		t.Fatal(err)
	}
	m, _ := r.(map[string]any)
	assert(t, "pkg-fs-walkup", m["c"], map[string]any{"zed": float64(99)})
}

// TestPkgResolverRelativeInPkg checks that a relative reference (./x, ../x)
// found *inside* a source loaded from a package resolves against that source's
// own directory rather than being treated as a node_modules package name.
// Covers an explicit extension, an implicit extension, and a sub-directory.
func TestPkgResolverRelativeInPkg(t *testing.T) {
	fsys := mapFS(map[string]string{
		"node_modules/relpkg/index.jsonic":    `{a:1, b:@"./child.jsonic", c:@"./leaf", d:@"./sub/deep.jsonic"}`,
		"node_modules/relpkg/child.jsonic":    `{x:10}`,
		"node_modules/relpkg/leaf.jsonic":     `{y:20}`,
		"node_modules/relpkg/sub/deep.jsonic": `{z:30}`,
	})

	j := MakeJsonic(MultiSourceOptions{
		Resolver: MakePkgResolver(PkgResolverOptions{Paths: []string{"."}}),
		FS:       fsys,
	})

	r, err := j.Parse(`{r:@"relpkg"}`)
	if err != nil {
		t.Fatal(err)
	}
	m, _ := r.(map[string]any)
	assert(t, "pkg-relative-internal", m["r"], map[string]any{
		"a": float64(1),
		"b": map[string]any{"x": float64(10)},
		"c": map[string]any{"y": float64(20)},
		"d": map[string]any{"z": float64(30)},
	})
}
