/* Copyright (c) 2026 tabnas, MIT License */

// Composition test: the MultiSource grammar plugin layered with the official
// @tabnas/debug plugin. @tabnas/debug is a devDependency, but this resolves it
// dynamically and SKIPS when it is absent so the suite stays runnable outside
// the package; CI can point TABNAS_DEBUG_PATH at a sibling checkout's built
// plugin.

import { describe, it } from 'node:test'
import assert from 'node:assert'

import { Tabnas } from '@tabnas/parser'
import { jsonic } from '@tabnas/jsonic'
import { MultiSource } from '../dist/multisource'

function loadDebug(): any {
  const candidates = [process.env.TABNAS_DEBUG_PATH, '@tabnas/debug'].filter(
    Boolean,
  ) as string[]
  for (const c of candidates) {
    try {
      return require(c).Debug
    } catch {
      /* try next */
    }
  }
  return null
}

const Debug = loadDebug()
const skip = Debug ? false : '@tabnas/debug not available (set TABNAS_DEBUG_PATH)'

describe('compose: multisource + @tabnas/debug', () => {
  it('parses normally with the debug plugin installed', { skip }, () => {
    const tn = new Tabnas().use(jsonic).use(MultiSource, {})
    tn.use(Debug, { print: false, trace: false })
    assert.deepStrictEqual(
      JSON.parse(JSON.stringify(tn.parse('{"a":[1,2]}'))),
      { a: [1, 2] },
    )
  })

  it('debug.model() returns the structured grammar', { skip }, () => {
    const tn = new Tabnas().use(jsonic).use(MultiSource, {})
    tn.use(Debug, { print: false, trace: false })
    const m = tn.debug.model()

    // The structured rule set: the shared val/map/list/pair/elem rules plus
    // the MultiSource-specific `multisource` rule.
    assert.deepStrictEqual(
      m.rules.map((r: any) => r.name).sort(),
      ['elem', 'list', 'map', 'multisource', 'pair', 'val'],
    )

    // The entry rule and the installed plugins.
    assert.equal(m.config.start, 'val')
    assert.ok(
      m.plugins.some((p: any) => p.name === 'MultiSource'),
      'plugins should list MultiSource',
    )

    // Structural facts specific to this grammar: the `@`-marked multisource
    // directive wires `val` into a `multisource` rule, which in turn parses a
    // `val` for the resolved source.
    const val = m.rules.find((r: any) => r.name === 'val')
    assert.ok(
      val.open.some((a: any) => a.push === 'multisource'),
      'val should push multisource',
    )
    const multisource = m.rules.find((r: any) => r.name === 'multisource')
    assert.ok(
      multisource.open.some((a: any) => a.push === 'val'),
      'multisource should push val',
    )

    // The grammar portion is JSON-serialisable and round-trips.
    const grammar = {
      tokens: m.tokens,
      rules: m.rules,
      graph: m.graph,
      config: m.config,
      abnf: m.abnf,
    }
    assert.deepStrictEqual(
      JSON.parse(JSON.stringify(grammar)).rules,
      m.rules,
    )
  })
})
