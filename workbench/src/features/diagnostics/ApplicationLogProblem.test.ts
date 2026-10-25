import {it,expect} from 'vitest';
import {applicationLogIssue} from './ApplicationLogProblem';
it('keeps browser and healthy log quiet, exposes unavailable and cumulative loss',()=>{
 const healthy={fileAvailable:true,directory:'/system',dropped:0,writeFailures:0};
 expect(applicationLogIssue(null)).toBeNull();expect(applicationLogIssue(healthy)).toBeNull();
 expect(applicationLogIssue({...healthy,fileAvailable:false,lastError:'disk full'})).toContain('不可用');
 expect(applicationLogIssue({...healthy,dropped:3})).toContain('丢弃 3');
 expect(applicationLogIssue({...healthy,writeFailures:2})).toContain('写入失败 2');
});
