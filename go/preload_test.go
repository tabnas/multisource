/* Copyright (c) 2025 Richard Rodger, MIT License */

package tabnasmultisource

import (
	"os"
	"path/filepath"
	"strings"
	"testing"
)

// hasKeySuffix reports whether any key in m ends with suffix.
func hasKeySuffix(m map[string]string, suffix string) bool {
	for k := range m {
		if strings.HasSuffix(k, suffix) {
			return true
		}
	}
	return false
}

// hasKeyContaining reports whether any key in m contains part.
func hasKeyContaining(m map[string]string, part string) bool {
	for k := range m {
		if strings.Contains(k, part) {
			return true
		}
	}
	return false
}

// preloadFixtureDir builds a folder shaped like the TypeScript test/ fixture
// dir: mixed extensions at the root, plus an f01/ subfolder.
func preloadFixtureDir(t *testing.T) string {
	t.Helper()
	dir := t.TempDir()
	writeTestFile(t, filepath.Join(dir, "t01.jsonic"), `{c:2}`)
	writeTestFile(t, filepath.Join(dir, "k03.json"), `{"g":4}`)
	writeTestFile(t, filepath.Join(dir, "k02.js"), `module.exports={e:3}`)
	writeTestFile(t, filepath.Join(dir, "f01", "f01t01.jsonic"), `{f:1}`)
	return dir
}

// TestPreloadExtensions ports the TypeScript 'preload-extensions' test:
// default extensions are .jsonic and .json; custom extensions replace them
// (and a missing leading dot is added).
func TestPreloadExtensions(t *testing.T) {
	dir := preloadFixtureDir(t)

	// Default extensions: .jsonic, .json
	defaultMap := PreloadFiles(PreloadOptions{Folders: []string{dir}})
	if !hasKeySuffix(defaultMap, ".jsonic") {
		t.Fatalf("default ext: expected a .jsonic key: %#v", defaultMap)
	}
	if !hasKeySuffix(defaultMap, ".json") {
		t.Fatalf("default ext: expected a .json key: %#v", defaultMap)
	}
	if hasKeySuffix(defaultMap, ".js") {
		t.Fatalf("default ext: unexpected .js key: %#v", defaultMap)
	}

	// Custom extensions.
	jsMap := PreloadFiles(PreloadOptions{Folders: []string{dir}, Ext: []string{".js"}})
	if !hasKeySuffix(jsMap, ".js") {
		t.Fatalf("custom ext: expected a .js key: %#v", jsMap)
	}
	if hasKeySuffix(jsMap, ".jsonic") {
		t.Fatalf("custom ext: unexpected .jsonic key: %#v", jsMap)
	}

	// Extensions are normalised to a leading dot ("js" == ".js").
	jsMap2 := PreloadFiles(PreloadOptions{Folders: []string{dir}, Ext: []string{"js"}})
	assert(t, "preload-ext-normalise", jsMap2, jsMap)
}

// TestPreloadRecursive ports the TypeScript 'preload-recursive' test:
// non-recursive scans stay at the folder root; recursive scans descend into
// subfolders.
func TestPreloadRecursive(t *testing.T) {
	dir := preloadFixtureDir(t)

	// Non-recursive (default): should not find files in f01/.
	flatMap := PreloadFiles(PreloadOptions{Folders: []string{dir}, Ext: []string{".jsonic"}})
	if hasKeyContaining(flatMap, "f01") {
		t.Fatalf("non-recursive should not descend into f01/: %#v", flatMap)
	}
	if len(flatMap) == 0 {
		t.Fatal("non-recursive scan should find root .jsonic files")
	}

	// Recursive: should find files in f01/.
	deepMap := PreloadFiles(PreloadOptions{
		Folders:   []string{dir},
		Ext:       []string{".jsonic"},
		Recursive: true,
	})
	if !hasKeyContaining(deepMap, "f01") {
		t.Fatalf("recursive should find files in f01/: %#v", deepMap)
	}
}

// TestPreloadMultipleFolders ports the TypeScript 'preload-multiple-folders'
// test: scanning several folders combines their files into one map.
func TestPreloadMultipleFolders(t *testing.T) {
	dir := preloadFixtureDir(t)
	f01Dir := filepath.Join(dir, "f01")

	rootOnly := PreloadFiles(PreloadOptions{Folders: []string{dir}, Ext: []string{".jsonic"}})
	f01Only := PreloadFiles(PreloadOptions{Folders: []string{f01Dir}, Ext: []string{".jsonic"}})

	if len(rootOnly) == 0 {
		t.Fatal("should have files from the root folder")
	}
	if len(f01Only) == 0 {
		t.Fatal("should have files from f01/")
	}

	combined := PreloadFiles(PreloadOptions{
		Folders: []string{dir, f01Dir},
		Ext:     []string{".jsonic"},
	})
	if len(combined) < len(rootOnly) || len(combined) < len(f01Only) {
		t.Fatalf("combined scan should include both folders: %#v", combined)
	}
	for k, v := range rootOnly {
		assert(t, "combined-root:"+k, combined[k], v)
	}
	for k, v := range f01Only {
		assert(t, "combined-f01:"+k, combined[k], v)
	}
}

// TestPreloadMissingFolder ports the TypeScript 'preload-missing-folder' test:
// non-existent folders are skipped without error.
func TestPreloadMissingFolder(t *testing.T) {
	filemap := PreloadFiles(PreloadOptions{
		Folders: []string{"/nonexistent/folder/path"},
	})
	assert(t, "preload-missing", filemap, map[string]string{})
}

// TestPreloadFileResolver ports the TypeScript 'preload-file-resolver' test:
// the PreloadFiles output feeds FileResolverOptions.Preload so references
// resolve from memory. The folder is removed after preloading to prove no disk
// I/O happens during the parse.
func TestPreloadFileResolver(t *testing.T) {
	dir := preloadFixtureDir(t)

	filemap := PreloadFiles(PreloadOptions{
		Folders:   []string{dir},
		Ext:       []string{".jsonic"},
		Recursive: true,
	})

	// Remove the files: resolution must come from the preloaded map.
	if err := os.RemoveAll(dir); err != nil {
		t.Fatal(err)
	}

	j := MakeJsonic(MultiSourceOptions{
		Resolver: MakeFileResolver(FileResolverOptions{Preload: filemap}),
		Path:     dir,
		Preload: &PreloadOptions{ // Declarative record of the scan, as in TS.
			Folders:   []string{dir},
			Ext:       []string{".jsonic"},
			Recursive: true,
		},
	})

	r, err := j.Parse(`@"t01.jsonic"`)
	if err != nil {
		t.Fatal(err)
	}
	assert(t, "preload-file-resolver", r, map[string]any{"c": float64(2)})
}

// TestPreloadFS checks the Go-specific io/fs.FS form of PreloadFiles: files
// are read from the injected filesystem and keyed by relative slash paths,
// matching the injected-fs resolver convention.
func TestPreloadFS(t *testing.T) {
	fsys := mapFS(map[string]string{
		"a.jsonic":     `{a:1}`,
		"sub/b.jsonic": `{b:2}`,
		"c.txt":        `CCC`,
	})

	flat := PreloadFiles(PreloadOptions{Folders: []string{"."}}, fsys)
	assert(t, "preload-fs-flat", flat, map[string]string{"a.jsonic": `{a:1}`})

	deep := PreloadFiles(PreloadOptions{Folders: []string{"."}, Recursive: true}, fsys)
	assert(t, "preload-fs-deep", deep, map[string]string{
		"a.jsonic":     `{a:1}`,
		"sub/b.jsonic": `{b:2}`,
	})
}
