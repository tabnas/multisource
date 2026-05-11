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
exports.makePkgResolver = makePkgResolver;
const SystemFs = __importStar(require("node:fs"));
const Path = __importStar(require("node:path"));
const multisource_1 = require("../multisource");
const mem_1 = require("./mem");
function makePkgResolver(options) {
    let useRequire = require;
    let requireOptions = undefined;
    if ('function' === typeof options.require) {
        useRequire = options.require;
    }
    else if (Array.isArray(options.require)) {
        requireOptions = {
            paths: options.require
        };
    }
    else if ('string' === typeof options.require) {
        requireOptions = {
            paths: [options.require]
        };
    }
    return function PkgResolver(spec, popts, _rule, ctx) {
        let fs = ctx.meta?.fs || SystemFs;
        // TODO: support pathfinder as file.ts
        let foundSpec = spec;
        let ps = (0, multisource_1.resolvePathSpec)(popts, ctx, foundSpec, resolvefolder);
        let src = undefined;
        let search = [];
        if (null != ps.path) {
            try {
                ps.full = useRequire.resolve(ps.path, requireOptions);
                if (null != ps.full) {
                    src = load(ps.full, SystemFs);
                    ps.kind = (ps.full.match(/\.([^.]*)$/) || [multisource_1.NONE, multisource_1.NONE])[1];
                }
            }
            catch (me) {
                search.push(ps.path);
                let potentials = [];
                if (null == ctx.meta?.fs) {
                    let localpath = Path.join(process.cwd(), 'NIL');
                    let localparts;
                    do {
                        localparts = Path.parse(localpath);
                        localpath = localparts.dir;
                        potentials.push(Path.join(localpath, 'node_modules', ps.path));
                    } while (localparts.root !== localparts.dir);
                }
                else {
                    potentials.push(ps.path);
                }
                if (null != ps.path && 'string' === typeof ps.path) {
                    const pspath = ps.path;
                    // Add the main paths of the current require
                    potentials.push(...useRequire.main.paths.map((p) => Path.join(p, pspath)));
                    // Remove module name prefix
                    const subpath = ps.path.replace(/^(@[^/]+\/)?[^/]+\//, '');
                    potentials.push(...useRequire.main.paths
                        .map((p) => p.replace(/node_modules$/, subpath)));
                }
                potentials.push(...(0, mem_1.buildPotentials)(ps, popts, (...s) => Path.resolve(s.reduce((a, p) => Path.join(a, p)))));
                // Check longest paths first
                potentials.sort((a, b) => b.length - a.length);
                requireOptions = { paths: ['/'] };
                for (let path of potentials) {
                    try {
                        ps.full = useRequire.resolve(path, requireOptions);
                        if (null != ps.full) {
                            src = load(ps.full, SystemFs);
                            ps.kind = (ps.full.match(/\.([^.]*)$/) || [multisource_1.NONE, multisource_1.NONE])[1];
                            break;
                        }
                    }
                    catch (me) {
                        // require.resolve failed — try the filesystem directly.
                        // .jsonic files are text, not JS modules, so require.resolve
                        // isn't needed; and it can't see virtual filesystems at all.
                        try {
                            if (fs.existsSync(path)) {
                                ps.full = path;
                                src = load(ps.full, fs);
                                if (null != src) {
                                    ps.kind = (path.match(/\.([^.]*)$/) || [multisource_1.NONE, multisource_1.NONE])[1];
                                    break;
                                }
                            }
                        }
                        catch (_e) { /* fall through */ }
                        search.push(path);
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
//# sourceMappingURL=pkg.js.map