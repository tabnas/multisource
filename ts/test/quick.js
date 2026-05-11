

// const JMPT = require('jsonic-multisource-pkg-test')
// console.log('JMPT', JMPT)

// const JMPTFOO = require('jsonic-multisource-pkg-test/foo')
// console.log('JMPTFOO', JMPTFOO)

// const JMPTBAR_path = require.resolve('jsonic-multisource-pkg-test/bar.txt')
// console.log('JMPTBAR_path', JMPTBAR_path)



const { Jsonic } = require('jsonic')
const { Debug } = require('jsonic/debug')
const { MultiSource } = require('..')
const { makeFileResolver } = require('../dist/resolver/file')
const { makeMemResolver } = require('../dist/resolver/mem')
const { makePkgResolver } = require('../dist/resolver/pkg')

console.log('MS', MultiSource, Debug)

const opts = {
  resolver:
  // makeMemResolver({'a.jsonic': 'a:1'}),
  makeFileResolver()
  // makePkgResolver({require:null})
}
const j = Jsonic.make()
      // .use(Debug,{trace:true})
      .use(MultiSource, opts)

// console.log(j('@t01.jsonic'))
console.log(j('@t02.jsonic'))
// console.log(j(`
// @t01.jsonic
// @t02.jsonic
// @e02.jsonic
// x:1
// `))

// console.log(j('@"./t01.jsonic"'))

// console.log(j('x:@a.jsonic'))
// // console.log(j('@a.jsonic'))
// // console.log(j('x:1,@a.jsonic',{log:-1}))
// // console.log(j('[x:1,@a.jsonic]',{log:-1}))
// // console.log(j('[x:1 @a.jsonic]',{log:-1}))
// // console.log(j('@a.jsonic y:2'))
// console.log(j('',{log:-1}))


// console.log(j('@"jsonic-multisource-pkg-test/zed"'))

// console.log(j(`
// a:1
// @"jsonic-multisource-pkg-test/zed"
// b:2
// `))
