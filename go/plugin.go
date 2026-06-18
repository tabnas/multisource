/* Copyright (c) 2025 Richard Rodger, MIT License */

package tabnasmultisource

import (
	"fmt"

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

			res := resolveSource(pathStr, opts, j)

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
func resolveSource(pathStr string, opts *MultiSourceOptions, j *jsonic.Jsonic) any {
	spec := ResolvePathSpec(pathStr, opts.Path)
	res := opts.Resolver(spec, opts)

	if !res.Found {
		return nil
	}

	proc := getProcessor(res.Kind, opts.Processor)
	proc(&res, opts, j)

	return res.Val
}
