/* Copyright (c) 2025 Richard Rodger, MIT License */

package tabnasmultisource

// The path coercion of an object-form directive, at the unit level. The
// end-to-end contract — which source a numeric reference loads — is
// pinned by ../test/spec/numeric-path.tsv, which every runtime runs.

import (
	"math"
	"testing"
)

// The table in tabnas/multisource#50, plus the two points where
// JavaScript itself switches to exponent form. Every value here is one
// where fmt.Sprintf("%v", f) disagrees, except the ones marked as the
// agreement cases that keep the test honest.
func TestJsNumberToString(t *testing.T) {
	cases := []struct {
		number float64
		want   string
	}{
		{100, "100"},                                        // %v agrees
		{0.0001, "0.0001"},                                  // %v agrees
		{1000000, "1000000"},                                // %v: 1e+06
		{123456789, "123456789"},                            // %v: 1.23456789e+08
		{1000000000000000, "1000000000000000"},              // %v: 1e+15
		{100000000000000000000, "100000000000000000000"},    // %v: 1e+20
		{1000000000000000000000, "1e+21"},                   // the switch
		{0.00001, "0.00001"},                                // %v: 1e-05
		{0.000001, "0.000001"},                              // still fixed
		{0.0000001, "1e-7"},                                 // the switch
		{1.5, "1.5"},                                        // %v agrees
		{-1000000, "-1000000"},                              // %v: -1e+06
		{math.Copysign(0, -1), "0"},                         // %v: -0
		{0, "0"},                                            // %v agrees
		{9007199254740993, "9007199254740992"},              // beyond 2^53
		{1.7976931348623157e308, "1.7976931348623157e+308"}, // max
		{5e-324, "5e-324"},                                  // min subnormal
		{math.NaN(), "NaN"},                                 // not %!v
		{math.Inf(1), "Infinity"},                           // not +Inf
		{math.Inf(-1), "-Infinity"},                         // not -Inf
	}

	for _, c := range cases {
		if got := jsNumberToString(c.number); got != c.want {
			t.Errorf("jsNumberToString(%v) = %q, want %q", c.number, got, c.want)
		}
	}
}

// A non-numeric path value still goes through the fallthrough, and a
// null one is an absent path rather than the text "<nil>".
func TestSpecPathString(t *testing.T) {
	cases := []struct {
		value any
		want  string
	}{
		{"a.jsonic", "a.jsonic"},
		{1000000.0, "1000000"},
		{true, "true"},
		{false, "false"},
		{nil, ""},
	}

	for _, c := range cases {
		if got := specPathString(c.value); got != c.want {
			t.Errorf("specPathString(%v) = %q, want %q", c.value, got, c.want)
		}
	}
}
