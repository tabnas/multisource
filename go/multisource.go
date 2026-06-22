/* Copyright (c) 2025 Richard Rodger, MIT License */

package tabnasmultisource

import (
	"encoding/json"
	"io/fs"
	"path"
	"strings"
	"sync"

	jsonic "github.com/tabnas/jsonic/go"
)

// Version is the Go module release version.
const Version = "0.3.1"

// MultiSourceOptions configures the multisource parser.
type MultiSourceOptions struct {
	Resolver    Resolver
	Path        string
	MarkChar    string
	Processor   map[string]Processor
	ImplicitExt []string

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
}

// Resolver finds source content for a given path spec. The ctx carries the
// parse metadata (ctx.Meta); resolvers may read ctx.Meta["fs"] for a per-parse
// filesystem override. Mirrors the TypeScript Resolver, which receives the
// parse Context.
type Resolver func(spec PathSpec, opts *MultiSourceOptions, ctx *jsonic.Context) Resolution

// Processor converts resolved source content into a value.
//
// The ctx carries the parse metadata for this load (ctx.Meta), including the
// multisource entry whose "path" is the full path of the source being
// processed. Processors that re-parse source (see JsonicProcessor) must thread
// ctx.Meta through so that nested relative references resolve against this
// source's own directory. This mirrors the TypeScript Processor, which
// receives the parse Context.
type Processor func(res *Resolution, opts *MultiSourceOptions, ctx *jsonic.Context, j *jsonic.Jsonic)

// NONE represents an unknown or missing extension.
const NONE = ""

// DefaultProcessor returns the raw source string as the value.
func DefaultProcessor(res *Resolution, opts *MultiSourceOptions, ctx *jsonic.Context, j *jsonic.Jsonic) {
	res.Val = res.Src
}

// JSONProcessor parses JSON source content.
func JSONProcessor(res *Resolution, opts *MultiSourceOptions, ctx *jsonic.Context, j *jsonic.Jsonic) {
	if res.Src == "" {
		res.Val = nil
		return
	}
	var val any
	if err := json.Unmarshal([]byte(res.Src), &val); err != nil {
		res.Val = res.Src
		return
	}
	res.Val = val
}

// JsonicProcessor parses source content using jsonic.
//
// It threads ctx.Meta (which records this source's full path under the
// multisource entry) into the nested parse via ParseMeta, so that relative
// references inside res.Src resolve against this source's own directory rather
// than the top-level base path. Mirrors the canonical TypeScript jsonic
// processor, which calls jsonic(res.src, ctx.meta).
func JsonicProcessor(res *Resolution, opts *MultiSourceOptions, ctx *jsonic.Context, j *jsonic.Jsonic) {
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
		res.Val = res.Src
		return
	}
	res.Val = val
}

// MakeMemResolver creates a resolver that looks up paths in a map. It reads
// from its own in-memory map and ignores ctx / opts.FS.
func MakeMemResolver(files map[string]string) Resolver {
	return func(spec PathSpec, opts *MultiSourceOptions, ctx *jsonic.Context) Resolution {
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

// defaultParser is a lazily-created instance reused by the no-options Parse
// path, so repeated calls don't rebuild the engine and grammar each time.
// Building the grammar dominates a parse — see perf_test.go — so a
// rebuild-per-call Parse is many times slower than reusing one instance.
// Parsing builds a fresh context per call and only reads instance state, so
// the shared instance is safe for concurrent use. Mirrors @tabnas/yaml's Parse.
//
// Only the default (no-options) path is cached: callers that pass a
// MultiSourceOptions get a fresh instance, because the options (resolver,
// processors, base path) configure that instance and must not be shared.
var (
	defaultOnce   sync.Once
	defaultParser *jsonic.Jsonic
)

// Parse parses a jsonic string with multisource support.
func Parse(src string, opts ...MultiSourceOptions) (any, error) {
	if len(opts) == 0 {
		defaultOnce.Do(func() { defaultParser = MakeJsonic() })
		return defaultParser.Parse(src)
	}
	j := MakeJsonic(opts[0])
	return j.Parse(src)
}

// MakeJsonic creates a jsonic instance configured with multisource support.
func MakeJsonic(opts ...MultiSourceOptions) *jsonic.Jsonic {
	var o MultiSourceOptions
	if len(opts) > 0 {
		o = opts[0]
	}

	dopts := defaultOpts()
	if o.MarkChar == "" {
		o.MarkChar = dopts.MarkChar
	}
	if o.Processor == nil {
		o.Processor = dopts.Processor
	}
	if o.ImplicitExt == nil {
		o.ImplicitExt = dopts.ImplicitExt
	}
	if o.Resolver == nil {
		o.Resolver = dopts.Resolver
	}

	for i, ext := range o.ImplicitExt {
		if !strings.HasPrefix(ext, ".") {
			o.ImplicitExt[i] = "." + ext
		}
	}

	bTrue := true

	jopts := jsonic.Options{
		Value: &jsonic.ValueOptions{
			Lex: &bTrue,
		},
	}

	j := jsonic.Make(jopts)

	pluginMap := map[string]any{
		"_opts": &o,
	}
	j.Use(MultiSource, pluginMap)

	return j
}

func defaultOpts() *MultiSourceOptions {
	return &MultiSourceOptions{
		MarkChar: "@",
		Processor: map[string]Processor{
			NONE:     DefaultProcessor,
			"json":   JSONProcessor,
			"jsonic": JsonicProcessor,
			"jsc":    JsonicProcessor,
		},
		ImplicitExt: []string{".jsonic", ".jsc", ".json"},
		Resolver:    MakeMemResolver(map[string]string{}),
	}
}

func getOpts(m map[string]any) *MultiSourceOptions {
	if m == nil {
		return defaultOpts()
	}
	if o, ok := m["_opts"].(*MultiSourceOptions); ok {
		return o
	}
	return defaultOpts()
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
