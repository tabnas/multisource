/* Copyright (c) 2025 Richard Rodger, MIT License */

package tabnasmultisource

// helpers_test.go — test-only stand-ins for what the package used to ship.
//
// The package no longer builds a host parser (MakeJsonic) or carries a
// json processor (JSONProcessor): users install MultiSource on a parser
// they build and register their own json processor. The tests do the same
// through these helpers, so jsonic stays a test-only dependency.

import (
	"encoding/json"

	jsonic "github.com/tabnas/jsonic/go"
	tabnas "github.com/tabnas/parser/go"
)

// makeJsonic builds a jsonic parser with the MultiSource plugin installed,
// as the removed MakeJsonic did: the same engine options (value lexing on),
// with the typed options passed under "_opts", where unset fields take the
// plugin defaults.
func makeJsonic(opts ...MultiSourceOptions) *jsonic.Jsonic {
	var o MultiSourceOptions
	if len(opts) > 0 {
		o = opts[0]
	}

	bTrue := true
	j := jsonic.Make(jsonic.Options{
		Value: &jsonic.ValueOptions{
			Lex: &bTrue,
		},
	})

	if err := j.Use(MultiSource, map[string]any{"_opts": &o}); err != nil {
		panic(err)
	}
	return j
}

// jsonProcessor parses JSON source content with encoding/json, as the
// removed built-in JSONProcessor did. An empty source yields nil. Malformed
// JSON fails the parse through Resolution.Err, with res.Val left holding the
// raw text for callers that inspect the resolution directly.
func jsonProcessor(res *Resolution, opts *MultiSourceOptions, ctx *tabnas.Context, j *tabnas.Tabnas) {
	if res.Src == "" {
		res.Val = nil
		return
	}
	var val any
	if err := json.Unmarshal([]byte(res.Src), &val); err != nil {
		res.Val = res.Src
		res.Err = err
		return
	}
	res.Val = val
}

// jsonProcessors returns the default processor map with jsonProcessor
// registered for the json kind: the set the plugin defaults used to be. Use
// it for MultiSourceOptions.Processor, which replaces the default map.
func jsonProcessors() map[string]Processor {
	procs := defaultOpts().Processor
	procs["json"] = jsonProcessor
	return procs
}
