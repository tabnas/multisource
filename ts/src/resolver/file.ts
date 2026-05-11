/* Copyright (c) 2021-2025 Richard Rodger and other contributors, MIT License */

import * as SystemFs from 'node:fs'
import * as Path from 'node:path'
import { type FST, MultiSourceOptions, Resolver, Resolution, resolvePathSpec, NONE } from '../multisource'
import { Rule, Context } from 'jsonic'
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

    const ps = resolvePathSpec(popts, ctx, foundSpec, resolvefolder)
    let src = undefined

    let search: string[] = []

    if (null != ps.full) {
      ps.full = Path.resolve(ps.full)

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
            potentials.push(Path.resolve(base, 'node_modules', ps.path))
            base = Path.dirname(base)
            if (last === base) break
            last = base
          }
        }

        if (NONE === ps.kind) {
          potentials.push(...
            buildPotentials(ps, popts, (...s) =>
              Path.resolve(s.reduce((a, p) => Path.join(a, p)))))
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

function resolvefolder(path: string, fs: FST) {
  if ('string' !== typeof path) {
    return path
  }

  let folder = path
  let pathstats = fs.statSync(path)

  if (pathstats.isFile()) {
    let pathdesc = Path.parse(path)
    folder = pathdesc.dir
  }

  return folder
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
