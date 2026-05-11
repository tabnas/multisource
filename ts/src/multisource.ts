/* Copyright (c) 2025 Richard Rodger, MIT License */

import * as SystemFs from 'node:fs'

import { Jsonic, Context, Rule, Plugin } from 'jsonic'
import { Directive, DirectiveOptions } from '@jsonic/directive'

import { makeJsonicProcessor } from './processor/jsonic'
import { makeJavaScriptProcessor } from './processor/js'

type FST = typeof SystemFs

// TODO: jsonic-cli should provide basepath

// Jsonic parsing meta data. In this case, storing the dependency tree.
interface MultiSourceMeta {
  path?: string // Base path for this parse run.
  parents?: string[] // Parent source paths.
  deps?: DependencyMap // Provide an empty object to be filled.
}

// Unknown source reference file extension.
const NONE = ''

// Preload configuration: read all matching files from specified folders
// into memory before parsing starts, avoiding per-file I/O during parse.
type PreloadOptions = {
  folders: string[]               // Folders to scan (non-recursive by default)
  ext?: string[]                  // File extensions to load (default: ['.jsonic', '.json'])
  recursive?: boolean             // Recurse into subfolders (default: false)
}

// Options for this plugin.
type MultiSourceOptions = {
  resolver: Resolver // Resolve multisource spec to source
  path?: string // Base path, prefixed to paths
  markchar?: string // Character to mark start of multisource directive
  processor?: { [kind: string]: Processor }
  implictExt?: []
  preload?: PreloadOptions // Preload files from folders into memory
}

// The path to the source, including base prefix (if any).
type PathSpec = {
  kind: string // Source kind, usually normalized file extension
  path?: string // Original path (possibly relative)
  full?: string // Normalized full path
  base?: string // Current base path
  abs: boolean // Path was absolute
}

// The source and where it was found.
type Resolution = PathSpec & {
  src?: string // Undefined if no resolution
  val?: any // Undefined if no resolution
  found: boolean // True if source file was found
  search?: string[] // List of searched paths.
}

// Resolve the source.
type Resolver = (
  spec: PathSpec,
  popts: MultiSourceOptions,
  rule: Rule,
  ctx: Context,
  jsonic: Jsonic,
) => Resolution

// Process the source into a value.
type Processor = (
  res: Resolution,
  popts: MultiSourceOptions,
  rule: Rule,
  ctx: Context,
  jsonic: Jsonic,
) => void

type Dependency = {
  tar: string | typeof TOP // Target that depends on source (src).
  src: string // Source that target (tar) depends on.
  wen: number // Time of resolution.
}

// A flattened dependency tree (assumes each element is a unique full path).
type DependencyMap = {
  [tar_full_path: string]: {
    [src_full_path: string]: Dependency
  }
}

// The top of the dependence tree.
const TOP = Symbol('TOP')

const MultiSource: Plugin = (jsonic: Jsonic, popts: MultiSourceOptions) => {
  const markchar = popts.markchar as string
  const resolver = popts.resolver as Resolver
  const processor = popts.processor as { [kind: string]: Processor }

  const { deep } = jsonic.util

  // Normalize implicit extensions to format `.name`.
  const implictExt = (popts.implictExt || []) as string[]
  for (let extI = 0; extI < implictExt.length; extI++) {
    let ext = implictExt[extI]
    implictExt[extI] = ext.startsWith('.') ? ext : '.' + ext
  }

  jsonic.options({
    error: {
      multisource_not_found: 'source not found: {path}',
    },
    hint: {
      // TODO: use $details for more explanation in error message.
      // In particular to show resolved absolute path.
      multisource_not_found:
        'The source path {path} was not found.\n\nSearch paths:\n{searchstr}',
    },
  })

  // Define a directive that can load content from multiple sources.
  let dopts: DirectiveOptions = {
    name: 'multisource',
    open: markchar,
    rules: {
      open: {
        val: {},
        pair: {
          c: (r: Rule) => r.lte('pk')
        },
      }
    },
    action: function multisourceStateAction(rule: Rule, ctx: Context) {
      let from = rule.parent.name
      let spec = rule.child.node

      let res = resolver(spec, popts, rule, ctx, jsonic)

      if (null == res || !res.found) {
        return rule.parent?.o0.bad('multisource_not_found', {
          ...(res || {}),
          searchstr: (res?.search || [res?.full]).join('\n'),
        })
      }

      let fullpath =
        null != res.full ? res.full : null != res.path ? res.path : 'no-path'

      res.kind = null == res.kind ? NONE : res.kind

      // Pass down any meta info.
      let msmeta: MultiSourceMeta = ctx.meta?.multisource || {}
      let parents = msmeta.parents || []
      if (null != msmeta.path) {
        parents.push(msmeta.path)
      }

      let meta: any = {
        ...(ctx.meta || {}),
        fileName: res.path,
        multisource: {
          ...msmeta,
          parents,
          path: res.full,
        },
      }

      if (rule.k.path && Array.isArray(rule.k.path)) {
        meta.path = { base: rule.k.path.slice(0) }
      }

      // Build dependency tree branch.
      if (msmeta.deps) {
        let depmap = msmeta.deps as DependencyMap
        let parent = (msmeta.path || TOP) as string
        if (null != parent) {
          let dep: Dependency = {
            tar: parent,
            src: fullpath,
            wen: Date.now(),
          }
          depmap[parent] = depmap[parent] || {}
          depmap[parent][fullpath] = dep
        }
      }

      // ctx.meta = meta
      let ctxproc = {
        ...ctx,
        meta,
      }

      // let proc = processor[res.kind] || processor[NONE]
      let proc = getProcessor(res.kind, processor)
      proc(res, popts, rule, ctxproc, jsonic)

      // Handle the {@foo} case, injecting keys into parent map.
      if ('pair' === from) {
        if (ctx.cfg.map.merge) {
          rule.parent.parent.node = ctx.cfg.map.merge(
            rule.parent.parent.node,
            res.val,
            rule,
            ctx,
          )
        }
        else if (ctx.cfg.map.extend) {
          rule.parent.parent.node = deep(rule.parent.parent.node, res.val)
        }
        else {
          Object.assign(rule.parent.node, res.val)
        }
      }
      else {
        rule.node = res.val
      }

      return undefined
    },

    custom: (jsonic: Jsonic, { OPEN, name }: any) => {
      // Handle special case of @foo first token - assume a map
      jsonic.grammar({
        rule: {
          val: {
            open: [
              {
                s: [OPEN],
                c: (r: Rule) => 0 < r.n.pk && 'pair' != r.parent.name,
                b: 1,
              },
              {
                s: [OPEN],
                c: (r: Rule) => 0 === r.d,
                p: 'map',
                b: 1,
                n: { [name + '_top']: 1 },
              },
            ],
          },
          map: {
            open: [{
              s: [OPEN],
              c: (r: Rule) => 1 === r.d && 1 === r.n[name + '_top'],
              p: 'pair',
              b: 1,
            }],
            close: [{
              s: [OPEN],
              c: (r: Rule) => 0 < r.n.pk,
              b: 1,
            }],
          },
          pair: {
            close: [{
              s: [OPEN],
              c: (r: Rule) => 0 < r.n.pk,
              b: 1,
            }],
          },
        },
      }, {
        rule: { alt: { g: name } },
      })
    },
  }

  jsonic.use(Directive, dopts)
}

// Convenience maker for Processors
function makeProcessor(process: (src: string, res: Resolution) => any) {
  return (res: Resolution) => (res.val = process(res.src as string, res))
}

// Default is just to insert file contents as a string.
const defaultProcessor = makeProcessor((src: string) => src)

const jsonicJsonParser = Jsonic.make('json' as any)

// TODO: use json plugin to get better error msgs.
const jsonProcessor = makeProcessor((src: string, res: Resolution) =>
  // null == src ? undefined : JSON.parse(src)
  null == src ? undefined : jsonicJsonParser(src, { fileName: res.path }),
)


// let proc = processor[res.kind] || processor[NONE]
function getProcessor(kind: string, procmap: Record<string, Processor>): Processor {
  let proc: Processor = procmap[NONE]
  let procref = procmap[kind]

  // Allow one level of aliasing
  if ('string' === typeof procref) {
    proc = procmap[procref]
  }
  else if ('function' === typeof procref) {
    proc = procref
  }

  return proc
}


const jsonicProcessor = makeJsonicProcessor()
const jsProcessor = makeJavaScriptProcessor()

MultiSource.defaults = {
  markchar: '@',
  processor: {
    [NONE]: defaultProcessor,
    jsonic: jsonicProcessor,
    jsc: jsonicProcessor,
    json: jsonProcessor,
    js: jsProcessor,
  },
  implictExt: ['jsonic', 'jsc', 'json', 'js'],
}

function resolvePathSpec(
  popts: MultiSourceOptions,
  ctx: Context,
  spec: any,
  resolvefolder: (path: string, fs: FST) => string,
): PathSpec {
  const fs = ctx.meta?.fs || SystemFs
  let msmeta = ctx.meta?.multisource
  let base = resolvefolder(
    null == msmeta || null == msmeta.path ? popts.path : msmeta.path,
    fs
  )

  let path =
    'string' === typeof spec
      ? spec
      : null != spec.path
        ? '' + spec.path
        : undefined

  let abs = !!(path?.startsWith('/') || path?.startsWith('\\'))
  let full = abs
    ? path
    : null != path && '' != path
      ? null != base && '' != base
        ? base + '/' + path
        : path
      : undefined

  let kind = null == full ? NONE : (full.match(/\.([^.]*)$/) || [NONE, NONE])[1]

  let res: Resolution = {
    kind,
    path,
    full,
    base,
    abs,
    found: false,
  }

  return res
}

// Preload all files matching the given extensions from the specified folders
// into a flat map keyed by full resolved path.
function preloadFiles(
  opts: PreloadOptions,
  fs?: FST,
): { [fullpath: string]: string } {
  const _fs = fs || SystemFs
  const Path = require('node:path')
  const ext = (opts.ext || ['.jsonic', '.json']).map(e =>
    e.startsWith('.') ? e : '.' + e
  )
  const recursive = opts.recursive || false
  const filemap: { [fullpath: string]: string } = {}

  function scanFolder(folder: string) {
    let entries: string[]
    try {
      entries = _fs.readdirSync(folder) as unknown as string[]
    } catch (_e) {
      return
    }
    for (const name of entries) {
      const full = Path.resolve(folder, name)
      let stat
      try {
        stat = _fs.statSync(full)
      } catch (_e) {
        continue
      }
      if (stat.isDirectory()) {
        if (recursive) scanFolder(full)
      }
      else if (stat.isFile()) {
        if (ext.some((e: string) => name.endsWith(e))) {
          try {
            filemap[full] = _fs.readFileSync(full).toString()
          } catch (_e) { /* skip unreadable */ }
        }
      }
    }
  }

  for (const folder of opts.folders) {
    scanFolder(Path.resolve(folder))
  }

  return filemap
}


// Plugin meta data
const meta = {
  name: 'MultiSource',
}

export type {
  Resolver,
  Resolution,
  Processor,
  MultiSourceOptions,
  PreloadOptions,
  Dependency,
  DependencyMap,
  MultiSourceMeta,
  PathSpec,
  FST,
}

export { MultiSource, resolvePathSpec, preloadFiles, NONE, TOP, meta }

