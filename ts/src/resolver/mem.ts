/* Copyright (c) 2021-2025 Richard Rodger and other contributors, MIT License */

import { Rule, Context } from 'jsonic'
import { MultiSourceOptions, Resolver, Resolution, resolvePathSpec, NONE, PathSpec } from '../multisource'


export function makeMemResolver(filemap: { [fullpath: string]: string }): Resolver {

  return function MemResolver(
    spec: any,
    popts: MultiSourceOptions,
    _rule: Rule,
    ctx: Context,
  ): Resolution {
    // TODO: support pathfinder as file.ts

    let ps = resolvePathSpec(popts, ctx, spec, makeresolvefolder(filemap))
    let src = undefined

    if (null != ps.full) {
      src = filemap[ps.full]

      if (null == src && NONE === ps.kind) {
        let potentials =
          buildPotentials(ps, popts, (...s) =>
            s.reduce((a, p) => a + '/' + p))

        for (let path of potentials) {
          if (null != (src = filemap[path])) {
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
      found: null != src
    }

    return res
  }
}


function makeresolvefolder(filemap: { [fullpath: string]: string }) {
  return function resolvefolder(path: string) {
    let folder = path
    if (filemap[path]) {
      folder = (path
        .replace(/[\\\/]+$/, '')
        .match(/[\\\/]+([^\\\/]+)$/) || ['', ''])[1]
    }
    return folder
  }
}


export function buildPotentials(
  ps: PathSpec,
  popts: MultiSourceOptions,
  pathjoin: (...parts: string[]) => string): string[] {
  let full = (ps.full as string)
  let potentials: string[] = []
  let implictExt: string[] = popts.implictExt || []
  let hasExt = implictExt.some(ext => full.endsWith(ext))

  // TODO: use Jsonic.util.escre
  if (!hasExt) {
    // Implicit extensions.
    for (let ext of implictExt) {
      potentials.push(full + ext)
    }

    // Folder index file.
    for (let ext of implictExt) {
      potentials.push(pathjoin(full, 'index' + ext))
    }

    // Folder index file (includes folder name).
    if (null != ps.path) {
      let folder = (ps.path
        .replace(/[\\\/]+$/, '')
        .match(/[^\\\/]+$/) || [])[0]
      if (null != folder) {
        for (let ext of implictExt) {
          potentials.push(pathjoin(full, 'index.' + folder + ext))
        }
      }
    }
  }

  return potentials
}
