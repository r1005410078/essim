import test from 'node:test';
import assert from 'node:assert/strict';
import {prepareRuntimeBatch,runtimeKey} from './runtimeSheet.js';
const device={id:'d'}, comm={id:'c',role:'主站'};
const control={id:'p',category:'遥控',address:'200',access:'读写',value:0};
test('批量写入校验失败不改变原有运行状态',()=>{
 const runtime={};assert.throws(()=>prepareRuntimeBatch({device,comm,points:[control],rows:[{...control,value:2}],operation:'write',runtime,peer:{device,comm:{id:'s',role:'从站'}},peerPoints:[control]}),/仅允许/);assert.deepEqual(runtime,{});
});
test('主站写入需要明确可写点与本地对端，更新双方状态',()=>{
 const peer={device:{id:'other'},comm:{id:'s',role:'从站'}};
 assert.throws(()=>prepareRuntimeBatch({device,comm,points:[control],rows:[control],operation:'write',runtime:{}}),/本地从站/);
 const next=prepareRuntimeBatch({device,comm,points:[control],rows:[{...control,value:1}],operation:'write',runtime:{},peer,peerPoints:[control]});
 assert.equal(next[runtimeKey(device,comm,control)].value,1);assert.equal(next[runtimeKey(peer.device,peer.comm,control)].value,1);
});
