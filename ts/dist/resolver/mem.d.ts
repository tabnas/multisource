import { MultiSourceOptions, Resolver, PathSpec } from '../multisource';
export declare function makeMemResolver(filemap: {
    [fullpath: string]: string;
}): Resolver;
export declare function buildPotentials(ps: PathSpec, popts: MultiSourceOptions, pathjoin: (...parts: string[]) => string): string[];
