"use strict";
/* Copyright (c) 2021-2023 Richard Rodger, MIT License */
Object.defineProperty(exports, "__esModule", { value: true });
exports.makeJsonicProcessor = makeJsonicProcessor;
function makeJsonicProcessor() {
    return function JsonicProcessor(res, _popts, _rule, ctx, jsonic) {
        if (null != res.src && null != res.full) {
            res.val = jsonic(res.src, ctx.meta);
        }
    };
}
//# sourceMappingURL=jsonic.js.map