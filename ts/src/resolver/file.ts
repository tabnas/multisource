/* Copyright (c) 2021-2025 Richard Rodger and other contributors, MIT License */

import * as SystemFs from 'node:fs'
import * as Path from 'node:path'
import { type FST, MultiSourceOptions, Resolver, Resolution, resolvePathSpec, NONE } from '../multisource'
import { Rule, Context } from '@tabnas/parser'
import { buildPotentials } from './mem'


type PathFinder = (spec: any) => string

type FileResolverOptions = {
  pathfinder?: PathFinder
  preload?: { [fullpath: string]: string }  // Preloaded file contents by full path
}

export function makeFileResolver(
  pathfinderOrOpts?: PathFinder | FileResolverOptions
): Resolver {

  let pathfinder: PathFinder | undefined
  let preload: { [fullpath: string]: string } | undefined

  if ('function' === typeof pathfinderOrOpts) {
    pathfinder = pathfinderOrOpts
  }
  else if (null != pathfinderOrOpts) {
    pathfinder = pathfinderOrOpts.pathfinder
    preload = pathfinderOrOpts.preload
  }

  return function FileResolver(
    spec: any,
    popts: MultiSourceOptions,
    _rule: Rule,
    ctx: Context,
  ): Resolution {
    const fs = ctx.meta?.fs || SystemFs
    const foundSpec = pathfinder ? pathfinder(spec) : spec

    // An injected fs (ctx.meta.fs, e.g. memfs) is keyed by POSIX absolute
    // paths. Use POSIX path semantics for it so Windows' win32 rules don't
    // mangle paths (e.g. turning `//./main.jsonic` into the device path
    // `\\.\main.jsonic`). The real filesystem keeps native semantics.
    //
    // NOTE: decide from whether an fs was explicitly injected, not from
    // object identity of the fs. resolvePathSpec lives in another module with
    // its own `import * as Fs` binding, so the fs it forwards to resolvefolder
    // is never `=== SystemFs` even on the real filesystem.
    const P = null != ctx.meta?.fs ? Path.posix : Path

    const ps = resolvePathSpec(popts, ctx, foundSpec, makeResolveFolder(P))
    let src = undefined

    let search: string[] = []

    if (null != ps.full) {
      ps.full = P.resolve(ps.full)

      search.push(ps.full)

      // Check preloaded files first, then fall back to disk.
      src = preload?.[ps.full] ?? load(ps.full, fs)

      if (null == src) {
        const potentials: string[] = []

        // Special case: support npm linked references
        if (null != ps.base && null != ps.path) {
          let base = ps.base
          let last
          for (let i = 0; i < 7; i++) { // Heuristically check 7 levels of folders
            potentials.push(P.resolve(base, 'node_modules', ps.path))
            base = P.dirname(base)
            if (last === base) break
            last = base
          }
        }

        if (NONE === ps.kind) {
          potentials.push(...
            buildPotentials(ps, popts, (...s) =>
              P.resolve(s.reduce((a, p) => P.join(a, p)))))
        }

        search.push(...potentials)

        for (let path of potentials) {
          src = preload?.[path] ?? load(path, fs)
          if (null != src) {
            ps.full = path
            ps.kind = (path.match(/\.([^.]*)$/) || [NONE, NONE])[1]
            break
          }
        }
      }
    }

    let res: Resolution = {
      ...ps,
      src,
      found: null != src,
      search,
    }

    return res
  }
}

// Build a resolvefolder bound to a path module (POSIX for an injected fs,
// native for the real filesystem). See note in FileResolver.
function makeResolveFolder(P: typeof Path) {
  return function resolvefolder(path: string, fs: FST) {
    if ('string' !== typeof path) {
      return path
    }

    let folder = path
    let pathstats = fs.statSync(path)

    if (pathstats.isFile()) {
      let pathdesc = P.parse(path)
      folder = pathdesc.dir
    }

    return folder
  }
}

function load(path: string, fs: FST) {
  try {
    return fs.readFileSync(path).toString()
  }
  catch (e) {
    // NOTE: don't need this, as in all cases, we consider failed
    // reads to indicate non-existence.
  }
}
