/* Copyright (c) 2025 Richard Rodger, MIT License */

package tabnasmultisource

import (
	"io/fs"
	"path"
	"strings"

	tabnas "github.com/tabnas/parser/go"
)

// VERSION is this module's version. It MUST equal ts/package.json
// "version": the release orchestrator rewrites both, and
// TestVersionMatchesPackageJSON fails the build if they drift.
const VERSION = "0.6.0"

// PreloadOptions configures folder-scanning preload: read all matching files
// from the specified folders into memory before parsing starts, avoiding
// per-file I/O during parse. Mirrors the TypeScript PreloadOptions.
type PreloadOptions struct {
	Folders   []string // Folders to scan (non-recursive by default).
	Ext       []string // File extensions to load (default: ".jsonic", ".json").
	Recursive bool     // Recurse into subfolders (default: false).
}

// MultiSourceOptions configures the multisource parser.
type MultiSourceOptions struct {
	Resolver    Resolver
	Path        string
	MarkChar    string
	Processor   map[string]Processor
	ImplicitExt []string

	// Preload configures folder-scanning preload, mirroring the TypeScript
	// top-level `preload` option. As in TypeScript, the plugin does not
	// consume it directly: pass it to PreloadFiles and feed the resulting
	// map to FileResolverOptions.Preload.
	Preload *PreloadOptions

	// FS is an optional filesystem for the file and pkg resolvers to read
	// from. When nil, the OS filesystem is used. Supplying an in-memory
	// implementation (for example testing/fstest.MapFS) makes resolution
	// hermetic. A per-parse override may also be passed as ctx.Meta["fs"],
	// mirroring the TypeScript ctx.meta.fs injection point.
	//
	// Note: an io/fs.FS uses relative, slash-separated paths (see fs.ValidPath),
	// so when FS is set the base Path and references resolve relative to the
	// FS root rather than as absolute OS paths.
	FS fs.FS
}

// PathSpec represents a normalized path to a source.
type PathSpec struct {
	Kind string // Source kind, usually normalized file extension.
	Path string // Original path (possibly relative).
	Full string // Normalized full path.
	Base string // Current base path.
	Abs  bool   // Path was absolute.
}

// Resolution is the result of resolving a path spec.
type Resolution struct {
	PathSpec
	Src    string   // Source content.
	Val    any      // Processed value.
	Found  bool     // True if source was found.
	Search []string // List of searched paths.

	// Err reports a failure while PROCESSING found source (for example a
	// nested reference inside it that could not be resolved). The plugin
	// propagates it so the parse fails, matching the TS processor, which lets
	// a nested parse error escape rather than substituting the raw text.
	// Leave nil on success. Not set for "not found" — that is Found=false.
	Err error
}

// Resolver finds source content for a given path spec. The ctx carries the
// parse metadata (ctx.Meta); resolvers may read ctx.Meta["fs"] for a per-parse
// filesystem override. Mirrors the TypeScript Resolver, which receives the
// parse Context.
type Resolver func(spec PathSpec, opts *MultiSourceOptions, ctx *tabnas.Context) Resolution

// Processor converts resolved source content into a value.
//
// The ctx carries the parse metadata for this load (ctx.Meta), including the
// multisource entry whose "path" is the full path of the source being
// processed. Processors that re-parse source (see JsonicProcessor) must thread
// ctx.Meta through so that nested relative references resolve against this
// source's own directory. This mirrors the TypeScript Processor, which
// receives the parse Context.
type Processor func(res *Resolution, opts *MultiSourceOptions, ctx *tabnas.Context, j *tabnas.Tabnas)

// NONE represents an unknown or missing extension.
const NONE = ""

// TOP marks the top of the dependency tree: it is the DependencyMap target key
// used for sources referenced directly by the top-level parse (no enclosing
// source). It is the Go counterpart of the TypeScript exported TOP symbol; the
// leading NUL byte guarantees it can never collide with a real source path.
const TOP = "\x00TOP"

// Dependency records that target Tar pulled in source Src during a parse.
// Mirrors the TypeScript Dependency type (tar/src/wen).
type Dependency struct {
	Tar string `json:"tar"` // Target that depends on source (Src); TOP at the top level.
	Src string `json:"src"` // Source that target (Tar) depends on.
	Wen int64  `json:"wen"` // Time of resolution (Unix milliseconds).
}

// DependencyMap is a flattened dependency tree (assumes each element is a
// unique full path), keyed by target full path, then source full path.
//
// To collect dependencies, pass an empty DependencyMap under the "deps" key of
// the "multisource" parse meta entry; the plugin fills it as sources resolve
// other sources, mirroring the TypeScript `deps` meta:
//
//	deps := tabnasmultisource.DependencyMap{}
//	j.ParseMeta(`@a.jsonic`, map[string]any{
//	    "multisource": map[string]any{"deps": deps},
//	})
//	// deps[tabnasmultisource.TOP] now maps each top-level source to a
//	// Dependency record; nested sources are keyed by their parent's full path.
type DependencyMap map[string]map[string]Dependency

// PluginMeta describes the plugin.
type PluginMeta struct {
	Name string
}

// Meta is the MultiSource plugin metadata, mirroring the TypeScript exported
// `meta` object.
var Meta = PluginMeta{Name: "MultiSource"}

// DefaultProcessor returns the raw source string as the value.
func DefaultProcessor(res *Resolution, opts *MultiSourceOptions, ctx *tabnas.Context, j *tabnas.Tabnas) {
	res.Val = res.Src
}

// JsonicProcessor parses source content by re-parsing it with the host
// parser j (the instance the plugin is installed on, typically jsonic).
//
// It threads ctx.Meta (which records this source's full path under the
// multisource entry) into the nested parse via ParseMeta, so that relative
// references inside res.Src resolve against this source's own directory rather
// than the top-level base path. Mirrors the canonical TypeScript jsonic
// processor, which calls jsonic(res.src, ctx.meta).
func JsonicProcessor(res *Resolution, opts *MultiSourceOptions, ctx *tabnas.Context, j *tabnas.Tabnas) {
	if res.Src == "" {
		res.Val = nil
		return
	}
	var meta map[string]any
	if ctx != nil {
		meta = ctx.Meta
	}
	val, err := j.ParseMeta(res.Src, meta)
	if err != nil {
		// Report it rather than silently yielding the raw source: a nested
		// `@missing` inside a loaded source must fail the whole parse, as it
		// does in TS. res.Val keeps the raw text for callers that inspect the
		// resolution directly.
		res.Val = res.Src
		res.Err = err
		return
	}
	res.Val = val
}

// MakeMemResolver creates a resolver that looks up paths in a map. It reads
// from its own in-memory map and ignores ctx / opts.FS.
func MakeMemResolver(files map[string]string) Resolver {
	return func(spec PathSpec, opts *MultiSourceOptions, ctx *tabnas.Context) Resolution {
		res := Resolution{
			PathSpec: spec,
			Found:    false,
		}

		potentials := buildPotentials(spec.Full, opts.ImplicitExt)
		res.Search = potentials

		for _, p := range potentials {
			if src, ok := files[p]; ok {
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

// ResolvePathSpec normalizes a path specification.
func ResolvePathSpec(specPath string, base string) PathSpec {
	abs := strings.HasPrefix(specPath, "/") || strings.HasPrefix(specPath, "\\")

	var full string
	if abs {
		full = specPath
	} else if specPath != "" {
		if base != "" {
			full = base + "/" + specPath
		} else {
			full = specPath
		}
	}

	kind := extKind(full)

	return PathSpec{
		Kind: kind,
		Path: specPath,
		Full: full,
		Base: base,
		Abs:  abs,
	}
}

// PreloadFiles scans the folders named in opts and returns a flat map of full
// resolved path -> file content for every file matching one of the configured
// extensions (default ".jsonic", ".json"; a missing leading dot is added).
// Folders are scanned non-recursively unless opts.Recursive is set; folders
// that do not exist (and files that cannot be read) are silently skipped.
// Mirrors the TypeScript exported preloadFiles.
//
// By default files are read from the OS filesystem and keyed by absolute path,
// matching the keys used by MakeFileResolver. Pass an io/fs.FS to read from it
// instead, with relative slash-separated keys (the convention used when
// resolving against an injected filesystem).
//
// The result feeds FileResolverOptions.Preload:
//
//	filemap := tabnasmultisource.PreloadFiles(tabnasmultisource.PreloadOptions{
//	    Folders: []string{dir}, Recursive: true,
//	})
//	resolver := tabnasmultisource.MakeFileResolver(
//	    tabnasmultisource.FileResolverOptions{Preload: filemap})
func PreloadFiles(opts PreloadOptions, fsys ...fs.FS) map[string]string {
	var v vfs = osVFS{}
	if len(fsys) > 0 && fsys[0] != nil {
		v = ioVFS{fsys[0]}
	}

	rawExts := opts.Ext
	if len(rawExts) == 0 {
		rawExts = []string{".jsonic", ".json"}
	}
	exts := make([]string, len(rawExts))
	for i, ext := range rawExts {
		if !strings.HasPrefix(ext, ".") {
			ext = "." + ext
		}
		exts[i] = ext
	}

	filemap := map[string]string{}

	var scan func(folder string)
	scan = func(folder string) {
		entries, ok := v.readDir(folder)
		if !ok {
			return
		}
		for _, e := range entries {
			full := v.join(folder, e.Name())
			if e.IsDir() {
				if opts.Recursive {
					scan(full)
				}
				continue
			}
			for _, ext := range exts {
				if strings.HasSuffix(e.Name(), ext) {
					if src, ok := v.readFile(full); ok {
						filemap[full] = src
					}
					break
				}
			}
		}
	}

	for _, folder := range opts.Folders {
		scan(v.canon(folder))
	}

	return filemap
}

// defaultOpts returns the plugin defaults. There is no built-in processor
// for the json kind: a .json source falls through to the NONE (raw text)
// processor like any other unregistered kind, and a caller who wants it
// parsed registers a json processor through the processor option. "json"
// stays an implicit extension, so `@foo` may still find foo.json.
func defaultOpts() *MultiSourceOptions {
	return &MultiSourceOptions{
		MarkChar: "@",
		Processor: map[string]Processor{
			NONE:     DefaultProcessor,
			"jsonic": JsonicProcessor,
			"jsc":    JsonicProcessor,
		},
		ImplicitExt: []string{".jsonic", ".jsc", ".json"},
		Resolver:    MakeMemResolver(map[string]string{}),
	}
}

// withDefaults returns a copy of o with each unset field (MarkChar,
// Processor, ImplicitExt, Resolver) taken from defaultOpts, and each implicit
// extension given its leading dot. It applies to a typed *MultiSourceOptions
// passed under the "_opts" plugin option. A Processor map given here replaces
// the default map rather than adding to it. The caller's struct and slices
// are not modified.
func withDefaults(o *MultiSourceOptions) *MultiSourceOptions {
	d := defaultOpts()
	if o == nil {
		return d
	}
	c := *o
	if c.MarkChar == "" {
		c.MarkChar = d.MarkChar
	}
	if c.Processor == nil {
		c.Processor = d.Processor
	}
	if c.Resolver == nil {
		c.Resolver = d.Resolver
	}
	if c.ImplicitExt == nil {
		c.ImplicitExt = d.ImplicitExt
	} else {
		exts := make([]string, len(c.ImplicitExt))
		for i, e := range c.ImplicitExt {
			if !strings.HasPrefix(e, ".") {
				e = "." + e
			}
			exts[i] = e
		}
		c.ImplicitExt = exts
	}
	return &c
}

func getOpts(m map[string]any) *MultiSourceOptions {
	if m == nil {
		return defaultOpts()
	}
	if o, ok := m["_opts"].(*MultiSourceOptions); ok {
		return withDefaults(o)
	}
	// Also honour the plain option keys, so `j.Use(MultiSource,
	// map[string]any{"resolver": r})` configures the plugin the way
	// `tn.use(MultiSource, {resolver})` does in TypeScript. Previously
	// anything but the internal `_opts` key was silently ignored, leaving
	// the caller with the default (empty) resolver.
	o := defaultOpts()
	if v, ok := m["resolver"].(Resolver); ok {
		o.Resolver = v
	}
	if v, ok := m["path"].(string); ok {
		o.Path = v
	}
	// `markchar` is the canonical name (it is what the TS plugin reads);
	// `markChar` stays accepted as the Go-field-cased alias.
	for _, k := range []string{"markchar", "markChar"} {
		if v, ok := m[k].(string); ok && v != "" {
			o.MarkChar = v
			break
		}
	}
	if v, ok := m["fs"].(fs.FS); ok {
		o.FS = v
	}
	if v, ok := m["processor"].(map[string]Processor); ok {
		for kind, proc := range v {
			o.Processor[kind] = proc
		}
	}
	// `implictExt` is the canonical name (TS spells it that way); accept the
	// Go field spelling too. Values may be a []string or the []any a JSON
	// options blob produces, and an undotted extension is normalised here —
	// withDefaults does the same for "_opts", and buildPotentials would
	// otherwise search for `namefoo` instead of `name.foo`.
	for _, k := range []string{"implictExt", "implicitExt"} {
		exts := toExtList(m[k])
		if exts == nil {
			continue
		}
		for i, e := range exts {
			if !strings.HasPrefix(e, ".") {
				exts[i] = "." + e
			}
		}
		o.ImplicitExt = exts
		break
	}
	return o
}

// toExtList reads an extension list given as []string or as the []any a
// decoded JSON options blob produces. Returns nil when absent or unusable.
func toExtList(v any) []string {
	switch x := v.(type) {
	case []string:
		out := make([]string, len(x))
		copy(out, x)
		return out
	case []any:
		out := make([]string, 0, len(x))
		for _, e := range x {
			if s, ok := e.(string); ok {
				out = append(out, s)
			}
		}
		return out
	}
	return nil
}

func getProcessor(kind string, procmap map[string]Processor) Processor {
	if proc, ok := procmap[kind]; ok {
		return proc
	}
	if proc, ok := procmap[NONE]; ok {
		return proc
	}
	return DefaultProcessor
}

func buildPotentials(fullpath string, implicitExt []string) []string {
	if fullpath == "" {
		return nil
	}
	potentials := []string{fullpath}

	// Determine the final path segment in a separator-agnostic way: the
	// in-memory resolver keys on forward slashes, while the file/pkg resolvers
	// pass OS-native paths (e.g. Windows backslashes from filepath.Abs).
	base := fullpath
	if i := strings.LastIndexAny(fullpath, `/\`); i >= 0 {
		base = fullpath[i+1:]
	}

	if path.Ext(base) == "" {
		// Implicit extensions.
		for _, ie := range implicitExt {
			potentials = append(potentials, fullpath+ie)
		}
		// Folder index file.
		for _, ie := range implicitExt {
			potentials = append(potentials, fullpath+"/index"+ie)
		}
		// Folder index file including the folder name, e.g. foo/index.foo.jsonic.
		if base != "" && base != "." {
			for _, ie := range implicitExt {
				potentials = append(potentials, fullpath+"/index."+base+ie)
			}
		}
	}
	return potentials
}

func extKind(fullpath string) string {
	ext := path.Ext(fullpath)
	if ext == "" {
		return NONE
	}
	return strings.TrimPrefix(ext, ".")
}
