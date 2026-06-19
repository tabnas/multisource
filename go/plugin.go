/* Copyright (c) 2025 Richard Rodger, MIT License */

package tabnasmultisource

import (
	"fmt"
	"strings"

	directive "github.com/tabnas/directive/go"
	jsonic "github.com/tabnas/jsonic/go"
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
			case map[string]any:
				if p, ok := v["path"]; ok {
					pathStr = fmt.Sprintf("%v", p)
				}
			}

			res := resolveSource(pathStr, opts, ctx, j)

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
					if parent, ok := gp.Node.(map[string]any); ok {
						if m, ok := res.(map[string]any); ok {
							for k, v := range m {
								if ctx.Cfg.MapMerge != nil {
									parent[k] = ctx.Cfg.MapMerge(parent[k], v, rule, ctx)
								} else if ctx.Cfg.MapExtend {
									parent[k] = jsonic.Deep(parent[k], v)
								} else {
									parent[k] = v
								}
							}
						}
					}
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
							{S: openToken, C: "@pk-pos", B: 1},
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
func resolveSource(pathStr string, opts *MultiSourceOptions, ctx *jsonic.Context, j *jsonic.Jsonic) any {
	base := opts.Path
	if parent := metaSourcePath(ctx); parent != "" {
		base = sourceDir(parent)
	}

	spec := ResolvePathSpec(pathStr, base)
	res := opts.Resolver(spec, opts, ctx)

	if !res.Found {
		return nil
	}

	// Process in a child context whose meta records this source's full path, so
	// any relative references inside res.Src resolve against this source's
	// directory. The parent context (and its meta) are left unmodified.
	childCtx := *ctx
	childCtx.Meta = childMeta(ctx.Meta, &res)

	proc := getProcessor(res.Kind, opts.Processor)
	proc(&res, opts, &childCtx, j)

	return res.Val
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
