/* Copyright (c) 2026 Richard Rodger, MIT License */

// The exported VERSION must equal package.json "version".
//
// This is the CI check for version drift. It exists because the constant HAS
// drifted: @tabnas/json exported Version = '1.0.0' for several releases while
// the package shipped 0.4.x, because nothing rewrote it and AGENTS.md wrongly
// claimed `make publish-go` kept it in sync. A release that bumps
// package.json and forgets the constant now fails here.
//
// Both requires are deliberately at module scope and unguarded: if
// package.json or the built package cannot be read, this file throws and the
// test run goes red. A version check that silently does not run is the exact
// failure mode being designed out, so there is no skip path.

import { describe, it } from 'node:test'
import assert from 'node:assert'

// Resolved from dist-test/, so '..' is the package root (ts/) and picks up
// package.json "main" — this also proves VERSION is reachable from the
// package root, not just from the entry module's deep path.
const pkg = require('../package.json')
const api = require('..')

describe('version', () => {
  it('VERSION matches package.json', () => {
    assert.equal(
      api.VERSION,
      pkg.version,
      `VERSION drift: ${pkg.name} exports ${api.VERSION} but package.json is ` +
        `${pkg.version}. Both are rewritten by admin/publish.sh at release; ` +
        `if you bumped one by hand, bump the other.`,
    )
  })

  it('VERSION is exported and looks like a semver', () => {
    assert.equal(
      typeof api.VERSION,
      'string',
      'VERSION must be exported as a string',
    )
    assert.match(api.VERSION, /^\d+\.\d+\.\d+/, 'VERSION must be a semver')
  })
})
