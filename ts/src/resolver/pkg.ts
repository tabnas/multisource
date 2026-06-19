/* Copyright (c) 2021-2025 Richard Rodger and other contributors, MIT License */

import * as SystemFs from 'node:fs'
import * as Path from 'node:path'
import { type FST, MultiSourceOptions, Resolver, Resolution, resolvePathSpec, NONE } from '../multisource'
import { Rule, Context } from '@tabnas/parser'
import { buildPotentials } from './mem'


export function makePkgResolver(options: {
  require: Function | string | string[]
}): Resolver {
  let useRequire: {
    resolve: (target: string, options?: any) => any
    main: {
      paths: string[]
    }
  } = require as any

  let requireOptions: any = undefined

  if ('function' === typeof options.require) {
    useRequire = options.require as any
  }
  else if (Array.isArray(options.require)) {
    requireOptions = {
      paths: options.require
    }
  }
  else if ('string' === typeof options.require) {
    requireOptions = {
      paths: [options.require]
    }
  }

  return function PkgResolver(
    spec: any,
    popts: MultiSourceOptions,
    _rule: Rule,
    ctx: Context,
  ): Resolution {
    let fs: FST = ctx.meta?.fs || SystemFs

    // TODO: support pathfinder as file.ts

    let foundSpec = spec

    let ps = resolvePathSpec(popts, ctx, foundSpec, resolvefolder)
    let src = undefined
    let search: string[] = []

    // A relative reference (./x, ../x) inside a source loaded from a package is
    // not a package name: resolve it against the containing source's directory
    // via ps.full, exactly as the file resolver does (this also works over an
    // injected fs). Bare package references fall through to the require /
    // node_modules resolution below.
    if (null != ps.path && isRelativeRef(ps.path) && null != ps.full) {
      let full = Path.resolve(ps.full)
      let potentials = [full]
      if (NONE === ps.kind) {
        potentials.push(...buildPotentials({ ...ps, full }, popts,
          (...s) => Path.resolve(s.reduce((a, p) => Path.join(a, p)))))
      }
      for (let path of potentials) {
        search.push(path)
        src = load(path, fs)
        if (null != src) {
          ps.full = path
          ps.kind = (path.match(/\.([^.]*)$/) || [NONE, NONE])[1]
          break
        }
      }
      return { ...ps, src, found: null != src, search }
    }

    if (null != ps.path) {
      try {
        ps.full = useRequire.resolve(ps.path, requireOptions)
        if (null != ps.full) {
          src = load(ps.full, SystemFs)
          ps.kind = (ps.full.match(/\.([^.]*)$/) || [NONE, NONE])[1]
        }
      }
      catch (me: any) {
        search.push(ps.path)

        let potentials = []

        if (null == ctx.meta?.fs) {
          let localpath = Path.join(process.cwd(), 'NIL')
          let localparts
          do {
            localparts = Path.parse(localpath)
            localpath = localparts.dir
            potentials.push(Path.join(localpath, 'node_modules', ps.path))
          }
          while (localparts.root !== localparts.dir)
        }
        else {
          potentials.push(ps.path)
        }

        if (null != ps.path && 'string' === typeof ps.path) {
          const pspath = ps.path

          // Add the main paths of the current require
          potentials.push(...useRequire.main.paths.map((p: string) => Path.join(p, pspath)))

          // Remove module name prefix
          const subpath = ps.path.replace(/^(@[^/]+\/)?[^/]+\//, '')
          potentials.push(...useRequire.main.paths
            .map((p: string) => p.replace(/node_modules$/, subpath))
          )
        }

        potentials.push(
          ...buildPotentials(ps, popts, (...s) =>
            Path.resolve(s.reduce((a, p) => Path.join(a, p)))))

        // Check longest paths first
        potentials.sort((a, b) => b.length - a.length)


        requireOptions = { paths: ['/'] }

        for (let path of potentials) {
          try {
            ps.full = useRequire.resolve(path, requireOptions)
            if (null != ps.full) {
              src = load(ps.full, SystemFs)
              ps.kind = (ps.full.match(/\.([^.]*)$/) || [NONE, NONE])[1]
              break
            }
          }
          catch (me: any) {
            // require.resolve failed — try the filesystem directly.
            // .jsonic files are text, not JS modules, so require.resolve
            // isn't needed; and it can't see virtual filesystems at all.
            try {
              if (fs.existsSync(path)) {
                ps.full = path
                src = load(ps.full, fs)
                if (null != src) {
                  ps.kind = (path.match(/\.([^.]*)$/) || [NONE, NONE])[1]
                  break
                }
              }
            }
            catch (_e) { /* fall through */ }
            search.push(path)
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


// isRelativeRef reports whether ref is an explicit relative reference (./x or
// ../x). Such a reference is resolved against the containing source's directory
// (via ps.full) rather than treated as a node_modules package name.
function isRelativeRef(ref: string): boolean {
  return ref === '.' || ref === '..' ||
    ref.startsWith('./') || ref.startsWith('../') ||
    ref.startsWith('.\\') || ref.startsWith('..\\')
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
