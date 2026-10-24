import {useState} from 'react';
import {Radio,Plug,RefreshCw,LoaderCircle,Square,Unplug,CircleAlert,CircleHelp,Clock,Dot,Ban,PauseCircle,TriangleAlert} from 'lucide-react';
import {Tooltip,TooltipContent,TooltipTrigger} from '@essim/ui/components/tooltip';
import type {TreeStatus,TreeStatusKind} from './treeStatus';
const icons={listening:Radio,serial:Plug,connected:Plug,polling:RefreshCw,connecting:LoaderCircle,reconnecting:RefreshCw,stopped:Square,disconnected:Unplug,failed:CircleAlert,unknown:CircleHelp,pending:Clock,dirty:Dot,disabled:Ban,paused:PauseCircle,stale:TriangleAlert,'unit-error':TriangleAlert};
function color(kind:TreeStatusKind){return ['listening','serial','connected','polling'].includes(kind)?'#259552':['connecting','reconnecting','dirty'].includes(kind)?'#2874d9':kind==='failed'?'#d74242':['pending','stale','unit-error'].includes(kind)?'#c98b16':'#8993a2';}
function StatusIcon({status}:{status:TreeStatus}){
 const [open,setOpen]=useState(false);const Icon=icons[status.kind];
 return <Tooltip open={open} onOpenChange={setOpen}><TooltipTrigger asChild>
  <span role="img" aria-label={status.description} onPointerEnter={()=>setOpen(true)} onPointerLeave={()=>setOpen(false)} style={{display:'inline-flex',alignItems:'center',justifyContent:'center',width:16,height:20,color:color(status.kind),flexShrink:0}}>
   <Icon size={13} strokeWidth={1.8}/>
  </span>
 </TooltipTrigger><TooltipContent side="right" sideOffset={6} style={{maxWidth:300,whiteSpace:'pre-line'}}>{status.description}</TooltipContent></Tooltip>;
}
export function TreeStatusIcons({statuses}:{statuses:TreeStatus[]}){return <>{statuses.map(status=><StatusIcon key={status.kind} status={status}/>)}</>;}
