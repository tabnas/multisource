"use strict";
/* Copyright (c) 2021-2025 Richard Rodger and other contributors, MIT License */
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
exports.makeFileResolver = makeFileResolver;
const SystemFs = __importStar(require("node:fs"));
const Path = __importStar(require("node:path"));
const multisource_1 = require("../multisource");
const mem_1 = require("./mem");
function makeFileResolver(pathfinderOrOpts) {
    let pathfinder;
    let preload;
    if ('function' === typeof pathfinderOrOpts) {
        pathfinder = pathfinderOrOpts;
    }
    else if (null != pathfinderOrOpts) {
        pathfinder = pathfinderOrOpts.pathfinder;
        preload = pathfinderOrOpts.preload;
    }
    return function FileResolver(spec, popts, _rule, ctx) {
        const fs = ctx.meta?.fs || SystemFs;
        const foundSpec = pathfinder ? pathfinder(spec) : spec;
        const ps = (0, multisource_1.resolvePathSpec)(popts, ctx, foundSpec, resolvefolder);
        let src = undefined;
        let search = [];
        if (null != ps.full) {
            ps.full = Path.resolve(ps.full);
            search.push(ps.full);
            // Check preloaded files first, then fall back to disk.
            src = preload?.[ps.full] ?? load(ps.full, fs);
            if (null == src) {
                const potentials = [];
                // Special case: support npm linked references
                if (null != ps.base && null != ps.path) {
                    let base = ps.base;
                    let last;
                    for (let i = 0; i < 7; i++) { // Heuristically check 7 levels of folders
                        potentials.push(Path.resolve(base, 'node_modules', ps.path));
                        base = Path.dirname(base);
                        if (last === base)
                            break;
                        last = base;
                    }
                }
                if (multisource_1.NONE === ps.kind) {
                    potentials.push(...(0, mem_1.buildPotentials)(ps, popts, (...s) => Path.resolve(s.reduce((a, p) => Path.join(a, p)))));
                }
                search.push(...potentials);
                for (let path of potentials) {
                    src = preload?.[path] ?? load(path, fs);
                    if (null != src) {
                        ps.full = path;
                        ps.kind = (path.match(/\.([^.]*)$/) || [multisource_1.NONE, multisource_1.NONE])[1];
                        break;
                    }
                }
            }
        }
        let res = {
            ...ps,
            src,
            found: null != src,
            search,
        };
        return res;
    };
}
function resolvefolder(path, fs) {
    if ('string' !== typeof path) {
        return path;
    }
    let folder = path;
    let pathstats = fs.statSync(path);
    if (pathstats.isFile()) {
        let pathdesc = Path.parse(path);
        folder = pathdesc.dir;
    }
    return folder;
}
function load(path, fs) {
    try {
        return fs.readFileSync(path).toString();
    }
    catch (e) {
        // NOTE: don't need this, as in all cases, we consider failed
        // reads to indicate non-existence.
    }
}
//# sourceMappingURL=file.js.map