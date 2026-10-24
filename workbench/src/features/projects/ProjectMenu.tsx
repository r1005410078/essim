import {useState} from "react";
import { FolderOpen, FolderPlus, History, LogOut, ChevronDown, Folder } from 'lucide-react';
import { DropdownMenu, DropdownMenuTrigger, DropdownMenuContent, DropdownMenuItem, DropdownMenuSeparator, DropdownMenuLabel, DropdownMenuSub, DropdownMenuSubTrigger, DropdownMenuSubContent } from '@essim/ui/components/dropdown-menu';
import type { RecentProject } from '../../shared/api/project';
export function ProjectMenu({ name, entry, recent, disabled, request }: { name: string; entry: string; recent: RecentProject[]; disabled?: boolean; request: (action: 'open' | 'create' | 'close', entry?: string) => void }) {
  const [opened,setOpened]=useState(false);
  return <DropdownMenu modal={false} open={opened} onOpenChange={setOpened}><DropdownMenuTrigger asChild><button className="titlebar-project" title={entry} aria-label={`工程：${name}`} disabled={disabled} onClick={e=>{if(e.detail===0){e.currentTarget.focus();setOpened(value=>!value);}}}><Folder size={16}/><span>{name}</span><ChevronDown size={13}/></button></DropdownMenuTrigger><DropdownMenuContent align="start" onCloseAutoFocus={e => { if (document.querySelector('dialog[open]')) e.preventDefault(); }}>
    <DropdownMenuLabel>当前工程 · {name}</DropdownMenuLabel><DropdownMenuSeparator/>
    <DropdownMenuItem onSelect={() => request('open')}><FolderOpen/>打开工程…</DropdownMenuItem>
    <DropdownMenuItem onSelect={() => request('create')}><FolderPlus/>新建工程…</DropdownMenuItem>
    <DropdownMenuSub><DropdownMenuSubTrigger><History/>打开最近工程</DropdownMenuSubTrigger><DropdownMenuSubContent>{recent.length ? recent.map(item => <DropdownMenuItem key={item.entry} title={item.entry} onSelect={() => request('open', item.entry)}><Folder/>{item.name}</DropdownMenuItem>) : <DropdownMenuItem disabled>暂无最近工程</DropdownMenuItem>}</DropdownMenuSubContent></DropdownMenuSub>
    <DropdownMenuSeparator/><DropdownMenuItem onSelect={() => request('close')}><LogOut/>关闭工程</DropdownMenuItem>
  </DropdownMenuContent></DropdownMenu>;
}
