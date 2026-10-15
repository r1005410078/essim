import { pointCategory, isTelemetry, findPeerPoint, validateAction } from './telecontrol.js';
export function runtimeKey(device, comm, point) {
  return `${comm.role === '主站' ? 'comm:' + comm.id : 'dev:' + device.id}:${point.id}`;
}
export function prepareRuntimeBatch({device,comm,points,rows,operation,runtime,peer,peerPoints=[]}) {
  if (!rows.length) throw Error('请先选择有数据的单元格或区域');
  const updates = {};
  for (const row of rows) {
    const point = points.find(p => p.id === row.id);
    if (!point) throw Error('选区中存在无效数据点');
    const category = pointCategory(point);
    if (operation === 'read' && (comm.role !== '主站' || !isTelemetry(category))) throw Error('读取仅适用于主站遥测、遥信');
    if (operation === 'write' && (comm.role !== '主站' || isTelemetry(category) || point.access !== '读写')) throw Error('选区包含不可写控制点');
    if (operation === 'set' && comm.role !== '从站') throw Error('置数仅适用于从站');
    let value = operation === 'reset' ? point.value : row.value;
    let target;
    if (operation === 'read' || operation === 'write') {
      if (!peer) throw Error('目标地址没有匹配的本地从站');
      target = findPeerPoint(point,peerPoints);
      if (operation === 'write' && target.access !== '读写') throw Error('对端点不允许写入');
      if (operation === 'read') value = runtime[runtimeKey(peer.device,peer.comm,target)]?.value ?? target.value;
    }
    if (value === '' || value === null || value === undefined) throw Error('数值不能为空');
    const numeric = Number(value);
    const action = comm.role === '主站' ? (isTelemetry(category) ? '接收' : '发送') : (isTelemetry(category) ? '置数' : '接收');
    const error = validateAction(comm.role,category,action,numeric);
    if (error) throw Error(error);
    const state = {value:numeric,time:new Date().toLocaleTimeString('zh-CN',{hour12:false}),status:operation === 'write' ? '对端已接收（本地模拟，未验证动作完成）' : '已更新（本地模拟）'};
    updates[runtimeKey(device,comm,point)] = state;
    if (operation === 'write') updates[runtimeKey(peer.device,peer.comm,target)] = state;
  }
  return updates;
}
