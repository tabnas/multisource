/* Copyright (c) 2021-2025 Richard Rodger and other contributors, MIT License */

import { Rule, Context } from '@tabnas/parser'
import { MultiSourceOptions, Resolver, Resolution, resolvePathSpec, extKind, NONE, PathSpec } from '../multisource'


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
            ps.kind = extKind(path)
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
    // `path` is either the configured base folder (top-level parse) or, for a
    // reference found INSIDE a loaded source, that source's own path. Only the
    // latter is a key of the map, and only it must be reduced to its
    // containing folder, so a relative reference resolves against the source
    // that contains it.
    //
    // The previous regex captured the LAST segment instead of the containing
    // folder, so a source at `b/a.jsonic` gave base `a.jsonic` and a nested
    // `@"c.jsonic"` was searched for at `a.jsonic/c.jsonic` — reported as not
    // found. It only looked correct for flat maps, where the capture fails and
    // yields ''. Test membership with `null !=` too: an empty source file is a
    // legitimate map entry, but it is falsy.
    if (null == path || null == filemap[path]) {
      return path
    }

    // Same rule as Go's sourceDir: no separator -> '' (a bare map key such as
    // 'a.jsonic' has no folder), a separator at index 0 -> the root separator.
    let p = path.replace(/[\\\/]+$/, '')
    let sep = Math.max(p.lastIndexOf('/'), p.lastIndexOf('\\'))
    return sep < 0 ? '' : 0 === sep ? p.slice(0, 1) : p.slice(0, sep)
  }
}


export function buildPotentials(
  ps: PathSpec,
  popts: MultiSourceOptions,
  pathjoin: (...parts: string[]) => string): string[] {
  let full = (ps.full as string)
  let potentials: string[] = []
  if (null == full || '' === full) {
    return potentials
  }
  let implictExt: string[] = popts.implictExt || []

  // "Already has an extension" is a property of the LAST path segment, not of
  // the whole path: `a.d/foo` has none. Testing the string suffix against the
  // implicit-extension list instead diverged from Go (which uses path.Ext on
  // the final segment) for a reference ending in a bare dot.
  let seg = (full.match(/[^\\/]*$/) as string[])[0]
  let hasExt = seg.includes('.')

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
