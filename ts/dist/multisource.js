"use strict";
/* Copyright (c) 2025 Richard Rodger, MIT License */
var __createBinding = (this && this.__createBinding) || (Object.create ? (function(o, m, k, k2) {
    if (k2 === undefined) k2 = k;
    var desc = Object.getOwnPropertyDescriptor(m, k);
    if (!desc || ("get" in desc ? !m.__esModule : desc.writable || desc.configurable)) {
      desc = { enumerable: true, get: function() { return m[k]; } };
    }
    Object.defineProperty(o, k2, desc);
}) : (function(o, m, k, k2) {
    if (k2 === undefined) k2 = k;
    o[k2] = m[k];
}));
var __setModuleDefault = (this && this.__setModuleDefault) || (Object.create ? (function(o, v) {
    Object.defineProperty(o, "default", { enumerable: true, value: v });
}) : function(o, v) {
    o["default"] = v;
});
var __importStar = (this && this.__importStar) || (function () {
    var ownKeys = function(o) {
        ownKeys = Object.getOwnPropertyNames || function (o) {
            var ar = [];
            for (var k in o) if (Object.prototype.hasOwnProperty.call(o, k)) ar[ar.length] = k;
            return ar;
        };
        return ownKeys(o);
    };
    return function (mod) {
        if (mod && mod.__esModule) return mod;
        var result = {};
        if (mod != null) for (var k = ownKeys(mod), i = 0; i < k.length; i++) if (k[i] !== "default") __createBinding(result, mod, k[i]);
        __setModuleDefault(result, mod);
        return result;
    };
})();
Object.defineProperty(exports, "__esModule", { value: true });
exports.meta = exports.TOP = exports.NONE = exports.MultiSource = void 0;
exports.resolvePathSpec = resolvePathSpec;
exports.preloadFiles = preloadFiles;
const SystemFs = __importStar(require("node:fs"));
const jsonic_1 = require("jsonic");
const directive_1 = require("@jsonic/directive");
const jsonic_2 = require("./processor/jsonic");
const js_1 = require("./processor/js");
// Unknown source reference file extension.
const NONE = '';
exports.NONE = NONE;
// The top of the dependence tree.
const TOP = Symbol('TOP');
exports.TOP = TOP;
const MultiSource = (jsonic, popts) => {
    const markchar = popts.markchar;
    const resolver = popts.resolver;
    const processor = popts.processor;
    const { deep } = jsonic.util;
    // Normalize implicit extensions to format `.name`.
    const implictExt = (popts.implictExt || []);
    for (let extI = 0; extI < implictExt.length; extI++) {
        let ext = implictExt[extI];
        implictExt[extI] = ext.startsWith('.') ? ext : '.' + ext;
    }
    jsonic.options({
        error: {
            multisource_not_found: 'source not found: {path}',
        },
        hint: {
            // TODO: use $details for more explanation in error message.
            // In particular to show resolved absolute path.
            multisource_not_found: 'The source path {path} was not found.\n\nSearch paths:\n{searchstr}',
        },
    });
    // Define a directive that can load content from multiple sources.
    let dopts = {
        name: 'multisource',
        open: markchar,
        rules: {
            open: {
                val: {},
                pair: {
                    c: (r) => r.lte('pk')
                },
            }
        },
        action: function multisourceStateAction(rule, ctx) {
            let from = rule.parent.name;
            let spec = rule.child.node;
            let res = resolver(spec, popts, rule, ctx, jsonic);
            if (null == res || !res.found) {
                return rule.parent?.o0.bad('multisource_not_found', {
                    ...(res || {}),
                    searchstr: (res?.search || [res?.full]).join('\n'),
                });
            }
            let fullpath = null != res.full ? res.full : null != res.path ? res.path : 'no-path';
            res.kind = null == res.kind ? NONE : res.kind;
            // Pass down any meta info.
            let msmeta = ctx.meta?.multisource || {};
            let parents = msmeta.parents || [];
            if (null != msmeta.path) {
                parents.push(msmeta.path);
            }
            let meta = {
                ...(ctx.meta || {}),
                fileName: res.path,
                multisource: {
                    ...msmeta,
                    parents,
                    path: res.full,
                },
            };
            if (rule.k.path && Array.isArray(rule.k.path)) {
                meta.path = { base: rule.k.path.slice(0) };
            }
            // Build dependency tree branch.
            if (msmeta.deps) {
                let depmap = msmeta.deps;
                let parent = (msmeta.path || TOP);
                if (null != parent) {
                    let dep = {
                        tar: parent,
                        src: fullpath,
                        wen: Date.now(),
                    };
                    depmap[parent] = depmap[parent] || {};
                    depmap[parent][fullpath] = dep;
                }
            }
            // ctx.meta = meta
            let ctxproc = {
                ...ctx,
                meta,
            };
            // let proc = processor[res.kind] || processor[NONE]
            let proc = getProcessor(res.kind, processor);
            proc(res, popts, rule, ctxproc, jsonic);
            // Handle the {@foo} case, injecting keys into parent map.
            if ('pair' === from) {
                if (ctx.cfg.map.merge) {
                    rule.parent.parent.node = ctx.cfg.map.merge(rule.parent.parent.node, res.val, rule, ctx);
                }
                else if (ctx.cfg.map.extend) {
                    rule.parent.parent.node = deep(rule.parent.parent.node, res.val);
                }
                else {
                    Object.assign(rule.parent.node, res.val);
                }
            }
            else {
                rule.node = res.val;
            }
            return undefined;
        },
        custom: (jsonic, { OPEN, name }) => {
            // Handle special case of @foo first token - assume a map
            jsonic.grammar({
                rule: {
                    val: {
                        open: [
                            {
                                s: [OPEN],
                                c: (r) => 0 < r.n.pk && 'pair' != r.parent.name,
                                b: 1,
                            },
                            {
                                s: [OPEN],
                                c: (r) => 0 === r.d,
                                p: 'map',
                                b: 1,
                                n: { [name + '_top']: 1 },
                            },
                        ],
                    },
                    map: {
                        open: [{
                                s: [OPEN],
                                c: (r) => 1 === r.d && 1 === r.n[name + '_top'],
                                p: 'pair',
                                b: 1,
                            }],
                        close: [{
                                s: [OPEN],
                                c: (r) => 0 < r.n.pk,
                                b: 1,
                            }],
                    },
                    pair: {
                        close: [{
                                s: [OPEN],
                                c: (r) => 0 < r.n.pk,
                                b: 1,
                            }],
                    },
                },
            }, {
                rule: { alt: { g: name } },
            });
        },
    };
    jsonic.use(directive_1.Directive, dopts);
};
exports.MultiSource = MultiSource;
// Convenience maker for Processors
function makeProcessor(process) {
    return (res) => (res.val = process(res.src, res));
}
// Default is just to insert file contents as a string.
const defaultProcessor = makeProcessor((src) => src);
const jsonicJsonParser = jsonic_1.Jsonic.make('json');
// TODO: use json plugin to get better error msgs.
const jsonProcessor = makeProcessor((src, res) => 
// null == src ? undefined : JSON.parse(src)
null == src ? undefined : jsonicJsonParser(src, { fileName: res.path }));
// let proc = processor[res.kind] || processor[NONE]
function getProcessor(kind, procmap) {
    let proc = procmap[NONE];
    let procref = procmap[kind];
    // Allow one level of aliasing
    if ('string' === typeof procref) {
        proc = procmap[procref];
    }
    else if ('function' === typeof procref) {
        proc = procref;
    }
    return proc;
}
const jsonicProcessor = (0, jsonic_2.makeJsonicProcessor)();
const jsProcessor = (0, js_1.makeJavaScriptProcessor)();
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
};
function resolvePathSpec(popts, ctx, spec, resolvefolder) {
    const fs = ctx.meta?.fs || SystemFs;
    let msmeta = ctx.meta?.multisource;
    let base = resolvefolder(null == msmeta || null == msmeta.path ? popts.path : msmeta.path, fs);
    let path = 'string' === typeof spec
        ? spec
        : null != spec.path
            ? '' + spec.path
            : undefined;
    let abs = !!(path?.startsWith('/') || path?.startsWith('\\'));
    let full = abs
        ? path
        : null != path && '' != path
            ? null != base && '' != base
                ? base + '/' + path
                : path
            : undefined;
    let kind = null == full ? NONE : (full.match(/\.([^.]*)$/) || [NONE, NONE])[1];
    let res = {
        kind,
        path,
        full,
        base,
        abs,
        found: false,
    };
    return res;
}
// Preload all files matching the given extensions from the specified folders
// into a flat map keyed by full resolved path.
function preloadFiles(opts, fs) {
    const _fs = fs || SystemFs;
    const Path = require('node:path');
    const ext = (opts.ext || ['.jsonic', '.json']).map(e => e.startsWith('.') ? e : '.' + e);
    const recursive = opts.recursive || false;
    const filemap = {};
    function scanFolder(folder) {
        let entries;
        try {
            entries = _fs.readdirSync(folder);
        }
        catch (_e) {
            return;
        }
        for (const name of entries) {
            const full = Path.resolve(folder, name);
            let stat;
            try {
                stat = _fs.statSync(full);
            }
            catch (_e) {
                continue;
            }
            if (stat.isDirectory()) {
                if (recursive)
                    scanFolder(full);
            }
            else if (stat.isFile()) {
                if (ext.some((e) => name.endsWith(e))) {
                    try {
                        filemap[full] = _fs.readFileSync(full).toString();
                    }
                    catch (_e) { /* skip unreadable */ }
                }
            }
        }
    }
    for (const folder of opts.folders) {
        scanFolder(Path.resolve(folder));
    }
    return filemap;
}
// Plugin meta data
const meta = {
    name: 'MultiSource',
};
exports.meta = meta;
//# sourceMappingURL=multisource.js.map