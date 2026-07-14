# Changelog

## [0.2.1](https://github.com/tabnas/multisource/compare/v0.2.0...v0.2.1) (2026-07-14)


### Bug Fixes

* **deps:** widen @tabnas/* plugin peer ranges to &gt;=0 ([df9ffdd](https://github.com/tabnas/multisource/commit/df9ffddaece86fcdc68724f9a44364e659c18027))
* **multisource:** add @tabnas/{directive,jsonic,path} as file: devDeps ([f4194a1](https://github.com/tabnas/multisource/commit/f4194a1a99a35e1f7d8f00dc4413fd01996dc898))
* **multisource:** port upstream resolver bug fixes (@jsonic/multisource v2.9.1) ([6ba5197](https://github.com/tabnas/multisource/commit/6ba51971ec493ef1572eb03b3a80f3ebf207d43a))
* **multisource:** port upstream resolver bug fixes from @jsonic/multisource v2.9.1 ([fd1667b](https://github.com/tabnas/multisource/commit/fd1667ba0953f944231219fb4c05d5655c171e00))
* **multisource:** seed the implicit map for directive-then-pair ([c79ffa0](https://github.com/tabnas/multisource/commit/c79ffa06687ef2ae08825af96ba2cd28f981f13f))
* widen plugin peer ranges to &gt;=0 + gate npm publish ([267bd90](https://github.com/tabnas/multisource/commit/267bd90f090829c86f8cbf2b8db400f5039fa8cc))


### Performance Improvements

* cache default Parse instance (Go) + reuse regression guards ([5bce292](https://github.com/tabnas/multisource/commit/5bce292be524b6621d853d6bd2174eb3df6b407c))
* **multisource:** cache the Parse() instance + machine-independent regression test ([b6324c2](https://github.com/tabnas/multisource/commit/b6324c2ce82764f55b1d6e09df548cd9a65f4f90))
