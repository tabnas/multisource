/* Copyright (c) 2025 Richard Rodger, MIT License */

package multisource

import (
	"encoding/json"
	"path"
	"strings"

	jsonic "github.com/tabnas/jsonic/go"
)

// Version is the Go module release version.
const Version = "0.1.4"

// MultiSourceOptions configures the multisource parser.
type MultiSourceOptions struct {
	Resolver    Resolver
	Path        string
	MarkChar    string
	Processor   map[string]Processor
	ImplicitExt []string
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

// Resolver finds source content for a given path spec.
type Resolver func(spec PathSpec, opts *MultiSourceOptions) Resolution

// Processor converts resolved source content into a value.
type Processor func(res *Resolution, opts *MultiSourceOptions, j *jsonic.Jsonic)

// NONE represents an unknown or missing extension.
const NONE = ""

// DefaultProcessor returns the raw source string as the value.
func DefaultProcessor(res *Resolution, opts *MultiSourceOptions, j *jsonic.Jsonic) {
	res.Val = res.Src
}

// JSONProcessor parses JSON source content.
func JSONProcessor(res *Resolution, opts *MultiSourceOptions, j *jsonic.Jsonic) {
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
func JsonicProcessor(res *Resolution, opts *MultiSourceOptions, j *jsonic.Jsonic) {
	if res.Src == "" {
		res.Val = nil
		return
	}
	val, err := j.Parse(res.Src)
	if err != nil {
		res.Val = res.Src
		return
	}
	res.Val = val
}

// MakeMemResolver creates a resolver that looks up paths in a map.
func MakeMemResolver(files map[string]string) Resolver {
	return func(spec PathSpec, opts *MultiSourceOptions) Resolution {
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

// Parse parses a jsonic string with multisource support.
func Parse(src string, opts ...MultiSourceOptions) (any, error) {
	var o MultiSourceOptions
	if len(opts) > 0 {
		o = opts[0]
	}
	j := MakeJsonic(o)
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
	ext := path.Ext(fullpath)
	if ext == "" {
		for _, ie := range implicitExt {
			potentials = append(potentials, fullpath+ie)
		}
		for _, ie := range implicitExt {
			potentials = append(potentials, fullpath+"/index"+ie)
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
