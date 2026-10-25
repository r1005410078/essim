import {describe,it,expect,vi} from 'vitest';
import {createDiagnosticReporter, type DiagnosticRecord} from './diagnostics';
describe('diagnostic transport boundaries',()=>{
 it('deduplicates five seconds and limits twenty per second',()=>{
  let now=10000;const send=vi.fn(async(_record:DiagnosticRecord)=>{});const report=createDiagnosticReporter(send,()=>now);
  report('error','same');report('error','same');expect(send).toHaveBeenCalledTimes(1);
  for(let i=0;i<40;i++)report('error',String(i));expect(send).toHaveBeenCalledTimes(20);
  now+=6000;report('error','\0'.repeat(10000),'\0'.repeat(20000));
  expect(new TextEncoder().encode(JSON.stringify(send.mock.calls.at(-1)?.[0])).length).toBeLessThan(16384);
 });
 it('does not serialize arbitrary objects or recurse on transport failure',async()=>{
  const warning=vi.spyOn(console,'warn').mockImplementation(()=>{});
  const send=vi.fn(async(_record:DiagnosticRecord)=>{throw Error('offline');});const report=createDiagnosticReporter(send);
  report('a',{password:'secret'});report('b',Error('other'));await new Promise(r=>setTimeout(r,0));
  expect(send).toHaveBeenCalledTimes(2);expect(warning).toHaveBeenCalledTimes(1);
  expect(JSON.stringify(send.mock.calls)).not.toContain('secret');warning.mockRestore();
 });
});
