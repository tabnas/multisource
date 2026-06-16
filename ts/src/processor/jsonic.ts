/* Copyright (c) 2021-2023 Richard Rodger, MIT License */

import { Tabnas, Rule, Context } from '@tabnas/parser'
import { MultiSourceOptions, Processor, Resolution } from '../multisource'


export function makeJsonicProcessor(): Processor {

  return function JsonicProcessor(
    res: Resolution,
    _popts: MultiSourceOptions,
    _rule: Rule,
    ctx: Context,
    tn: Tabnas
  ) {
    if (null != res.src && null != res.full) {
      res.val = tn.parse(res.src, ctx.meta)
    }
  }
}
