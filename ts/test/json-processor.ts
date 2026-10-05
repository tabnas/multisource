/* Copyright (c) 2026 Richard Rodger and other contributors, MIT License */

// A JSON source processor, as an application now supplies one: multisource
// ships none, so without it a `.json` source is raw text. The tests register
// this one wherever a case includes a `.json` source, so their expectations
// are the ones the built-in processor gave. It is the strict JSON grammar of
// jsonic's factory, the parser the plugin used before it dropped its jsonic
// peer; jsonic stays a devDependency for the tests.

import { Jsonic } from '@tabnas/jsonic'
import type { Resolution } from '../dist/multisource'

const strict = Jsonic.make('json' as any)

export function jsonProcessor(res: Resolution) {
  res.val = null == res.src ? undefined : strict(res.src, { fileName: res.path })
}
