/* Copyright (c) 2025 Richard Rodger and other contributors, MIT License */

// Cross-runtime conformance, driven by the shared `test/spec/*.tsv` fixtures
// at the repo root (see ../../test/AGENTS.md).
//
// The fixture loader, the escape codec, the `ERROR:<code>` contract and the
// row loop all come from @tabnas/support, whose Go half `go/parity_test.go`
// uses to run the SAME files — so the two implementations cannot drift
// without one of them going red, and neither can the two loaders.
//
// What is left here is only what is specific to multisource: how to build
// the parser for a row's options.

import { Tabnas } from '@tabnas/parser'
import { jsonic } from '@tabnas/jsonic'
import { findSpecDir, makeRunner } from '@tabnas/support'

import { MultiSource } from '../dist/multisource'
import { makeMemResolver } from '../dist/resolver/mem'

makeRunner({
  parse: (input, row) => {
    // The opts column carries the in-memory source set the case resolves
    // against (`mem`), plus optional engine options (`options`). A real
    // filesystem cannot live in a fixture, so the file/pkg resolvers stay
    // covered by the in-language tests.
    const raw = row.named('opts')
    const opts = '' === raw.trim() ? {} : JSON.parse(raw)

    const tn = new Tabnas().use(jsonic).use(MultiSource, {
      resolver: makeMemResolver(opts.mem ?? {}),
    })
    if (opts.options) tn.options(opts.options)

    return tn.parse(input)
  },
})
  // `findSpecDir` walks up from this file — `dist-test/` at runtime — to the
  // repo root's `test/spec`, so moving the suite does not mean recounting
  // `..` hops. `dir` then auto-discovers every fixture in it, so adding a
  // .tsv runs it in both runtimes without touching either runner.
  .dir(findSpecDir(__dirname))
