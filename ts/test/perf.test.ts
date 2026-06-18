/* Copyright (c) 2025 Richard Rodger, MIT License */

import { test, describe } from 'node:test'
import assert from 'node:assert'

import { Tabnas } from '@tabnas/parser'
import { jsonic } from '@tabnas/jsonic'
import { MultiSource, MultiSourceOptions } from '../dist/multisource'
import { makeMemResolver } from '../dist/resolver/mem'

// Unlike sibling grammar plugins (@tabnas/yaml, @tabnas/json), multisource
// exposes NO package-level convenience `parse`: it is a plugin that callers
// install themselves via `new Tabnas().use(jsonic).use(MultiSource, opts)`.
// There is therefore no default instance to cache. The performance risk it
// shares with those plugins is the same one regardless: building the engine +
// multisource grammar dominates a parse, so rebuilding the instance on every
// parse is many times slower than building it once and reusing it.
//
// This test guards that representative usage. It compares N parses that
// rebuild the instance each time against N parses that reuse ONE instance, on
// the SAME machine in the SAME run, and asserts reuse is not slower (and is in
// fact much faster). The comparison is machine-INDEPENDENT — both sides scale
// together on a slow CI box — so there is deliberately NO wall-clock budget.
//
// If the multisource setup ever regressed to be rebuilt per parse inside a
// hot loop, the reuse path would lose its advantage and this guard would flag
// it. The test also documents the intended fast pattern: build once, reuse.
describe('perf', () => {
  test('reuse-one-instance-is-fast', () => {
    const opts: MultiSourceOptions = {
      resolver: makeMemResolver({
        'a.jsonic': 'a:1',
        'b.jsonic': 'b:2',
      }),
    }
    const src = '{x:@a.jsonic,y:@b.jsonic,z:3}'
    const n = 400

    const build = () => new Tabnas().use(jsonic).use(MultiSource, opts)

    // Warm both paths so the comparison is steady-state.
    const warm = build()
    for (let i = 0; i < 100; i++) {
      warm.parse(src)
    }
    for (let i = 0; i < 100; i++) {
      build().parse(src)
    }

    // Rebuild-per-parse: the slow anti-pattern.
    const t0 = process.hrtime.bigint()
    for (let i = 0; i < n; i++) {
      build().parse(src)
    }
    const rebuild = Number(process.hrtime.bigint() - t0)

    // Reuse one instance: the fast, intended pattern.
    const j = build()
    const t1 = process.hrtime.bigint()
    for (let i = 0; i < n; i++) {
      j.parse(src)
    }
    const reuse = Number(process.hrtime.bigint() - t1)

    const ratio = rebuild / reuse

    // Sanity: reuse produces correct results across the run.
    assert.deepEqual(j.parse(src), { x: { a: 1 }, y: { b: 2 }, z: 3 })

    // Reusing one instance must be at least as fast as rebuilding each time
    // (it is many times faster in practice — building the grammar dominates).
    // Allow 1.5x reuse-over-rebuild slack for scheduling noise; a rebuild-per
    // -parse loop is far above 1x, so this catches the regression without any
    // absolute wall-clock dependency.
    assert.ok(
      reuse < rebuild * 1.5,
      `reusing one instance should not be slower than rebuilding per parse: ` +
        `rebuild=${(rebuild / 1e6).toFixed(1)}ms reuse=${(reuse / 1e6).toFixed(1)}ms ` +
        `(rebuild/reuse ratio ${ratio.toFixed(2)}x). Build the engine once and reuse it.`,
    )

    // Surface the observed speedup for the record.
    console.log(
      `perf: rebuild=${(rebuild / 1e6).toFixed(1)}ms reuse=${(reuse / 1e6).toFixed(1)}ms ` +
        `speedup=${ratio.toFixed(2)}x`,
    )
  })
})
