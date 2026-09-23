# libtabnasmultisource — the multisource parser as a C ABI

<!-- tabnas-clib-template: v3 — stamped by admin tasks/adopt-clib.sh;
     edit the template and re-stamp, not this file. -->

The multisource format parser as a C shared library, so languages with no
tabnas port can parse and validate multisource input. This is one of the
per-format tabnas clibs sharing the **uniform ABI** decided by ADR-12:
every such library exports the same five symbols, and which library you
load decides which format you parse — so one generic binding per
language covers the whole fleet. Two format clibs consequently cannot
be statically linked into one binary; load them dynamically.

```sh
./build.sh            # host, into ./dist
ZIG=/path/to/zig ./build.sh all
```

## The contract

| Function | Returns |
|---|---|
| `tabnas_version()` | `{"ok":true,"lib":"libtabnasmultisource","format":"multisource","template":"v3"}` |
| `tabnas_grammar(opts, len)` | `{"ok":true,"handle":N}` — opts reserved, pass `(NULL, 0)`, unless the format notes below define them |
| `tabnas_parse(handle, src, len)` | `{"ok":true,"accept":true[,"value":…]}` or `{"ok":true,"accept":false,"error":{…}}` |
| `tabnas_grammar_free(handle)` | — |
| `tabnas_free(str)` | — |

The rules every tabnas clib shares, each load-bearing:

1. **Every call returns JSON.** A C ABI has one return value and no
   exceptions; each entry point returns a document, so a binding in any
   language is *call, decode* and the error contract is identical
   everywhere.
2. **Three outcomes, not two.** A broken call is `ok:false` with a
   code; input outside the language is `ok:true, accept:false`; an
   accepted input is `ok:true, accept:true` — plus `value` where the
   parse result is JSON-representable.
3. **A rejection is an answer, not a failure.**
4. **Lengths are explicit.** Buffers are not read as NUL-terminated C
   strings; input may legitimately contain a zero byte.
5. **The caller owns what it is given.** Every `char*` must be released
   with `tabnas_free` (malloc'd — `free(3)`); every handle with
   `tabnas_grammar_free`.

Handles are safe to use from several threads: each carries a mutex,
because the underlying engine is not safe for concurrent Parse and an
FFI caller is under no obligation to serialise.

The grammar is installed **natively** — compiled in-process, not
serialized and reloaded. Lexing configuration is part of the accepted
language, and format plugins keep format-specific behaviour as
closures, which cannot cross a data boundary; see
`admin/notes/2026-08-16-clib-ffi-strategy.md` for the full account.

## Consuming without a binding

C, C++, Zig, Swift, Nim and D consume `include/tabnas.h` directly — no
binding layer exists or is needed. Zig example:

```zig
const c = @cImport(@cInclude("tabnas.h"));
// link against libtabnasmultisource; every call returns a JSON []u8 to free with
// c.tabnas_free.
```

## Format notes

Includes are REFUSED, not resolved. The handle is built by `MakeJsonic()` with the package's default resolver. That resolver is an EMPTY in-memory map (`MakeMemResolver`), the same one the package-level `Parse` uses. So this library never reads the filesystem, the network or the environment. Every `@` reference in a directive position rejects with `multisource_not_found`, and the error payload names the path. That covers value, pair, top-level, list-element and object-form `@{path: ...}` references. `accept:true` therefore means a self-contained multisource document: lenient jsonic with no outstanding includes. The mark char is an ender, so an UNQUOTED `@` in text starts a reference: `a: foo@example.com` rejects. A quoted `"foo@example.com"` and an `@` inside a comment have no effect. Options are reserved, so this ABI cannot supply a resolver or a source map. To assemble multi-source documents, use a native tabnas runtime. The invalid sample names `core.go`, a file next to the contract test. A construct with filesystem access, such as `MakeFileResolver()`, accepts that sample and returns the file's text. So the sample catches a sandbox regression, and it also shows the plugin is installed: plain jsonic accepts `a: @core.go` as a string.

## Layout

- `core.go` — the behaviour, in plain Go (testable).
- `tabnas_c.go` — the cgo shim: `(pointer, length)` in, malloc'd string
  out, nothing else. (Go forbids cgo in `_test.go`, which is why the
  behaviour lives in `core.go`.)
- `core_test.go` — the contract: accept/reject samples, unknown-handle,
  reserved options, double-free, concurrency under `-race`.
- `include/tabnas.h`, `tabnas.pc.in` — the header and pkg-config file
  for C-header-native consumers.
