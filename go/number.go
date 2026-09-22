/* Copyright (c) 2025 Richard Rodger, MIT License */

package tabnasmultisource

// number.go — how a non-string directive spec becomes a source path.
//
// A directive may be written in object form, `@{path: ...}`, and neither
// runtime requires the `path` value to be a string. The canonical
// coerces it by concatenating it onto the empty string
// (ts/src/multisource.ts), so the coercion rules are JavaScript's, and
// they decide WHICH SOURCE a reference names. A port that formats the value its own way does not
// merely print it differently: it loads a different document, or reports
// a source the author named correctly as missing.

import (
	"fmt"
	"math"
	"strconv"
	"strings"
)

// specPathString is the canonical string coercion of the value under
// the `path` key of an object-form directive.
//
// A missing key and an explicit null both yield the empty string, which
// is what the canonical's null check leaves as an absent path, and what
// resolvePathSpec already treats as one.
func specPathString(value any) string {
	switch v := value.(type) {
	case string:
		return v
	case float64:
		// NOT fmt.Sprintf("%v", v). %v on a float64 is %g, which switches
		// to exponent form as soon as the shortest form is shorter that
		// way — at 1e+06, far below where JavaScript does — and keeps the
		// sign of negative zero. Either one names a different file.
		return jsNumberToString(v)
	case nil:
		return ""
	default:
		return fmt.Sprintf("%v", v)
	}
}

// jsNumberToString is ECMAScript `Number::toString` with radix 10
// (ECMA-262 6.1.6.1.20), which is what the canonical string coercion
// gives, and therefore which source a numeric reference names there.
//
// strconv.FormatFloat(v, 'f', -1, 64) is NOT a substitute. It has no
// switch to exponent form, so 1e21 comes out as twenty-two digits where
// JavaScript writes "1e+21", and 1e-7 as a string of zeros where
// JavaScript writes "1e-7".
//
// The digits come from a FIXED-precision render rather than the shortest
// one. Both round-trip, but they break an exact decimal midpoint
// differently: the shortest form rounds away from zero, while the
// specification takes the even digit, which is what a fixed-precision
// render does.
//
// The same implementation, for the same reason, is in tabnas/csv and
// tabnas/ini (Go) and in rs/src/lib.rs (format_number); keep them in
// step.
func jsNumberToString(f float64) string {
	if math.IsNaN(f) {
		return "NaN"
	}
	if math.IsInf(f, 1) {
		return "Infinity"
	}
	if math.IsInf(f, -1) {
		return "-Infinity"
	}
	// Covers -0, which JavaScript prints as "0".
	if f == 0 {
		return "0"
	}

	magnitude := math.Abs(f)

	// The specification's `s` (the digits) and `n` (where the decimal
	// point sits). Take the digit count from the shortest form, then take
	// the digits themselves at that fixed precision.
	shortest := strconv.FormatFloat(magnitude, 'e', -1, 64)
	mantissa, _, _ := strings.Cut(shortest, "e")
	k := len(strings.Replace(mantissa, ".", "", 1))

	fixed := strconv.FormatFloat(magnitude, 'e', k-1, 64)
	mantissa, exponentText, _ := strings.Cut(fixed, "e")
	digits := strings.Replace(mantissa, ".", "", 1)
	exponent, err := strconv.Atoi(exponentText)
	if err != nil {
		// FormatFloat with 'e' always emits a signed integer exponent.
		return strconv.FormatFloat(f, 'g', -1, 64)
	}
	n := exponent + 1

	var body string
	switch {
	case k <= n && n <= 21:
		// 12 -> "12", 1e19 -> "10000000000000000000"
		body = digits + strings.Repeat("0", n-k)
	case 0 < n && n <= 21:
		// 1.5 -> "1.5"
		body = digits[:n] + "." + digits[n:]
	case -6 < n && n <= 0:
		// 1e-6 -> "0.000001"
		body = "0." + strings.Repeat("0", -n) + digits
	default:
		// 1e21 -> "1e+21", 1e-7 -> "1e-7"
		e := n - 1
		head := digits
		if k > 1 {
			head = digits[:1] + "." + digits[1:]
		}
		sign := "+"
		if e < 0 {
			sign = "-"
			e = -e
		}
		body = head + "e" + sign + strconv.Itoa(e)
	}

	if f < 0 {
		return "-" + body
	}
	return body
}
