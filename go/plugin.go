/* Copyright (c) 2025 Richard Rodger, MIT License */

package tabnasmultisource

import (
	"errors"
	"fmt"
	"strings"
	"time"

	directive "github.com/tabnas/directive/go"
	jsonic "github.com/tabnas/jsonic/go"
	tabnas "github.com/tabnas/parser/go"
)

// MultiSource is a jsonic plugin that adds multisource reference support.
// When '@path' is encountered in the input, the path is resolved using
// the configured resolver and processed into a value.
func MultiSource(j *jsonic.Jsonic, pluginOpts map[string]any) error {
	opts := getOpts(pluginOpts)
	markChar := opts.MarkChar
	if markChar == "" {
		markChar = "@"
	}

	cfg := j.Config()

	// Add the mark character to ender chars so built-in matchers stop there.
	if cfg.EnderChars == nil {
		cfg.EnderChars = make(map[rune]bool)
	}
	cfg.EnderChars[rune(markChar[0])] = true

	// Message templates for the not-found error, matching the TS plugin's
	// tn.options({error, hint}) registration.
	j.SetOptions(jsonic.Options{
		Error: map[string]string{
			"multisource_not_found": "source not found: {path}",
		},
		Hint: map[string]string{
			"multisource_not_found": "The source path {path} was not found.\n\nSearch paths:\n{searchstr}",
		},
	})

	// Define a directive that can load content from multiple sources.
	dopts := directive.DirectiveOptions{
		Name: "multisource",
		Open: markChar,
		Rules: &directive.RulesOption{
			Open: map[string]*directive.RuleMod{
				"val": {},
				"pair": {
					C: func(r *jsonic.Rule, ctx *jsonic.Context) bool {
						return r.Lte("pk", 0)
					},
				},
			},
		},
		Action: func(rule *jsonic.Rule, ctx *jsonic.Context) {
			spec := rule.Child.Node

			var pathStr string
			switch v := spec.(type) {
			case string:
				pathStr = v
			default:
				// The directive spec may be an object form ({path: ...}).
				// A parsed object is now an insertion-ordered *OrderedMap;
				// AsStringMap unwraps either that or a plain map for value
				// access (order is irrelevant when reading a single key).
				if m, ok := tabnas.AsStringMap(v); ok {
					if p, ok := m["path"]; ok {
						pathStr = fmt.Sprintf("%v", p)
					}
				}
			}

			res, missing, perr := resolveSource(pathStr, opts, ctx, j)
			if perr != nil {
				// A source resolved but failed to parse — typically a nested
				// `@missing` inside it. Re-raise the nested code (so a
				// recursive missing reference still reports
				// multisource_not_found) instead of quietly substituting the
				// raw source text.
				code := "unexpected"
				var je *jsonic.JsonicError
				if errors.As(perr, &je) && je.Code != "" {
					code = je.Code
				}
				tkn := ctx.T0
				if rule.Parent != nil && rule.Parent != jsonic.NoRule && rule.Parent.O0 != nil {
					tkn = rule.Parent.O0
				}
				if tkn != nil {
					ctx.ParseErr = tkn.Bad(code, map[string]any{"path": pathStr})
				}
				return
			}
			if missing != nil {
				// Mirror the TS action: a reference that cannot be resolved
				// halts the parse with multisource_not_found, reporting the
				// paths that were searched.
				search := missing.Search
				if len(search) == 0 && missing.Full != "" {
					search = []string{missing.Full}
				}
				details := map[string]any{
					"path":      missing.Path,
					"full":      missing.Full,
					"searchstr": strings.Join(search, "\n"),
				}
				if details["path"] == "" {
					details["path"] = pathStr
				}
				tkn := ctx.T0
				if rule.Parent != nil && rule.Parent != jsonic.NoRule && rule.Parent.O0 != nil {
					tkn = rule.Parent.O0
				}
				if tkn != nil {
					ctx.ParseErr = tkn.Bad("multisource_not_found", details)
				}
				return
			}

			from := ""
			if rule.Parent != nil && rule.Parent != jsonic.NoRule {
				from = rule.Parent.Name
			}

			// Handle the {@foo} case, injecting keys into parent map. Mirror
			// the TS deep-merge (`deep(gp.node, res.val)`): the loaded map is
			// deep-merged into the grandparent map IN PLACE, not a shallow
			// per-key overwrite. Two requirements:
			//   - existing nested values survive: `a:{d:3} @b.jsonic`
			//     (b => a:{b,c}) must give {a:{d:3,b:1,c:2}}, not drop `d:3`.
			//   - the grandparent map reference must stay stable, so a pair
			//     that follows the directive (`@a.jsonic b:2`) writes into the
			//     same node. TS's `deep` mutates its base in place; Go's `Deep`
			//     returns a fresh map, so merge key-by-key back into gp instead
			//     of reassigning gp.Node.
			if from == "pair" {
				if rule.Parent.Parent != nil && rule.Parent.Parent != jsonic.NoRule {
					gp := rule.Parent.Parent
					mergeIntoParent(gp.Node, res, rule, ctx)
				}
			} else {
				rule.Node = res
			}
		},
		Custom: func(j *jsonic.Jsonic, cfg directive.DirectiveConfig) {
			name := cfg.Name
			openToken := "#OD_" + name
			topCounter := name + "_top"

			// Handle special case of @foo first token - assume a map.
			err := j.Grammar(&jsonic.GrammarSpec{
				Ref: map[jsonic.FuncRef]any{
					"@pk-pos": jsonic.AltCond(func(r *jsonic.Rule, ctx *jsonic.Context) bool {
						return r.N["pk"] > 0
					}),
					// val-open back-track condition. Mirrors the canonical
					// TypeScript `0 < r.n.pk && 'pair' != r.parent.name`: only
					// back-track the @ up the path dive when we are inside a
					// dive AND the @ is NOT already the value of a pair. In a
					// colon-chain (`a: b: @"f"`) the value-position val rule has
					// pk > 0 but its parent IS the pair for that key, so this
					// stays false and the import resolves nested under the key
					// rather than unwinding to depth 0 (which drops it to null).
					"@pk-pos-val": jsonic.AltCond(func(r *jsonic.Rule, ctx *jsonic.Context) bool {
						return r.N["pk"] > 0 &&
							(r.Parent == nil || r.Parent == jsonic.NoRule || r.Parent.Name != "pair")
					}),
					"@d-zero": jsonic.AltCond(func(r *jsonic.Rule, ctx *jsonic.Context) bool {
						return r.D == 0
					}),
					"@d-one-top": jsonic.AltCond(func(r *jsonic.Rule, ctx *jsonic.Context) bool {
						return r.D == 1 && r.N[topCounter] == 1
					}),
				},
				Rule: map[string]*jsonic.GrammarRuleSpec{
					"val": {
						Open: []*jsonic.GrammarAltSpec{
							{S: openToken, C: "@pk-pos-val", B: 1},
							{S: openToken, C: "@d-zero", P: "map", B: 1, N: map[string]int{topCounter: 1}},
						},
					},
					"map": {
						Open: []*jsonic.GrammarAltSpec{
							{S: openToken, C: "@d-one-top", P: "pair", B: 1},
						},
						Close: []*jsonic.GrammarAltSpec{
							{S: openToken, C: "@pk-pos", B: 1},
						},
					},
					"pair": {
						Close: []*jsonic.GrammarAltSpec{
							{S: openToken, C: "@pk-pos", B: 1},
						},
					},
				},
			}, &jsonic.GrammarSetting{
				Rule: &jsonic.GrammarSettingRule{
					Alt: &jsonic.GrammarSettingAlt{G: name},
				},
			})
			if err != nil {
				panic(err)
			}
		},
	}

	directive.Apply(j, dopts)
	return nil
}

// mergeIntoParent deep-merges the loaded value res into the grandparent map
// node in place, mirroring the TypeScript `deep(gp.node, res.val)`. It handles
// the parser's insertion-ordered *OrderedMap object node (now the default) as
// well as a plain map[string]any, so nested values survive, the grandparent
// reference stays stable (following pairs write into the same node), and the
// loaded source's key order is preserved.
//
// Non-object nodes/values (either side not object-shaped) are a no-op, matching
// the prior behaviour where the type assertion simply failed.
func mergeIntoParent(gpNode, res any, rule *jsonic.Rule, ctx *jsonic.Context) {
	set, ok := parentSetter(gpNode)
	if !ok {
		return
	}
	m, ok := tabnas.AsStringMap(res)
	if !ok {
		return
	}
	// Iterate in the loaded source's key order when res carries one, so keys
	// injected into the grandparent map follow the loaded file's order.
	for _, k := range orderedKeys(res, m) {
		v := m[k]
		existing := set.get(k)
		if ctx.Cfg.MapMerge != nil {
			set.put(k, ctx.Cfg.MapMerge(existing, v, rule, ctx))
		} else if ctx.Cfg.MapExtend {
			set.put(k, jsonic.Deep(existing, v))
		} else {
			set.put(k, v)
		}
	}
}

// parentGetSet reads and writes keys on an object node (an *OrderedMap or a
// plain map[string]any) while preserving whichever representation the node uses.
type parentGetSet struct {
	om *jsonic.OrderedMap
	pm map[string]any
}

func (p parentGetSet) get(k string) any {
	if p.om != nil {
		v, _ := p.om.Get(k)
		return v
	}
	return p.pm[k]
}

func (p parentGetSet) put(k string, v any) {
	if p.om != nil {
		p.om.Set(k, v)
		return
	}
	p.pm[k] = v
}

// parentSetter returns a getter/setter over the grandparent object node,
// reporting whether the node was object-shaped.
func parentSetter(node any) (parentGetSet, bool) {
	switch n := node.(type) {
	case *jsonic.OrderedMap:
		return parentGetSet{om: n}, true
	case map[string]any:
		if n == nil { // a typed-nil map is not a usable merge target
			return parentGetSet{}, false
		}
		return parentGetSet{pm: n}, true
	}
	return parentGetSet{}, false
}

// orderedKeys returns the keys of the loaded value in source order when it is an
// *OrderedMap, else the plain map's keys (Go map-iteration order — order is
// irrelevant for a plain map, which has none recorded).
func orderedKeys(res any, m map[string]any) []string {
	if om, ok := res.(*jsonic.OrderedMap); ok {
		return om.Keys
	}
	keys := make([]string, 0, len(m))
	for k := range m {
		keys = append(keys, k)
	}
	return keys
}

// resolveSource resolves a multisource path and returns the processed value.
//
// Relative references resolve against the directory of the *current* source.
// For a top-level parse that is opts.Path; for a reference loaded from inside
// another source it is that source's own directory. The current source's full
// path is threaded through ctx.Meta["multisource"]["path"], mirroring the
// canonical TypeScript @jsonic/multisource (ctx.meta.multisource.path). This
// makes nested loads (a -> b -> c) resolve each relative reference against the
// source that contains it, at any nesting depth, without mutating the shared
// options. Sibling loads are unaffected because the parent context is copied,
// not modified.
// The second return is non-nil when the source could not be FOUND — an
// unresolvable reference is an error (multisource_not_found), not a silent
// nil, matching the TS action's
// `rule.parent?.o0.bad('multisource_not_found', ...)`. The third is non-nil
// when found source failed to PROCESS, e.g. a nested reference inside it
// could not be resolved; TS lets that escape too.
func resolveSource(pathStr string, opts *MultiSourceOptions, ctx *jsonic.Context, j *jsonic.Jsonic) (any, *Resolution, error) {
	base := opts.Path
	if parent := metaSourcePath(ctx); parent != "" {
		base = sourceDir(parent)
	}

	spec := ResolvePathSpec(pathStr, base)
	res := opts.Resolver(spec, opts, ctx)

	if !res.Found {
		return nil, &res, nil
	}

	// Build the dependency tree branch, mirroring the TypeScript action: when
	// the parse meta carries a DependencyMap under multisource.deps, record
	// that the enclosing source (or TOP for the top-level parse) pulled in
	// this resolution. The map travels by reference through the copied child
	// meta, so nested loads keep filling the same tree.
	if deps := metaDeps(ctx); deps != nil {
		tar := metaSourcePath(ctx)
		if tar == "" {
			tar = TOP
		}
		fullpath := res.Full
		if fullpath == "" {
			fullpath = res.Path
		}
		if fullpath == "" {
			fullpath = "no-path"
		}
		if deps[tar] == nil {
			deps[tar] = map[string]Dependency{}
		}
		deps[tar][fullpath] = Dependency{
			Tar: tar,
			Src: fullpath,
			Wen: time.Now().UnixMilli(),
		}
	}

	// Process in a child context whose meta records this source's full path, so
	// any relative references inside res.Src resolve against this source's
	// directory. The parent context (and its meta) are left unmodified.
	childCtx := *ctx
	childCtx.Meta = childMeta(ctx.Meta, &res)

	proc := getProcessor(res.Kind, opts.Processor)
	proc(&res, opts, &childCtx, j)

	return res.Val, nil, res.Err
}

// metaSourcePath returns the full path of the source currently being parsed,
// as threaded through ctx.Meta["multisource"]["path"]. It is empty for a
// top-level parse (no enclosing source).
func metaSourcePath(ctx *jsonic.Context) string {
	if ctx == nil || ctx.Meta == nil {
		return ""
	}
	ms, ok := ctx.Meta["multisource"].(map[string]any)
	if !ok {
		return ""
	}
	p, _ := ms["path"].(string)
	return p
}

// metaDeps returns the DependencyMap threaded through the parse meta as
// ctx.Meta["multisource"]["deps"], or nil when the caller did not ask for
// dependency tracking. Mirrors the TypeScript ctx.meta.multisource.deps.
func metaDeps(ctx *jsonic.Context) DependencyMap {
	if ctx == nil || ctx.Meta == nil {
		return nil
	}
	ms, ok := ctx.Meta["multisource"].(map[string]any)
	if !ok {
		return nil
	}
	deps, _ := ms["deps"].(DependencyMap)
	return deps
}

// sourceDir returns the directory portion of a source path, used as the base
// for relative references found inside that source. A path with no separator
// (an in-memory resolver key such as "a.jsonic") yields "", so bare nested
// references resolve plainly — matching the TypeScript mem resolver. A path
// that contains separators yields everything up to the last one (its
// containing directory), matching the TypeScript file/pkg resolver for a
// loaded file.
func sourceDir(p string) string {
	i := strings.LastIndexAny(p, `/\`)
	if i < 0 {
		return ""
	}
	if i == 0 {
		return p[:1] // filesystem root: keep the separator
	}
	return p[:i]
}

// childMeta returns a copy of the parent parse meta with the multisource entry
// updated to record path (the full path of the source about to be processed)
// and parents (the chain of enclosing source paths). The parent map is not
// mutated. Mirrors the meta construction in the TypeScript plugin action.
func childMeta(parent map[string]any, res *Resolution) map[string]any {
	child := make(map[string]any, len(parent)+1)
	for k, v := range parent {
		child[k] = v
	}

	var prevMS map[string]any
	if m, ok := parent["multisource"].(map[string]any); ok {
		prevMS = m
	}

	var parents []string
	if ps, ok := prevMS["parents"].([]string); ok {
		parents = append(parents, ps...)
	}
	if prev, ok := prevMS["path"].(string); ok && prev != "" {
		parents = append(parents, prev)
	}

	ms := make(map[string]any, len(prevMS)+2)
	for k, v := range prevMS {
		ms[k] = v
	}
	ms["path"] = res.Full
	ms["parents"] = parents

	child["multisource"] = ms
	return child
}
