import * as SystemFs from 'node:fs';
import { Jsonic, Context, Rule, Plugin } from 'jsonic';
type FST = typeof SystemFs;
interface MultiSourceMeta {
    path?: string;
    parents?: string[];
    deps?: DependencyMap;
}
declare const NONE = "";
type PreloadOptions = {
    folders: string[];
    ext?: string[];
    recursive?: boolean;
};
type MultiSourceOptions = {
    resolver: Resolver;
    path?: string;
    markchar?: string;
    processor?: {
        [kind: string]: Processor;
    };
    implictExt?: [];
    preload?: PreloadOptions;
};
type PathSpec = {
    kind: string;
    path?: string;
    full?: string;
    base?: string;
    abs: boolean;
};
type Resolution = PathSpec & {
    src?: string;
    val?: any;
    found: boolean;
    search?: string[];
};
type Resolver = (spec: PathSpec, popts: MultiSourceOptions, rule: Rule, ctx: Context, jsonic: Jsonic) => Resolution;
type Processor = (res: Resolution, popts: MultiSourceOptions, rule: Rule, ctx: Context, jsonic: Jsonic) => void;
type Dependency = {
    tar: string | typeof TOP;
    src: string;
    wen: number;
};
type DependencyMap = {
    [tar_full_path: string]: {
        [src_full_path: string]: Dependency;
    };
};
declare const TOP: unique symbol;
declare const MultiSource: Plugin;
declare function resolvePathSpec(popts: MultiSourceOptions, ctx: Context, spec: any, resolvefolder: (path: string, fs: FST) => string): PathSpec;
declare function preloadFiles(opts: PreloadOptions, fs?: FST): {
    [fullpath: string]: string;
};
declare const meta: {
    name: string;
};
export type { Resolver, Resolution, Processor, MultiSourceOptions, PreloadOptions, Dependency, DependencyMap, MultiSourceMeta, PathSpec, FST, };
export { MultiSource, resolvePathSpec, preloadFiles, NONE, TOP, meta };
