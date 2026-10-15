import { useMemo } from 'react';
import { TooltipProvider } from '@essim/ui/components/tooltip';
import { DockviewReact, themeLight, type DockviewReadyEvent } from 'dockview-react';
import 'dockview-react/dist/styles/dockview.css';
import { createClient } from './bootstrap';
import { SimulatorPanel } from '../features/modbus/SimulatorPanel';
function ProjectPanel() { return <div className="p-3 text-xs text-muted-foreground">尚无项目。正式项目与点表功能待接入。</div>; }
function InspectorPanel() { return <div className="p-3 text-xs text-muted-foreground">选择项目文件查看属性。</div>; }
export function App() {
  const client=useMemo(createClient,[]);
  const components=useMemo(()=>({project:ProjectPanel,inspector:InspectorPanel,status:()=> <SimulatorPanel client={client}/>}),[client]);
  function ready({api}:DockviewReadyEvent) {
    api.addPanel({id:'status',component:'status',title:'BMS · Modbus 从站'});
    api.addPanel({id:'project',component:'project',title:'项目文件',position:{referencePanel:'status',direction:'left'},initialWidth:240});
    api.addPanel({id:'inspector',component:'inspector',title:'属性',position:{referencePanel:'status',direction:'right'},initialWidth:250});
  }
  return <TooltipProvider delayDuration={400}><div className="flex h-screen flex-col bg-background text-foreground"><header className="flex h-8 shrink-0 items-center gap-5 border-b px-3 text-xs"><strong>ESSim</strong><span>储能协议仿真工作台</span></header><main className="min-h-0 flex-1"><DockviewReact className="h-full" components={components} onReady={ready} theme={themeLight}/></main><footer className="border-t px-3 py-1 text-xs text-muted-foreground">正式工程 · 本地 Tauri · libmodbus 教学闭环</footer></div></TooltipProvider>;
}
