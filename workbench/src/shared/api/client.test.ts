import { expect, it } from 'vitest';
import { createClient } from '../../app/bootstrap';
it('浏览器无后端时明确失败，不返回模拟连接成功', async()=>{await expect(createClient().getBackendInfo()).rejects.toMatchObject({code:'BACKEND_UNAVAILABLE'});});
