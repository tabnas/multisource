"use strict";
/* Copyright (c) 2021-2025 Richard Rodger and other contributors, MIT License */
var __importDefault = (this && this.__importDefault) || function (mod) {
    return (mod && mod.__esModule) ? mod : { "default": mod };
};
Object.defineProperty(exports, "__esModule", { value: true });
const node_test_1 = require("node:test");
const node_assert_1 = __importDefault(require("node:assert"));
const memfs_1 = require("memfs");
const jsonic_1 = require("jsonic");
const multisource_1 = require("../dist/multisource");
// import { makeJavaScriptProcessor } from '../dist/processor/js'
const mem_1 = require("../dist/resolver/mem");
const file_1 = require("../dist/resolver/file");
const pkg_1 = require("../dist/resolver/pkg");
const path_1 = require("@jsonic/path");
(0, node_test_1.describe)('multisource', () => {
    (0, node_test_1.test)('happy', () => {
        const o = {
            resolver: (0, mem_1.makeMemResolver)({
                'a.jsonic': 'a:1',
                'b.jsc': 'b:2',
                'c.txt': 'CCC',
                'd.json': '{"d":3}',
                // 'e.js': 'module.exports={e:4}',
                'f.jsc': 'f:5',
                'g/index.jsc': 'g:6',
                'h/index.h.jsc': 'h:7',
            }),
            // processor: {
            //   js: makeJavaScriptProcessor({ evalOnly: true }),
            // },
        };
        const j = jsonic_1.Jsonic.make().use(multisource_1.MultiSource, o);
        node_assert_1.default.deepEqual(j('a:@a.jsonic,x:1'), { a: { a: 1 }, x: 1 });
        node_assert_1.default.deepEqual(j('b:@b.jsc,x:1'), { b: { b: 2 }, x: 1 });
        node_assert_1.default.deepEqual(j('c:@c.txt,x:1'), { c: 'CCC', x: 1 });
        node_assert_1.default.deepEqual(j('d:@d.json,x:1'), { d: { d: 3 }, x: 1 });
        // assert.deepEqual(j('e:@e.js,x:1'), { e: { e: 4 }, x: 1 })
        node_assert_1.default.deepEqual(j('f:@f,x:1'), { f: { f: 5 }, x: 1 });
        node_assert_1.default.deepEqual(j('g:@g,x:1'), { g: { g: 6 }, x: 1 });
        node_assert_1.default.deepEqual(j('h:@h,x:1'), { h: { h: 7 }, x: 1 });
        node_assert_1.default.deepEqual(j(`
  x:a:@a.jsonic 
  x:b:@b.jsc 
  x:c:@c.txt 
  x:d:@d.json 
  // x:e:@e.js 
  y:1
  `), {
            x: {
                a: {
                    a: 1,
                },
                b: {
                    b: 2,
                },
                c: 'CCC',
                d: {
                    d: 3,
                },
                // e: {
                //   e: 4,
                // },
            },
            y: 1,
        });
    });
    (0, node_test_1.test)('pair-val', () => {
        const o = {
            resolver: (0, mem_1.makeMemResolver)({
                'a.jsonic': 'a:1',
            }),
        };
        const j = jsonic_1.Jsonic.make()
            // .use(Debug, { trace: true })
            .use(multisource_1.MultiSource, o);
        node_assert_1.default.deepEqual(j('{x:@a.jsonic}'), { x: { a: 1 } });
        node_assert_1.default.deepEqual(j('x:@a.jsonic'), { x: { a: 1 } });
        node_assert_1.default.deepEqual(j('{x:@a.jsonic y:1}'), { x: { a: 1 }, y: 1 });
        node_assert_1.default.deepEqual(j('x:@a.jsonic y:1'), { x: { a: 1 }, y: 1 });
        node_assert_1.default.deepEqual(j('{z:2 x:@a.jsonic y:1}'), { z: 2, x: { a: 1 }, y: 1 });
        node_assert_1.default.deepEqual(j('z:2 x:@a.jsonic y:1'), { z: 2, x: { a: 1 }, y: 1 });
        node_assert_1.default.deepEqual(j('{x:y:@a.jsonic}'), { x: { y: { a: 1 } } });
        node_assert_1.default.deepEqual(j('x:y:@a.jsonic'), { x: { y: { a: 1 } } });
        node_assert_1.default.deepEqual(j('{x:y:2 @a.jsonic}'), { x: { y: 2 }, a: 1 });
        node_assert_1.default.deepEqual(j('x:y:2 @a.jsonic'), { x: { y: 2 }, a: 1 });
        node_assert_1.default.deepEqual(j('x:2 @a.jsonic'), { x: 2, a: 1 });
    });
    (0, node_test_1.test)('implicit', () => {
        const o = {
            resolver: (0, mem_1.makeMemResolver)({
                'a.jsonic': 'a:1',
                'b.jsonic': 'a:{b:1,c:2}',
                'd.jsonic': 'd:3',
            }),
        };
        const j = jsonic_1.Jsonic.make().use(multisource_1.MultiSource, o);
        node_assert_1.default.deepEqual(j('a:@a.jsonic,x:1'), { a: { a: 1 }, x: 1 });
        node_assert_1.default.deepEqual(j('[@a.jsonic,{x:1}]'), [{ a: 1 }, { x: 1 }]);
        node_assert_1.default.deepEqual(j('@a.jsonic'), { a: 1 });
        node_assert_1.default.deepEqual(j('b:2 @a.jsonic'), { b: 2, a: 1 });
        node_assert_1.default.deepEqual(j('b:2 @a.jsonic c:3'), { b: 2, a: 1, c: 3 });
        node_assert_1.default.deepEqual(j('@a.jsonic b:2'), { a: 1, b: 2 });
        node_assert_1.default.deepEqual(j('y:@b.jsonic,x:1'), { y: { a: { b: 1, c: 2 } }, x: 1 });
        node_assert_1.default.deepEqual(j('@b.jsonic'), { a: { b: 1, c: 2 } });
        node_assert_1.default.deepEqual(j('x:2 @b.jsonic'), { x: 2, a: { b: 1, c: 2 } });
        node_assert_1.default.deepEqual(j('x:2 @b.jsonic y:3'), { x: 2, a: { b: 1, c: 2 }, y: 3 });
        node_assert_1.default.deepEqual(j('@b.jsonic y:2'), { a: { b: 1, c: 2 }, y: 2 });
        node_assert_1.default.deepEqual(j('a:{d:3} @b.jsonic'), { a: { b: 1, c: 2, d: 3 } });
        node_assert_1.default.deepEqual(j('a:{d:3} @b.jsonic y:2'), {
            a: { b: 1, c: 2, d: 3 },
            y: 2,
        });
        node_assert_1.default.deepEqual(j('a:{d:3} @b.jsonic a:{d:4,f:5}'), {
            a: { b: 1, c: 2, d: 4, f: 5 },
        });
        node_assert_1.default.deepEqual(j('@b.jsonic a:{d:4,f:5}'), {
            a: { b: 1, c: 2, d: 4, f: 5 },
        });
        node_assert_1.default.deepEqual(j('a:{d:3} @b.jsonic a:{d:4,f:5} z:1'), {
            a: { b: 1, c: 2, d: 4, f: 5 },
            z: 1,
        });
        node_assert_1.default.deepEqual(j('@b.jsonic a:{d:4,f:5} z:1'), {
            a: { b: 1, c: 2, d: 4, f: 5 },
            z: 1,
        });
        node_assert_1.default.deepEqual(j('@a.jsonic @d.jsonic'), {
            a: 1,
            d: 3,
        });
        node_assert_1.default.deepEqual(j('x:11 @a.jsonic @d.jsonic'), {
            x: 11,
            a: 1,
            d: 3,
        });
        node_assert_1.default.deepEqual(j('@a.jsonic x:11 @d.jsonic'), {
            x: 11,
            a: 1,
            d: 3,
        });
        node_assert_1.default.deepEqual(j('x:{} @a.jsonic @d.jsonic'), {
            x: {},
            a: 1,
            d: 3,
        });
        node_assert_1.default.deepEqual(j('x:y:{} @a.jsonic @d.jsonic'), {
            x: { y: {} },
            a: 1,
            d: 3,
        });
    });
    (0, node_test_1.test)('deps', () => {
        const o = {
            resolver: (0, mem_1.makeMemResolver)({
                'a.jsc': 'a:1,b:@b.jsc,x:99',
                'b.jsc': 'b:2,c:@c',
                'c/index.jsc': 'c:3',
            }),
        };
        const j = jsonic_1.Jsonic.make().use(multisource_1.MultiSource, o);
        node_assert_1.default.deepEqual(j('@a'), { a: 1, b: { b: 2, c: { c: 3 } }, x: 99 });
        node_assert_1.default.deepEqual(j('@a', {}), { a: 1, b: { b: 2, c: { c: 3 } }, x: 99 });
        node_assert_1.default.deepEqual(j('@a', { x: 1 }), { a: 1, b: { b: 2, c: { c: 3 } }, x: 99 });
        node_assert_1.default.deepEqual(j('@a', { multisource: { path: undefined } }), {
            a: 1,
            b: { b: 2, c: { c: 3 } },
            x: 99,
        });
    });
    (0, node_test_1.test)('error-basic', () => {
        const o = {
            resolver: (0, mem_1.makeMemResolver)({}),
        };
        const j = jsonic_1.Jsonic.make().use(multisource_1.MultiSource, o);
        // j('x:@a')
        node_assert_1.default.throws(() => j('x:@a'), /multisource_not_found.*:1:3/s);
        node_assert_1.default.throws(() => j('x:@a', { fileName: 'foo' }), /foo:1:3/s);
    });
    (0, node_test_1.test)('error-file', () => {
        const o = {
            resolver: (0, file_1.makeFileResolver)(),
        };
        const j = jsonic_1.Jsonic.make().use(multisource_1.MultiSource, o);
        node_assert_1.default.throws(() => j('@../test/e02.jsonic', { multisource: { path: __dirname } }), /e02\.jsonic:2:3/);
        let deps = {};
        try {
            j('@../test/e01.jsonic', { multisource: { path: __dirname, deps } });
        }
        catch (e) {
            // console.log(e)
            // console.dir(e.meta.multisource, { depth: null })
            node_assert_1.default.match(e.message, /e02\.jsonic:2:3/);
            node_assert_1.default.match(e.meta.multisource.path, /e02\.jsonic/);
            node_assert_1.default.match(e.meta.multisource.parents[1], /e01\.jsonic/);
        }
    });
    (0, node_test_1.test)('basic-file', () => {
        let j0 = jsonic_1.Jsonic.make().use(multisource_1.MultiSource, {
            resolver: (0, file_1.makeFileResolver)(),
        });
        let deps = {};
        node_assert_1.default.deepEqual(j0('a:1,b:@"../test/t01.jsonic"', { multisource: { path: __dirname, deps } }), { a: 1, b: { c: 2 } });
        // console.dir(deps, { depth: null })
        node_assert_1.default.deepEqual(j0('a:1,b:@"../test/t01.jsonic"', { multisource: { path: __dirname } }), { a: 1, b: { c: 2 } });
        node_assert_1.default.deepEqual(j0('@"../test/t01.jsonic"', { multisource: { path: __dirname } }), { c: 2 });
        node_assert_1.default.deepEqual(j0('a:1,@"../test/t01.jsonic"', { multisource: { path: __dirname } }), { a: 1, c: 2 });
        node_assert_1.default.deepEqual(j0('@"../test/t01.jsonic",a:1', { multisource: { path: __dirname } }), { a: 1, c: 2 });
        node_assert_1.default.deepEqual(j0('a:1,@"../test/t01.jsonic",b:2', { multisource: { path: __dirname } }), { a: 1, c: 2, b: 2 });
        node_assert_1.default.deepEqual(j0('a:1,@"../test/t01.jsonic",b:2,@"../test/t01.jsonic",', { multisource: { path: __dirname } }), { a: 1, c: 2, b: 2 });
        node_assert_1.default.throws(() => j0('a:1,b:@"../test/t01.jsonic"', { multisource: {} }), /not found/);
        node_assert_1.default.throws(() => j0('a:1,b:@"../test/t01.jsonic"', {}), /not found/);
        node_assert_1.default.throws(() => j0('a:1,b:@"../test/t01.jsonic"'), /not found/);
        deps = {};
        node_assert_1.default.deepEqual(j0('a:1,b:@"../test/t02.jsonic",c:3', {
            multisource: { path: __dirname, deps },
        }), { a: 1, b: { d: 2, e: { f: 4 }, g: 9 }, c: 3 });
    });
    (0, node_test_1.test)('file-kind', () => {
        let j0 = jsonic_1.Jsonic.make().use(multisource_1.MultiSource, {
            resolver: (0, file_1.makeFileResolver)(),
        });
        let deps = {};
        node_assert_1.default.deepEqual(j0('a:1,b:@"../test/k01.jsonic"', { multisource: { path: __dirname, deps } }), { a: 1, b: { c: 2 } });
        // console.dir(deps, { depth: null })
        deps = {};
        node_assert_1.default.deepEqual(j0('a:1,d:@"../test/k02.js"', { multisource: { path: __dirname, deps } }), { a: 1, d: { e: 3 } });
        deps = {};
        node_assert_1.default.deepEqual(j0('a:1,f:@"../test/k03.json"', { multisource: { path: __dirname, deps } }), { a: 1, f: { g: 4 } });
        deps = {};
        node_assert_1.default.deepEqual(j0('a:1,b:@"../test/k01.jsonic",d:@"../test/k02.js",f:@"../test/k03.json"', {
            multisource: { path: __dirname, deps },
        }), { a: 1, b: { c: 2 }, d: { e: 3 }, f: { g: 4 } });
        deps = {};
        node_assert_1.default.deepEqual(j0('@"../test/k04.jsc"', { multisource: { path: __dirname, deps } }), { a: 1, b: { c: 2 }, d: { e: 3 }, f: { g: 4 } });
    });
    (0, node_test_1.test)('custom-ext', () => {
        let j0 = jsonic_1.Jsonic.make().use(multisource_1.MultiSource, {
            resolver: (0, file_1.makeFileResolver)(),
            processor: {
                foo: 'jsonic'
            }
        });
        let deps = {};
        node_assert_1.default.deepEqual(j0('@"../test/t04.foo"', { multisource: { path: __dirname, deps } }), { a: 1 });
    });
    (0, node_test_1.test)('path', () => {
        const o = {
            resolver: (0, mem_1.makeMemResolver)({
                'x.jsonic': 'x:y:1',
            }),
            // processor: {
            //   js: makeJavaScriptProcessor({ evalOnly: true }),
            // },
        };
        const j = jsonic_1.Jsonic.make()
            .use(multisource_1.MultiSource, o)
            .use(path_1.Path)
            .use((jsonic) => {
            jsonic.rule('val', (rs) => {
                rs.ac(false, (r) => {
                    if ('object' === typeof r.node) {
                        r.node.$ = `${r.k.path}`;
                    }
                });
            });
        });
        node_assert_1.default.deepEqual(j('a:b:@"x.jsonic"'), {
            $: '',
            a: {
                $: 'a',
                b: {
                    $: 'a,b',
                    x: {
                        $: 'a,b,x',
                        y: 1,
                    },
                },
            },
        });
    });
    (0, node_test_1.test)('memfs', () => {
        const j0 = jsonic_1.Jsonic.make()
            .use(multisource_1.MultiSource, {
            resolver: (0, file_1.makeFileResolver)()
        });
        const { fs, vol } = (0, memfs_1.memfs)({
            'b.jsonic': '2',
            node_modules: {
                foo: {
                    'c.jsonic': '3'
                }
            }
        });
        //      ; (fs as any).ISMEM = true
        node_assert_1.default.deepEqual(j0('a:1 b:@"/b.jsonic"', { fs }), {
            a: 1, b: 2
        });
        node_assert_1.default.deepEqual(j0('a:1 b:@"b.jsonic"', { fs, multisource: { path: '/' } }), {
            a: 1, b: 2
        });
        const j1 = jsonic_1.Jsonic.make()
            .use(multisource_1.MultiSource, {
            resolver: (0, pkg_1.makePkgResolver)({ require })
        });
        node_assert_1.default.deepEqual(j1('a:1 c:@"jsonic-multisource-pkg-test/zed.jsonic"', { fs, multisource: { path: '/' } }), {
            a: 1, c: { zed: 99 }
        });
        // TODO: implement require over virtual fs
        // assert.deepEqual(j1('a:1 c:@"foo/c.jsonic"', { fs, multisource: { path: '/' } }), {
        //   a: 1, c: 3
        // })
    });
    (0, node_test_1.test)('pkg-require-array', () => {
        const j1 = jsonic_1.Jsonic.make()
            .use(multisource_1.MultiSource, {
            resolver: (0, pkg_1.makePkgResolver)({
                require: [__dirname + '/..']
            })
        });
        node_assert_1.default.deepEqual(j1('a:1 c:@"jsonic-multisource-pkg-test/zed.jsonic"', { multisource: { path: '/' } }), { a: 1, c: { zed: 99 } });
    });
    (0, node_test_1.test)('pkg-require-string', () => {
        const j1 = jsonic_1.Jsonic.make()
            .use(multisource_1.MultiSource, {
            resolver: (0, pkg_1.makePkgResolver)({
                require: __dirname + '/..'
            })
        });
        node_assert_1.default.deepEqual(j1('a:1 c:@"jsonic-multisource-pkg-test/zed.jsonic"', { multisource: { path: '/' } }), { a: 1, c: { zed: 99 } });
    });
    (0, node_test_1.test)('pkg-virtual-fs-fallback', () => {
        const { fs } = (0, memfs_1.memfs)({
            'data.jsonic': 'data:42',
        });
        const j1 = jsonic_1.Jsonic.make()
            .use(multisource_1.MultiSource, {
            resolver: (0, pkg_1.makePkgResolver)({ require })
        });
        node_assert_1.default.deepEqual(j1('a:1 d:@"/data.jsonic"', { fs, multisource: { path: '/' } }), { a: 1, d: { data: 42 } });
    });
    (0, node_test_1.test)('pkg-no-path', () => {
        const j1 = jsonic_1.Jsonic.make()
            .use(multisource_1.MultiSource, {
            resolver: (0, pkg_1.makePkgResolver)({ require })
        });
        node_assert_1.default.deepEqual(j1('z:@"jsonic-multisource-pkg-test"'), { z: 11 });
    });
    (0, node_test_1.test)('pkg-resolvefolder-file', () => {
        const j1 = jsonic_1.Jsonic.make()
            .use(multisource_1.MultiSource, {
            resolver: (0, pkg_1.makePkgResolver)({ require })
        });
        // multisource path is a file, not a directory - tests resolvefolder isFile branch
        const filePath = __dirname + '/../package.json';
        node_assert_1.default.deepEqual(j1('z:@"jsonic-multisource-pkg-test/zed.jsonic"', { multisource: { path: filePath } }), { z: { zed: 99 } });
    });
    (0, node_test_1.test)('pkg-fs-error', () => {
        const brokenFs = {
            existsSync: () => { throw new Error('broken'); },
            readFileSync: () => Buffer.from(''),
            statSync: () => ({ isFile: () => false }),
        };
        const j1 = jsonic_1.Jsonic.make()
            .use(multisource_1.MultiSource, {
            resolver: (0, pkg_1.makePkgResolver)({ require })
        });
        node_assert_1.default.throws(() => j1('x:@"/nonexistent.jsonic"', { fs: brokenFs, multisource: { path: '/' } }), /not_found/);
    });
    (0, node_test_1.test)('pkg-load-failure', () => {
        const errorFs = {
            existsSync: (p) => p.endsWith('/data.jsonic'),
            readFileSync: () => { throw new Error('read error'); },
            statSync: () => ({ isFile: () => false }),
        };
        const j1 = jsonic_1.Jsonic.make()
            .use(multisource_1.MultiSource, {
            resolver: (0, pkg_1.makePkgResolver)({ require })
        });
        // existsSync returns true but readFileSync throws - covers load catch
        node_assert_1.default.throws(() => j1('x:@"/data.jsonic"', { fs: errorFs, multisource: { path: '/' } }), /not_found/);
    });
    (0, node_test_1.test)('pkg-node-modules-walk', () => {
        const j1 = jsonic_1.Jsonic.make()
            .use(multisource_1.MultiSource, {
            resolver: (0, pkg_1.makePkgResolver)({
                require: ['/nonexistent']
            })
        });
        // Initial require.resolve fails with bad paths,
        // then node_modules walk (no virtual fs) finds the package
        node_assert_1.default.deepEqual(j1('z:@"jsonic-multisource-pkg-test/zed.jsonic"', { multisource: { path: process.cwd() } }), { z: { zed: 99 } });
    });
    (0, node_test_1.test)('file-implicit', () => {
        let j0 = jsonic_1.Jsonic.make().use(multisource_1.MultiSource, {
            resolver: (0, file_1.makeFileResolver)(),
        });
        // File without extension - found via implicit extension and potentials loop
        node_assert_1.default.deepEqual(j0('a:1,b:@"t01"', { multisource: { path: process.cwd() + '/test' } }), { a: 1, b: { c: 2 } });
    });
    (0, node_test_1.test)('file-pathfinder', () => {
        let j0 = jsonic_1.Jsonic.make().use(multisource_1.MultiSource, {
            resolver: (0, file_1.makeFileResolver)((spec) => {
                return '../test/' + spec;
            }),
        });
        node_assert_1.default.deepEqual(j0('b:@"t01.jsonic"', { multisource: { path: __dirname } }), { b: { c: 2 } });
    });
    (0, node_test_1.test)('spec-object', () => {
        const o = {
            resolver: (0, mem_1.makeMemResolver)({
                'a.jsonic': 'a:1',
            }),
        };
        const j = jsonic_1.Jsonic.make().use(multisource_1.MultiSource, o);
        // spec as object with path property - covers resolvePathSpec spec.path branch
        node_assert_1.default.deepEqual(j('x:@{path:"a.jsonic"}'), { x: { a: 1 } });
    });
    (0, node_test_1.test)('merge', () => {
        const o = {
            resolver: (0, mem_1.makeMemResolver)({
                'a.jsonic': 'a:1',
            }),
        };
        const j = jsonic_1.Jsonic.make()
            .use(multisource_1.MultiSource, o);
        j.options({
            map: {
                merge: (prev, curr) => Object.assign({}, prev, curr)
            }
        });
        node_assert_1.default.deepEqual(j('x:2 @a.jsonic'), { x: 2, a: 1 });
    });
    (0, node_test_1.test)('assign', () => {
        const o = {
            resolver: (0, mem_1.makeMemResolver)({
                'a.jsonic': 'a:1',
            }),
        };
        const j = jsonic_1.Jsonic.make()
            .use(multisource_1.MultiSource, o);
        j.options({
            map: {
                extend: false
            }
        });
        node_assert_1.default.deepEqual(j('x:2 @a.jsonic'), { x: 2, a: 1 });
    });
    (0, node_test_1.test)('js-default-export', () => {
        let j0 = jsonic_1.Jsonic.make().use(multisource_1.MultiSource, {
            resolver: (0, file_1.makeFileResolver)(),
        });
        // JS module with exports.default - tests the out.default branch in js.ts
        let deps = {};
        node_assert_1.default.deepEqual(j0('a:1,d:@"../test/k05.js"', { multisource: { path: __dirname, deps } }), { a: 1, d: { f: 5 } });
    });
    (0, node_test_1.test)('jsonic-null-src', () => {
        const o = {
            resolver: (_spec, _popts, _rule, _ctx) => ({
                kind: 'jsonic',
                abs: false,
                found: true,
                src: undefined,
                full: undefined,
            }),
        };
        const j = jsonic_1.Jsonic.make().use(multisource_1.MultiSource, o);
        // Covers the null src/full guard in jsonic processor
        node_assert_1.default.deepEqual(j('x:@"foo"'), { x: null });
    });
    (0, node_test_1.test)('preload-basic', () => {
        const Fs = require('node:fs');
        const Path = require('node:path');
        // Use the existing test fixtures
        const testDir = Path.resolve(__dirname, '..', 'test');
        const filemap = (0, multisource_1.preloadFiles)({
            folders: [testDir],
            ext: ['.jsonic'],
        });
        // Should have loaded the test .jsonic files
        const keys = Object.keys(filemap);
        node_assert_1.default.ok(keys.length > 0, 'preloadFiles should find .jsonic files');
        // Check a known file is loaded
        const t01Path = Path.resolve(testDir, 't01.jsonic');
        node_assert_1.default.ok(filemap[t01Path], 't01.jsonic should be preloaded');
        node_assert_1.default.strictEqual(filemap[t01Path], Fs.readFileSync(t01Path).toString());
    });
    (0, node_test_1.test)('preload-extensions', () => {
        const Path = require('node:path');
        const testDir = Path.resolve(__dirname, '..', 'test');
        // Default extensions: .jsonic, .json
        const defaultMap = (0, multisource_1.preloadFiles)({ folders: [testDir] });
        const defaultKeys = Object.keys(defaultMap);
        node_assert_1.default.ok(defaultKeys.some(k => k.endsWith('.jsonic')));
        node_assert_1.default.ok(defaultKeys.some(k => k.endsWith('.json')));
        node_assert_1.default.ok(!defaultKeys.some(k => k.endsWith('.js')));
        // Custom extensions
        const jsMap = (0, multisource_1.preloadFiles)({ folders: [testDir], ext: ['.js'] });
        const jsKeys = Object.keys(jsMap);
        node_assert_1.default.ok(jsKeys.some(k => k.endsWith('.js')));
        node_assert_1.default.ok(!jsKeys.some(k => k.endsWith('.jsonic')));
    });
    (0, node_test_1.test)('preload-recursive', () => {
        const Path = require('node:path');
        const testDir = Path.resolve(__dirname, '..', 'test');
        // Non-recursive (default): should not find files in f01/
        const flatMap = (0, multisource_1.preloadFiles)({ folders: [testDir], ext: ['.jsonic'] });
        const flatKeys = Object.keys(flatMap);
        node_assert_1.default.ok(!flatKeys.some(k => k.includes('f01')), 'non-recursive should not descend into f01/');
        // Recursive: should find files in f01/
        const deepMap = (0, multisource_1.preloadFiles)({
            folders: [testDir],
            ext: ['.jsonic'],
            recursive: true,
        });
        const deepKeys = Object.keys(deepMap);
        node_assert_1.default.ok(deepKeys.some(k => k.includes('f01')), 'recursive should find files in f01/');
    });
    (0, node_test_1.test)('preload-file-resolver', () => {
        const Path = require('node:path');
        const testDir = Path.resolve(__dirname, '..', 'test');
        // Preload all .jsonic files
        const filemap = (0, multisource_1.preloadFiles)({
            folders: [testDir],
            ext: ['.jsonic'],
            recursive: true,
        });
        // Create a file resolver with preloaded files
        const o = {
            resolver: (0, file_1.makeFileResolver)({ preload: filemap }),
            path: testDir,
        };
        const j = jsonic_1.Jsonic.make().use(multisource_1.MultiSource, o);
        const result = j('@"t01.jsonic"');
        node_assert_1.default.deepEqual(result, { c: 2 });
    });
    (0, node_test_1.test)('preload-multiple-folders', () => {
        const Path = require('node:path');
        const testDir = Path.resolve(__dirname, '..', 'test');
        const f01Dir = Path.resolve(testDir, 'f01');
        // Scan root (non-recursive) and f01 separately
        const rootOnly = (0, multisource_1.preloadFiles)({ folders: [testDir], ext: ['.jsonic'] });
        const f01Only = (0, multisource_1.preloadFiles)({ folders: [f01Dir], ext: ['.jsonic'] });
        node_assert_1.default.ok(Object.keys(rootOnly).length > 0, 'should have files from test root');
        node_assert_1.default.ok(Object.keys(f01Only).length > 0, 'should have files from f01/');
        // Combined scan should have files from both
        const combined = (0, multisource_1.preloadFiles)({
            folders: [testDir, f01Dir],
            ext: ['.jsonic'],
        });
        const combinedKeys = Object.keys(combined);
        node_assert_1.default.ok(combinedKeys.length >= Object.keys(rootOnly).length);
        node_assert_1.default.ok(combinedKeys.length >= Object.keys(f01Only).length);
    });
    (0, node_test_1.test)('preload-missing-folder', () => {
        // Should not throw for non-existent folders
        const filemap = (0, multisource_1.preloadFiles)({
            folders: ['/nonexistent/folder/path'],
        });
        node_assert_1.default.deepEqual(filemap, {});
    });
});
//# sourceMappingURL=multisource.test.js.map