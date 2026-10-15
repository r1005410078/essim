import { useEffect, useState, useMemo } from 'react';
import { NovaSheetGrid } from '@zhiguang/novasheet-react';
import { UnitSocSheets, draftSoc, socSheetTheme, socCellTypes } from './socSheet';
import { Play, Square, RefreshCw, Check, Plus, Cable } from 'lucide-react';
import { Button } from '@essim/ui/components/button';
import { Input } from '@essim/ui/components/input';
import { Tooltip, TooltipContent, TooltipTrigger } from '@essim/ui/components/tooltip';
import { RtuFields } from './RtuFields';
import { defaultRtuConfig, sameCommunication, validateRtuConfig } from './rtuConfig';
import { nextUnitId, parseNewUnitId } from './unitConfig';
import type { EssimClient } from '../../shared/api/client';
import { newerSnapshot, type Communication, type RtuConfig, type SimulatorSnapshot } from '../../shared/api/contracts';
const stateNames = { stopped: '已停止', starting: '启动中', listening: '监听中', stopping: '停止中', failed: '失败' };
function message(e: unknown) { return typeof e === 'object' && e !== null && 'message' in e ? String(e.message) : String(e); }
export function SimulatorPanel({ client }: { client: EssimClient }) {
  const [snapshot, setSnapshot] = useState<SimulatorSnapshot>();
  const [address, setAddress] = useState('127.0.0.1'); const [port, setPort] = useState('1502');
  const [transport,setTransport]=useState<'tcp'|'rtu'>('tcp');
  const [rtu,setRtu]=useState<RtuConfig>({...defaultRtuConfig});
  const [selectedTcpUnit,setSelectedUnit]=useState(1);
  const selectedUnit=transport==='tcp'?selectedTcpUnit:rtu.slave;
  const draftCommunication:Communication=transport==='tcp'?{transport,address,port:Number(port)}:{transport,config:rtu};
  const configured=sameCommunication(draftCommunication,snapshot?.communication);
  const [newUnit, setNewUnit] = useState('2');
  const sheets = useMemo(() => new UnitSocSheets(), []);
  const data = useMemo(() => sheets.get(selectedUnit,transport), [sheets, selectedUnit,transport]);
  const units = snapshot?.communication.transport===transport ? snapshot.units : [];
  const selected = configured ? units.find(unit => unit.unitId === selectedUnit) : undefined;
  const [busy, setBusy] = useState(false); const [error, setError] = useState('');
  useEffect(() => {
    let active = true; let cancel: (() => Promise<void>) | undefined;
    const receive = (next: SimulatorSnapshot) => { if (active) setSnapshot(old => newerSnapshot(old, next)); };
    client.subscribe(receive).then(async stop => { if (active) cancel = stop; else await stop(); }).catch(e => { if (active) setError(message(e)); });
    client.getSnapshot().then(next => { if (active) { receive(next); setAddress(next.address); setPort(String(next.port)); setTransport(next.communication.transport); setRtu(next.rtu ?? {...defaultRtuConfig}); } }).catch(e => { if (active) setError(message(e)); });
    return () => { active = false; if (cancel) void cancel().catch(console.error); };
  }, [client]);
  useEffect(() => {
    if (selected) sheets.sync(selected,transport);
  }, [selected, sheets,transport]);
  async function addUnit() {
    setBusy(true); setError('');
    try {
      const unitId = parseNewUnitId(newUnit, units.map(unit => unit.unitId));
      const next = await client.addUnit(unitId);
      setSnapshot(old => newerSnapshot(old, next));
      if (next.units.some(unit => unit.unitId === unitId)) setSelectedUnit(unitId);
      setNewUnit(String(nextUnitId(next.units.map(unit => unit.unitId)) ?? ''));
    } catch (e) { setError(message(e)); } finally { setBusy(false); }
  }
  async function run(operation: () => Promise<SimulatorSnapshot>) {
    setBusy(true); setError('');
    try { const next = await operation(); setSnapshot(old => newerSnapshot(old, next)); } catch (e) { setError(message(e)); } finally { setBusy(false); }
  }
  const running = snapshot?.state === 'listening' || snapshot?.state === 'starting' || snapshot?.state === 'stopping';
  function communication():Communication {
    if (transport==='rtu') return {transport,config:validateRtuConfig(rtu)};
    return {transport,address,port:Number(port)};
  }
  const tools = [
    { label: '启动从站', icon: Play, disabled: !snapshot || running, action: () => run(() => { const c=communication(); return c.transport==='rtu'?client.startRtu(c.config):client.start(c.address,c.port); }) },
    { label: '停止从站', icon: Square, disabled: !snapshot || snapshot.state==='stopped', action: () => run(() => client.stop()) },
    { label: '应用通信配置', icon: Cable, disabled: !snapshot || running, action: () => run(() => client.configure(communication())) },
    { label: '应用 SOC 运行值', icon: Check, disabled: !selected, action: () => run(async () => client.updateSoc(draftSoc(data), selectedUnit)) },
    { label: '刷新状态', icon: RefreshCw, disabled: false, action: () => run(() => client.getSnapshot()) },
  ];
  return <section className="flex h-full flex-col text-xs">
    <div className="flex h-8 shrink-0 items-center gap-1 border-b px-2">
      {tools.map(({ label, icon: Icon, disabled, action }) => <Tooltip key={label}><TooltipTrigger asChild><Button size="icon" variant="ghost" className="h-6 w-6" aria-label={label} disabled={busy || disabled} onClick={action}><Icon className="size-3.5" /></Button></TooltipTrigger><TooltipContent>{label}</TooltipContent></Tooltip>)}
      <span className="ml-2">{snapshot ? snapshot.state==='listening' && snapshot.communication.transport==='rtu'?'串口已打开':stateNames[snapshot.state] : '后端未连接'}</span><span className="ml-auto text-muted-foreground">{snapshot?.communication.transport==='rtu'?'RTU · 单串口单从站':`连接 ${snapshot?.connections ?? '—'} / 1`}</span>
    </div>
    <div className="flex flex-wrap items-center gap-2 border-b bg-muted/30 px-2 py-1">
      <label className="flex items-center gap-1">从站<select aria-label="通信方式" className="h-6 rounded-sm border border-input bg-background px-1 text-xs" value={transport} disabled={running || busy} onChange={e=>setTransport(e.target.value as 'tcp'|'rtu')}><option value="tcp">TCP</option><option value="rtu">RTU 串口</option></select></label>
      {transport==='rtu'?<RtuFields config={rtu} disabled={running || busy} onChange={setRtu}/>:<>
      <Input aria-label="监听 IPv4" className="h-6 w-32 rounded-sm px-1 text-xs" value={address} disabled={running || busy} onChange={e => setAddress(e.target.value)} />
      <span>:</span><Input aria-label="监听端口" className="h-6 w-16 rounded-sm px-1 text-xs" type="number" min="1" max="65535" value={port} disabled={running || busy} onChange={e => setPort(e.target.value)} />
      <label className="flex items-center gap-1">Unit ID
        <select aria-label="当前 Unit ID" className="h-6 min-w-14 rounded-sm border border-input bg-background px-1 text-xs" value={selectedUnit} disabled={busy || !snapshot} onChange={e => setSelectedUnit(Number(e.target.value))}>
          {units.length ? units.map(unit => <option key={unit.unitId} value={unit.unitId}>{unit.unitId}</option>) : <option value={1}>—</option>}
        </select>
      </label>
      <form className="flex items-center gap-1" onSubmit={e => { e.preventDefault(); void addUnit(); }}>
        <Input aria-label="新增 Unit ID" title="Unit ID 范围 1–247" className="h-6 w-16 rounded-sm px-1 text-xs" type="number" min="1" max="247" step="1" value={newUnit} disabled={busy || !snapshot || !configured || units.length >= 247} onChange={e => setNewUnit(e.target.value)} />
        <Tooltip><TooltipTrigger asChild><Button type="submit" size="icon" variant="ghost" className="h-6 w-6" aria-label="添加 Unit ID" disabled={busy || !snapshot || !configured || units.length >= 247}><Plus className="size-3.5" /></Button></TooltipTrigger><TooltipContent>添加 Unit ID</TooltipContent></Tooltip>
      </form>
      <span className="text-muted-foreground">{units.length} 个单元</span></>}
      {!configured && snapshot && <span className="text-muted-foreground">通信草稿未应用 · 点击启动或应用通信配置</span>}
    </div>
    {(error || snapshot?.error) && <p className="border-b px-2 py-1 text-destructive" role="alert">{error || snapshot?.error?.message}</p>}
    <div className="h-64 min-h-32 shrink-0 border-b">
      <NovaSheetGrid key={`${transport}:${selectedUnit}`} excelHeaders rowHeaderCornerLabel="序号" data={data} theme={socSheetTheme} className="h-full w-full" cellTypes={socCellTypes} interactions={{ contextMenu: false, reorder: false, resize: { rows: false, columns: true } }} />
    </div>
    <p className="px-2 py-2 text-muted-foreground">{transport==='rtu'?'RTU 站号':'Unit ID'} {selectedUnit} · holding · 教学映射：地址为零起始，外部只读。编辑草稿后按 Enter，再点击工具栏 ✓ 应用。初始值为 650。</p>
    <div className="mt-2 border-y bg-muted/30 px-2 py-1">通讯记录 · 最近 128 条</div>
    <div className="min-h-0 flex-1 overflow-auto font-mono">{snapshot?.logs.slice().reverse().map((log, i) => <div key={`${snapshot.sequence}-${i}`} className="border-b px-2 py-1"><span className="mr-2 text-muted-foreground">{log.direction}</span>{log.message}<div className="mt-0.5 select-text break-all text-muted-foreground">{log.hex}</div></div>)}</div>
    <div className="shrink-0 border-t px-2 py-1 text-muted-foreground">{transport==='tcp'?'单 TCP 端点 · 单客户端':'单串口 · 单从站 · 真实 RS-485 待硬件验证'} · 配置与运行值仅保留本次应用会话</div>
  </section>;
}
