// Synthetic inputs only. Main owns the real Native/AppContainer broker.
import assert from 'node:assert/strict';
import {execFileSync} from 'node:child_process';
import {readFileSync, writeFileSync, mkdirSync} from 'node:fs';
import path from 'node:path';
import {fileURLToPath} from 'node:url';

const root=path.resolve(path.dirname(fileURLToPath(import.meta.url)),'..');
const [probe,worker,runtime,producer,consumer,output]=process.argv.slice(2);
assert(output,'Expected probe worker runtime producer consumer output paths');
const stage=path.resolve(output);mkdirSync(stage,{recursive:true});
const text=value=>({version:1,root:{type:'text',text:value}});
const input=value=>({in:{state:'data',packet:text(value)}});
const option=(value,score)=>({value:{type:'text',text:value},...(score===undefined?{}:{score})});
const choice=options=>({out:{state:'data',packet:{version:1,root:{type:'choice',options}}}});
const records=[];
function invoke(name,source,cases){
  const sourcePath=path.join(stage,`${name}.tsv`);
  writeFileSync(sourcePath,source);
  const spec={workspace:root,worker:path.resolve(worker),runtime:path.resolve(runtime),producer:path.resolve(producer),consumer:path.resolve(consumer),source:sourcePath,sourceReference:'dictionary.tsv',node:{kind:'text.dictionary-exact',version:1},config:{path:'dictionary.tsv'},cases};
  const manifest=path.join(stage,`${name}.json`);writeFileSync(manifest,JSON.stringify(spec));
  const raw=execFileSync(probe,[manifest],{encoding:'utf8',maxBuffer:16*1024*1024,timeout:180000});
  const result=JSON.parse(raw);assert.equal(result.syntheticScope,true);
  assert.equal(result.preparation.actualExitConfirmed,true);
  assert.equal(result.preparation.exitCode,0);
  assert.equal(result.preparation.inputDelivered,true);
  for(const item of result.cases){
    if(item.cancelledBeforeStart){assert.equal(item.processStarted,false);continue;}
    assert.equal(item.receipt.actualExitConfirmed,true);
    assert.equal(item.receipt.exitCode,0);assert.equal(item.receipt.reason,'Exited');
    assert.equal(item.receipt.inputDelivered,true);
  }
  writeFileSync(path.join(stage,`${name}-receipt.json`),raw);
  records.push({name,artifact:result.artifact,preparationError:result.preparationError,cases:result.cases.length,hashes:result.hashes});
  return result;
}
const fixture=JSON.parse(readFileSync(path.join(root,'tests/contracts/dictionary_exact.json'),'utf8'));
assert.equal(fixture.cases.length,13);
const cases=fixture.cases.map(item=>({inputs:{in:item.input.status==='no-output'?{state:'noOutput'}:{state:'data',packet:item.input.packet}},mode:'finalize'}));
cases.push({inputs:input('こうすい'),cancelBeforeStart:true});
const formal=invoke('formal',readFileSync(path.join(root,'examples/proposals/dictionary-exact/entries.tsv')),cases);
assert(formal.artifact);assert.equal(formal.preparationError,undefined);
for(const [index,item] of fixture.cases.entries()){
  const actual=formal.cases[index],expected=item.expected;
  if(expected.status==='error')assert.equal(actual.error,expected.code,item.id);
  else if(expected.status==='no-output')assert.deepEqual(actual.data,{out:{state:'noOutput'}},item.id);
  else assert.deepEqual(actual.data,{out:{state:'data',packet:expected.packet}},item.id);
}
assert.equal(formal.cases[13].cancelledBeforeStart,true);
const collisions=invoke('collision','reading\tsurface\tweight\nぃぁ\t一\t-0\nあだ\t二\t2\nぃぁ\t三\t-3\n',['ぃぁ','あだ','ぁみ'].map(query=>({inputs:input(query)})));
assert.deepEqual(collisions.cases.map(item=>item.data),[choice([option('一',-0),option('三',-3)]),choice([option('二',2)]),choice([option('ぁみ')])]);
const empty=invoke('empty','reading\tsurface\tweight',[{inputs:input('未知')},{inputs:{in:{state:'noOutput'}}}]);
assert.deepEqual(empty.cases.map(item=>item.data),[choice([option('未知')]),{out:{state:'noOutput'}}]);
for(const [name,source,code] of [
  ['bad-tail','reading\tsurface\tweight\na\tA\t1\nbad','dictionary-format'],
  ['duplicate','reading\tsurface\tweight\na\tA\t1\na\tA\t2','dictionary-format'],
  ['bad-utf8',Buffer.concat([Buffer.from('reading\tsurface\tweight\na\tA\t1\n'),Buffer.from([255])]),'dictionary-encoding']
]){
  const result=invoke(name,source,[{inputs:input('a')}]);
  assert.equal(result.preparationError,code);assert.equal(result.artifact,undefined);assert.equal(result.cases.length,0);
}
// Functional compatibility only. Large candidate performance is deferred.
const surfaces=Array.from({length:64},(_,i)=>'漢'.repeat(4093)+String(i).padStart(3,'0'));
const large=invoke('large-functional','reading\tsurface\tweight\n'+surfaces.map((value,i)=>`a\t${value}\t${i}\n`).join(''),[{inputs:input('a')}]);
assert.deepEqual(large.cases[0].data,choice(surfaces.map((value,i)=>option(value,i))));
writeFileSync(path.join(stage,'summary.json'),JSON.stringify({syntheticScope:true,performanceMeasured:false,records},null,2));
console.log('Actual dictionary Wasm: formal 13 cases, cancellation, collisions, empty source, three invalid sources and full multi-page candidates passed with true exits');
