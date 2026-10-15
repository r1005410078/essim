import { Input } from '@essim/ui/components/input';
import type { RtuConfig } from '../../shared/api/contracts';
const selectClass='h-6 rounded-sm border border-input bg-background px-1 text-xs';
export function RtuFields({config,disabled,onChange}:{config:RtuConfig;disabled:boolean;onChange:(config:RtuConfig)=>void}) {
  const update=(patch:Partial<RtuConfig>)=>onChange({...config,...patch});
  return <>
    <label className="flex items-center gap-1">串口<Input aria-label="串口路径" className="h-6 w-48 rounded-sm px-1 text-xs" placeholder="/dev/cu.usbserial…" value={config.device} disabled={disabled} onChange={e=>update({device:e.target.value})}/></label>
    <label className="flex items-center gap-1">波特率<select aria-label="波特率" className={selectClass} value={config.baud} disabled={disabled} onChange={e=>update({baud:Number(e.target.value)})}>{[9600,19200,38400,57600,115200].map(v=><option key={v} value={v}>{v}</option>)}</select></label>
    <label className="flex items-center gap-1">校验<select aria-label="校验位" className={selectClass} value={config.parity} disabled={disabled} onChange={e=>update({parity:e.target.value as RtuConfig['parity']})}><option value="N">N 无</option><option value="E">E 偶</option><option value="O">O 奇</option></select></label>
    <label className="flex items-center gap-1">数据位<select aria-label="数据位" className={selectClass} value={8} disabled><option>8</option></select></label>
    <label className="flex items-center gap-1">停止位<select aria-label="停止位" className={selectClass} value={config.stopBits} disabled={disabled} onChange={e=>update({stopBits:Number(e.target.value) as 1|2})}><option value={1}>1</option><option value={2}>2</option></select></label>
    <label className="flex items-center gap-1">站号<Input aria-label="RTU 站号" type="number" min={1} max={247} step={1} className="h-6 w-14 rounded-sm px-1 text-xs" value={config.slave} disabled={disabled} onChange={e=>update({slave:Number(e.target.value)})}/></label>
    <label className="flex items-center gap-1">等待 ms<Input aria-label="接收超时毫秒" type="number" min={10} max={100} step={1} className="h-6 w-14 rounded-sm px-1 text-xs" value={config.receiveTimeoutMs} disabled={disabled} onChange={e=>update({receiveTimeoutMs:Number(e.target.value)})}/></label>
    <label className="flex items-center gap-1">字节间 ms<Input aria-label="字节间超时毫秒" type="number" min={1} max={2} step={1} className="h-6 w-12 rounded-sm px-1 text-xs" value={config.byteTimeoutMs} disabled={disabled} onChange={e=>update({byteTimeoutMs:Number(e.target.value)})}/></label>
  </>;
}
