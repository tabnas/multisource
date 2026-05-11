import { Resolver } from '../multisource';
type PathFinder = (spec: any) => string;
type FileResolverOptions = {
    pathfinder?: PathFinder;
    preload?: {
        [fullpath: string]: string;
    };
};
export declare function makeFileResolver(pathfinderOrOpts?: PathFinder | FileResolverOptions): Resolver;
export {};
