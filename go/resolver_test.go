/* Copyright (c) 2025 Richard Rodger, MIT License */

package tabnasmultisource

import (
	"os"
	"path/filepath"
	"testing"
)

// writeTestFile writes content to p, creating parent directories.
func writeTestFile(t *testing.T, p, content string) {
	t.Helper()
	if err := os.MkdirAll(filepath.Dir(p), 0o755); err != nil {
		t.Fatal(err)
	}
	if err := os.WriteFile(p, []byte(content), 0o644); err != nil {
		t.Fatal(err)
	}
}

func TestFileResolver(t *testing.T) {
	dir := t.TempDir()
	writeTestFile(t, filepath.Join(dir, "a.jsonic"), `{a:1}`)

	j := MakeJsonic(MultiSourceOptions{
		Resolver: MakeFileResolver(),
		Path:     dir,
	})

	r, err := j.Parse(`{x: @a.jsonic}`)
	if err != nil {
		t.Fatal(err)
	}
	m, _ := r.(map[string]any)
	assert(t, "file-ref", m["x"], map[string]any{"a": float64(1)})
}

func TestFileResolverImplicitExt(t *testing.T) {
	dir := t.TempDir()
	writeTestFile(t, filepath.Join(dir, "b.jsonic"), `{b:2}`)

	j := MakeJsonic(MultiSourceOptions{
		Resolver: MakeFileResolver(),
		Path:     dir,
	})

	r, err := j.Parse(`{x: @b}`)
	if err != nil {
		t.Fatal(err)
	}
	m, _ := r.(map[string]any)
	assert(t, "file-implicit", m["x"], map[string]any{"b": float64(2)})
}

func TestFileResolverIndex(t *testing.T) {
	dir := t.TempDir()
	writeTestFile(t, filepath.Join(dir, "mod", "index.jsonic"), `{m:3}`)

	j := MakeJsonic(MultiSourceOptions{
		Resolver: MakeFileResolver(),
		Path:     dir,
	})

	r, err := j.Parse(`{x: @mod}`)
	if err != nil {
		t.Fatal(err)
	}
	m, _ := r.(map[string]any)
	assert(t, "file-index", m["x"], map[string]any{"m": float64(3)})
}

func TestFileResolverFolderIndex(t *testing.T) {
	dir := t.TempDir()
	writeTestFile(t, filepath.Join(dir, "h", "index.h.jsonic"), `{h:7}`)

	j := MakeJsonic(MultiSourceOptions{
		Resolver: MakeFileResolver(),
		Path:     dir,
	})

	r, err := j.Parse(`{x: @h}`)
	if err != nil {
		t.Fatal(err)
	}
	m, _ := r.(map[string]any)
	assert(t, "file-folder-index", m["x"], map[string]any{"h": float64(7)})
}

func TestFileResolverPreload(t *testing.T) {
	dir := t.TempDir()
	// No file on disk; provide content via preload keyed by absolute path.
	abs, _ := filepath.Abs(filepath.Join(dir, "p.jsonic"))

	j := MakeJsonic(MultiSourceOptions{
		Resolver: MakeFileResolver(FileResolverOptions{
			Preload: map[string]string{abs: `{p:4}`},
		}),
		Path: dir,
	})

	r, err := j.Parse(`{x: @p.jsonic}`)
	if err != nil {
		t.Fatal(err)
	}
	m, _ := r.(map[string]any)
	assert(t, "file-preload", m["x"], map[string]any{"p": float64(4)})
}

func TestFileResolverPathFinder(t *testing.T) {
	dir := t.TempDir()
	writeTestFile(t, filepath.Join(dir, "sub", "a.jsonic"), `{a:1}`)

	j := MakeJsonic(MultiSourceOptions{
		Resolver: MakeFileResolver(FileResolverOptions{
			PathFinder: func(spec string) string { return "sub/" + spec },
		}),
		Path: dir,
	})

	r, err := j.Parse(`{x: @a.jsonic}`)
	if err != nil {
		t.Fatal(err)
	}
	m, _ := r.(map[string]any)
	assert(t, "file-pathfinder", m["x"], map[string]any{"a": float64(1)})
}

func TestFileResolverNotFound(t *testing.T) {
	dir := t.TempDir()
	j := MakeJsonic(MultiSourceOptions{
		Resolver: MakeFileResolver(),
		Path:     dir,
	})

	r, err := j.Parse(`{x: @missing.jsonic}`)
	if err != nil {
		t.Fatal(err)
	}
	m, _ := r.(map[string]any)
	assert(t, "file-not-found", m["x"], nil)
}

func TestPkgResolver(t *testing.T) {
	dir := t.TempDir()
	writeTestFile(t, filepath.Join(dir, "node_modules", "mypkg", "zed.jsonic"), `{zed:99}`)

	j := MakeJsonic(MultiSourceOptions{
		Resolver: MakePkgResolver(PkgResolverOptions{Paths: []string{dir}}),
	})

	r, err := j.Parse(`{c: @"mypkg/zed.jsonic"}`)
	if err != nil {
		t.Fatal(err)
	}
	m, _ := r.(map[string]any)
	assert(t, "pkg-subpath", m["c"], map[string]any{"zed": float64(99)})
}

func TestPkgResolverMain(t *testing.T) {
	dir := t.TempDir()
	writeTestFile(t, filepath.Join(dir, "node_modules", "mypkg", "package.json"), `{"main":"main.jsonic"}`)
	writeTestFile(t, filepath.Join(dir, "node_modules", "mypkg", "main.jsonic"), `{z:11}`)

	j := MakeJsonic(MultiSourceOptions{
		Resolver: MakePkgResolver(PkgResolverOptions{Paths: []string{dir}}),
	})

	r, err := j.Parse(`{z: @"mypkg"}`)
	if err != nil {
		t.Fatal(err)
	}
	m, _ := r.(map[string]any)
	assert(t, "pkg-main", m["z"], map[string]any{"z": float64(11)})
}

func TestPkgResolverIndex(t *testing.T) {
	dir := t.TempDir()
	writeTestFile(t, filepath.Join(dir, "node_modules", "idxpkg", "index.jsonic"), `{i:5}`)

	j := MakeJsonic(MultiSourceOptions{
		Resolver: MakePkgResolver(PkgResolverOptions{Paths: []string{dir}}),
	})

	r, err := j.Parse(`{i: @"idxpkg"}`)
	if err != nil {
		t.Fatal(err)
	}
	m, _ := r.(map[string]any)
	assert(t, "pkg-index", m["i"], map[string]any{"i": float64(5)})
}

func TestPkgResolverWalkUp(t *testing.T) {
	// Package installed in an ancestor's node_modules; resolve from a nested dir.
	root := t.TempDir()
	writeTestFile(t, filepath.Join(root, "node_modules", "mypkg", "zed.jsonic"), `{zed:99}`)
	nested := filepath.Join(root, "a", "b", "c")
	if err := os.MkdirAll(nested, 0o755); err != nil {
		t.Fatal(err)
	}

	j := MakeJsonic(MultiSourceOptions{
		Resolver: MakePkgResolver(PkgResolverOptions{Paths: []string{nested}}),
	})

	r, err := j.Parse(`{c: @"mypkg/zed.jsonic"}`)
	if err != nil {
		t.Fatal(err)
	}
	m, _ := r.(map[string]any)
	assert(t, "pkg-walkup", m["c"], map[string]any{"zed": float64(99)})
}

func TestPkgResolverNotFound(t *testing.T) {
	dir := t.TempDir()
	j := MakeJsonic(MultiSourceOptions{
		Resolver: MakePkgResolver(PkgResolverOptions{Paths: []string{dir}}),
	})

	r, err := j.Parse(`{x: @"nopkg/zed.jsonic"}`)
	if err != nil {
		t.Fatal(err)
	}
	m, _ := r.(map[string]any)
	assert(t, "pkg-not-found", m["x"], nil)
}
