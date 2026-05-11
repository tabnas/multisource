/* Copyright (c) 2021-2023 Richard Rodger, MIT License */

import { Jsonic, Rule, Context } from 'jsonic'
import { MultiSourceOptions, Processor, Resolution } from '../multisource'


export function makeJsonicProcessor(): Processor {

  return function JsonicProcessor(
    res: Resolution,
    _popts: MultiSourceOptions,
    _rule: Rule,
    ctx: Context,
    jsonic: Jsonic
  ) {
    if (null != res.src && null != res.full) {
      res.val = jsonic(res.src, ctx.meta)
    }
  }
}
