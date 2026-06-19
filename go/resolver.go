/* Copyright (c) 2021-2025 Richard Rodger and other contributors, MIT License */

package tabnasmultisource

import (
	"encoding/json"
	"io/fs"
	"os"
	"path"
	"path/filepath"
	"strings"

	jsonic "github.com/tabnas/jsonic/go"
)

// FileResolverOptions configures MakeFileResolver.
type FileResolverOptions struct {
	// PathFinder transforms the raw reference path before resolution.
	PathFinder func(spec string) string
	// Preload maps full paths to content, consulted before reading from disk.
	Preload map[string]string
}

// MakeFileResolver creates a resolver that loads sources from the filesystem.
//
// It mirrors the TypeScript makeFileResolver: the reference is resolved to a
// canonical path; when the path has no extension, implicit extensions and index
// files are tried; and a preload map (full path -> content) is consulted before
// touching the filesystem.
//
// By default sources are read from the OS filesystem and references resolve to
// absolute paths. When a filesystem is supplied (via MultiSourceOptions.FS or
// ctx.Meta["fs"]) sources are read from it instead, with references resolved as
// relative, slash-separated paths under the filesystem root — mirroring the
// TypeScript ctx.meta.fs injection point.
func MakeFileResolver(opts ...FileResolverOptions) Resolver {
	var o FileResolverOptions
	if len(opts) > 0 {
		o = opts[0]
	}

	return func(spec PathSpec, mopts *MultiSourceOptions, ctx *jsonic.Context) Resolution {
		// A pathfinder transforms the raw reference before resolution.
		if o.PathFinder != nil {
			spec = ResolvePathSpec(o.PathFinder(spec.Path), spec.Base)
		}

		res := Resolution{PathSpec: spec, Found: false}
		if spec.Full == "" {
			return res
		}

		v := resolveVFS(mopts, ctx)

		full := v.canon(spec.Full)
		res.Full = full

		potentials := buildPotentials(full, mopts.ImplicitExt)
		res.Search = potentials

		for _, p := range potentials {
			if src, ok := o.Preload[p]; ok {
				res.Full = p
				res.Kind = extKind(p)
				res.Src = src
				res.Found = true
				return res
			}
			if src, ok := v.readFile(p); ok {
				res.Full = p
				res.Kind = extKind(p)
				res.Src = src
				res.Found = true
				return res
			}
		}

		return res
	}
}

// PkgResolverOptions configures MakePkgResolver.
type PkgResolverOptions struct {
	// Paths lists directories whose node_modules folders are searched; each is
	// also walked upwards. When empty, the resolver walks up from the current
	// working directory (OS filesystem) or from the root "." (injected
	// filesystem).
	Paths []string
}

// MakePkgResolver creates a resolver that resolves references inside
// node_modules folders, mirroring the TypeScript makePkgResolver.
//
// Go has no equivalent of Node's require.resolve, so this implements the
// portable subset: it walks node_modules directories, honours a package's
// package.json "main" for bare references, and tries implicit extensions and
// index files. It does not implement Node's full module-resolution algorithm
// (for example, conditional "exports").
//
// Like the file resolver, it reads from the OS by default and from an injected
// filesystem (MultiSourceOptions.FS or ctx.Meta["fs"]) when one is supplied.
func MakePkgResolver(opts ...PkgResolverOptions) Resolver {
	var o PkgResolverOptions
	if len(opts) > 0 {
		o = opts[0]
	}

	return func(spec PathSpec, mopts *MultiSourceOptions, ctx *jsonic.Context) Resolution {
		res := Resolution{PathSpec: spec, Found: false}
		ref := spec.Path
		if ref == "" {
			return res
		}

		v := resolveVFS(mopts, ctx)

		// A relative reference (./x, ../x) found inside a source loaded from a
		// package is not a package name: resolve it against the containing
		// source's directory via spec.Full, exactly as the file resolver does.
		// Mirrors the TypeScript pkg resolver, whose fallback search resolves
		// ps.full (base + path) rather than treating it as a bare package.
		if isRelativeRef(ref) && spec.Full != "" {
			full := v.canon(spec.Full)
			potentials := buildPotentials(full, mopts.ImplicitExt)
			res.Search = potentials
			for _, p := range potentials {
				if src, ok := v.readFile(p); ok {
					res.Full = p
					res.Kind = extKind(p)
					res.Src = src
					res.Found = true
					return res
				}
			}
			return res
		}

		var roots []string
		if len(o.Paths) > 0 {
			roots = o.Paths
		} else if _, isOS := v.(osVFS); isOS {
			if cwd, err := os.Getwd(); err == nil {
				roots = []string{cwd}
			}
		} else {
			roots = []string{"."}
		}

		seen := map[string]bool{}
		var search []string

		for _, root := range roots {
			for _, dir := range ancestors(v, root) {
				nm := v.join(dir, "node_modules")
				if seen[nm] {
					continue
				}
				seen[nm] = true

				if full, src, ok := resolveInPkgDir(v, nm, ref, mopts.ImplicitExt, &search); ok {
					res.Full = full
					res.Kind = extKind(full)
					res.Src = src
					res.Found = true
					res.Search = search
					return res
				}
			}
		}

		res.Search = search
		return res
	}
}

// isRelativeRef reports whether ref is an explicit relative reference (./x or
// ../x). Such a reference is resolved against the containing source's directory
// (via spec.Full) rather than treated as a node_modules package name.
func isRelativeRef(ref string) bool {
	return ref == "." || ref == ".." ||
		strings.HasPrefix(ref, "./") || strings.HasPrefix(ref, "../") ||
		strings.HasPrefix(ref, `.\`) || strings.HasPrefix(ref, `..\`)
}

// resolveInPkgDir resolves a package reference inside a node_modules directory,
// trying the reference directly (with implicit extensions and index files) and
// then the target package's package.json "main".
func resolveInPkgDir(v vfs, nodeModules, ref string, exts []string, search *[]string) (full, src string, found bool) {
	target := v.join(nodeModules, ref)

	for _, p := range buildPotentials(target, exts) {
		*search = append(*search, p)
		if s, ok := v.readFile(p); ok {
			return p, s, true
		}
	}

	// Bare package reference: honour package.json "main".
	pkgJSON := v.join(target, "package.json")
	*search = append(*search, pkgJSON)
	if data, ok := v.readFile(pkgJSON); ok {
		var meta struct {
			Main string `json:"main"`
		}
		if json.Unmarshal([]byte(data), &meta) == nil && meta.Main != "" {
			mainPath := v.join(target, meta.Main)
			for _, p := range buildPotentials(mainPath, exts) {
				*search = append(*search, p)
				if s, ok := v.readFile(p); ok {
					return p, s, true
				}
			}
		}
	}

	return "", "", false
}

// vfs is the minimal read-only filesystem view used by the file and pkg
// resolvers. osVFS uses the OS filesystem (absolute paths); ioVFS adapts an
// injected io/fs.FS (relative, slash-separated paths), for example
// testing/fstest.MapFS. This is the Go counterpart to the TypeScript
// ctx.meta.fs abstraction.
type vfs interface {
	// readFile reads the file at a canonical path; ok reports existence.
	readFile(p string) (string, bool)
	// join joins path elements in this filesystem's convention.
	join(parts ...string) string
	// dir returns the parent directory of p.
	dir(p string) string
	// canon returns the canonical lookup form of a (possibly relative) path.
	canon(p string) string
}

// resolveVFS selects the filesystem view: a per-parse ctx.Meta["fs"] override,
// then MultiSourceOptions.FS, then the OS filesystem.
func resolveVFS(opts *MultiSourceOptions, ctx *jsonic.Context) vfs {
	if ctx != nil && ctx.Meta != nil {
		if f, ok := ctx.Meta["fs"].(fs.FS); ok && f != nil {
			return ioVFS{f}
		}
	}
	if opts != nil && opts.FS != nil {
		return ioVFS{opts.FS}
	}
	return osVFS{}
}

// osVFS reads from the OS filesystem using absolute, OS-native paths.
type osVFS struct{}

func (osVFS) readFile(p string) (string, bool) { return loadFile(p) }
func (osVFS) join(parts ...string) string      { return filepath.Join(parts...) }
func (osVFS) dir(p string) string              { return filepath.Dir(p) }
func (osVFS) canon(p string) string {
	if abs, err := filepath.Abs(p); err == nil {
		return abs
	}
	return p
}

// ioVFS reads from an injected io/fs.FS using relative, slash-separated paths.
type ioVFS struct{ fsys fs.FS }

func (v ioVFS) readFile(p string) (string, bool) {
	name := fsClean(p)
	if !fs.ValidPath(name) {
		return "", false
	}
	b, err := fs.ReadFile(v.fsys, name)
	if err != nil {
		return "", false
	}
	return string(b), true
}
func (ioVFS) join(parts ...string) string { return fsClean(path.Join(parts...)) }
func (ioVFS) dir(p string) string         { return path.Dir(fsClean(p)) }
func (ioVFS) canon(p string) string       { return fsClean(p) }

// fsClean normalizes a path into an io/fs.FS name: slash-separated, with "."
// and ".." resolved and any leading slash removed. An empty result becomes ".".
func fsClean(p string) string {
	p = path.Clean(filepath.ToSlash(p))
	p = strings.TrimPrefix(p, "/")
	if p == "" {
		return "."
	}
	return p
}

// loadFile reads a file from the OS, reporting whether it was read
// successfully. A failed read is treated as the source not existing.
func loadFile(p string) (string, bool) {
	b, err := os.ReadFile(p)
	if err != nil {
		return "", false
	}
	return string(b), true
}

// ancestors returns dir followed by each of its parent directories, using the
// directory convention of the given filesystem view.
func ancestors(v vfs, dir string) []string {
	if dir == "" {
		return nil
	}
	var dirs []string
	for {
		dirs = append(dirs, dir)
		parent := v.dir(dir)
		if parent == dir {
			break
		}
		dir = parent
	}
	return dirs
}
