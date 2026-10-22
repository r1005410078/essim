"""独立异步PyModbus持续扫描：真实完成计时、固定采样、完整录制与资源拒绝报告。
地址、数值和负载均为测试约定；不修改本机IP。使用已配置127.0.0.1的不同端口。
"""
import argparse, asyncio, collections, hashlib, json, math, os, platform, resource, socket, sys, tempfile, time
from pathlib import Path
from pymodbus.client import AsyncModbusTcpClient
from verify_tcp_slave_runtime import snapshot, point

BUDGET = 2 * 1024**3

def points(n):
    # 四区各有65536个地址；每个业务点占一个地址，不把100000点挤入单区。
    areas=['holding','input','coil','discrete']
    if n>4*65536: raise ValueError('超过四区u16/bool地址容量')
    return [point(f'p{i}',areas[i//65536],i%65536,i%65536 if i<2*65536 else bool(i%2),i<65536 or i>=2*65536) for i in range(n)]

def ranges(n):
    areas=['holding','input','coil','discrete']; result=[]
    for i,area in enumerate(areas):
        count=min(65536,max(0,n-i*65536)); width=125 if i<2 else 2000
        result.extend((area,address,min(width,count-address)) for address in range(0,count,width))
    return result

def percentile(hist,p):
    if not sum(hist): return None
    target=math.ceil(sum(hist)*p);total=0
    for i,n in enumerate(hist):
        total+=n
        if total>=target:return i/10

async def process_output(*args):
    p=await asyncio.create_subprocess_exec(*args,stdout=asyncio.subprocess.PIPE,stderr=asyncio.subprocess.DEVNULL)
    out,_=await p.communicate();return out.decode().strip() if p.returncode==0 else None

async def main(a):
    report={'binary':str(Path(a.binary).resolve()),'binarySha256':hashlib.sha256(Path(a.binary).read_bytes()).hexdigest(),'platform':platform.platform(),'python':sys.version.split()[0],'client':'PyModbus 3.11.3 asyncio','build':'debug','endpoints':a.endpoints,'unitsPerEndpoint':a.units,'pointsPerUnit':a.points,'clientsPerEndpoint':a.clients,'requestedConnections':a.endpoints*a.clients,'totalPoints':a.endpoints*a.units*a.points,'recording':'完整JSONL；4录制线程，64KiB／5ms上限；每文件独立sync_data因果屏障','sampleMethod':'ps RSS/%CPU/time 每秒采样；RSS峰值为采样最大值，非内核瞬时峰值；线程与fd每10秒；独立核心与驱动进程','phases':[],'samples':[],'faults':[]}
    def save():Path(a.output).write_text(json.dumps(report,ensure_ascii=False,indent=2)+'\n')
    data=points(a.points)
    template=snapshot('e0',1502,a.units,0)
    for v in template['values'].values():v['model']['points']=data;v['raw']={p['id']:p['initial'] for p in data}
    estimate=len(json.dumps(template).encode())*a.endpoints
    report['estimatedInputBytes']=estimate;report['loadGuardBytes']=BUDGET//8
    if estimate>BUDGET//8:
        report.update(result='NOT_RUN_RESOURCE_GUARD',reason='输入文件暂存按序列化长度×8预检超出2GiB工程预算；未构造全量输入、未启动目标组合。不是容量通过。');save();print(json.dumps({'result':report['result'],'output':a.output}));return
    reservations=[];ports=[]
    for _ in range(a.endpoints):
        s=socket.socket();s.bind(('127.0.0.1',0));ports.append(s.getsockname()[1]);reservations.append(s)
    root=Path(a.root) if a.root else Path(tempfile.mkdtemp(prefix='essim-m06-scale-'));root.mkdir(parents=True,exist_ok=True);report['runRoot']=str(root)
    source=root/'snapshots.json'
    with source.open('w') as f:
        f.write('[')
        for i,port in enumerate(ports):
            row=snapshot(f'e{i}',port,a.units,0)
            for v in row['values'].values():v['model']['points']=data;v['raw']={p['id']:p['initial'] for p in data}
            if i:f.write(',')
            json.dump(row,f)
        f.write(']')
    for s in reservations:s.close()
    stderr=(root/'server.stderr.log').open('wb')
    begin=time.monotonic();server=await asyncio.create_subprocess_exec(a.binary,str(source),str(root/'runs'),stdin=asyncio.subprocess.PIPE,stdout=asyncio.subprocess.PIPE,stderr=stderr,limit=16*1024*1024)
    clients=[];sampler=None;stopping=False;phase_index=0
    async def sample():
        index=0
        while not stopping:
            sample={'elapsedSeconds':round(time.monotonic()-begin,3)}
            for name,pid in [('core',server.pid),('driver',os.getpid())]:
                raw=await process_output('/bin/ps','-o','%cpu=,rss=,time=','-p',str(pid));sample[name+'Ps']=raw
                if raw:
                    fields=raw.split();sample[name+'CpuPercent']=float(fields[0]);sample[name+'RssKiB']=int(fields[1])
                if index%10==0:
                    threads=await process_output('/bin/ps','-M','-p',str(pid));sample[name+'Threads']=max(0,len(threads.splitlines())-1) if threads else None
                    fds=await process_output('/usr/sbin/lsof','-nP','-p',str(pid),'-Ff');sample[name+'Fds']=sum(line.startswith('f') and line[1:].isdigit() for line in fds.splitlines()) if fds else None
            sample['diskBytes']=sum(p.stat().st_size for p in (root/'runs').glob('*.jsonl'))
            report['samples'].append(sample);save();index+=1;await asyncio.sleep(1)
    try:
        for _ in ports:
            line=await asyncio.wait_for(server.stdout.readline(),120)
            if not line.startswith(b'{'):raise RuntimeError('端点启动失败：'+line.decode())
        ready=await asyncio.wait_for(server.stdout.readline(),30)
        if ready.strip()!=b'READY':raise RuntimeError('未就绪：'+ready.decode())
        report['startupSeconds']=round(time.monotonic()-begin,3);report['inputBytes']=source.stat().st_size
        sampler=asyncio.create_task(sample())
        connecting=asyncio.Semaphore(128)
        async def connect_one(endpoint,port):
            async with connecting:
                c=AsyncModbusTcpClient('127.0.0.1',port=port,timeout=a.timeout,retries=0)
                if not await c.connect():raise RuntimeError(f'连接失败 e{endpoint}')
                clients.append((endpoint,c))
        await asyncio.gather(*(connect_one(i,port) for i,port in enumerate(ports) for _ in range(a.clients)))
        report['actualConnections']=len(clients)
        plans=ranges(a.points)
        for period in a.periods:
            phase={'targetPeriodSeconds':period,'durationSeconds':a.duration,'requests':0,'exceptions':0,'timeouts':0,'unitScansCompleted':0,'unitScansFailed':0,'missedRounds':0,'inFlight':0,'maxInFlight':0,'healthyBefore':{'requests':0,'errors':0},'healthyAfter':{'requests':0,'errors':0},'endpoints':{},'scanDurationsSeconds':[]}
            hist=[0]*100001;scan_hist=[0]*100001;healthy_before=[0]*100001;healthy_after=[0]*100001;fault_started=False;start=time.monotonic();deadline=start+a.duration
            async def scan(endpoint,c):
                next_round=start
                counters=phase['endpoints'].setdefault(str(endpoint),{'requests':0,'errors':0})
                while time.monotonic()<deadline:
                    await asyncio.sleep(max(0,next_round-time.monotonic()))
                    if time.monotonic()>=deadline:break
                    for unit in range(1,a.units+1):
                        tick=time.monotonic();ok=True
                        for area,address,count in plans:
                            t=time.monotonic();failed=False
                            phase['inFlight']+=1;phase['maxInFlight']=max(phase['maxInFlight'],phase['inFlight'])
                            try:
                                method={'holding':c.read_holding_registers,'input':c.read_input_registers,'coil':c.read_coils,'discrete':c.read_discrete_inputs}[area]
                                result=await method(address,count=count,device_id=unit)
                                if result.isError():phase['exceptions']+=1;counters['errors']+=1;ok=False;failed=True
                            except Exception:phase['timeouts']+=1;counters['errors']+=1;ok=False;failed=True
                            phase['inFlight']-=1
                            bucket=min(100000,int((time.monotonic()-t)*10000))
                            if endpoint>=2:
                                side='healthyAfter' if fault_started else 'healthyBefore';phase[side]['requests']+=1;phase[side]['errors']+=int(failed);(healthy_after if fault_started else healthy_before)[bucket]+=1
                            hist[bucket]+=1;phase['requests']+=1;counters['requests']+=1
                        duration=time.monotonic()-tick;scan_hist[min(100000,int(duration*10000))]+=1
                        phase['unitScansCompleted' if ok else 'unitScansFailed']+=1
                    next_round+=period
                    if next_round<time.monotonic():
                        skipped=math.ceil((time.monotonic()-next_round)/period);phase['missedRounds']+=skipped;next_round+=skipped*period
            workers=[asyncio.create_task(scan(endpoint,c)) for endpoint,c in clients]
            half=None
            if a.faults and len(ports)>1:
                async def fault():
                    nonlocal fault_started
                    await asyncio.sleep(min(2,a.duration/4));r,w=await asyncio.open_connection('127.0.0.1',ports[0]);w.write(b'\x00\x01\x00\x00');await w.drain();fault_started=True;report['faults'].append({'kind':'TCP半帧','endpoint':0,'atSeconds':time.monotonic()-begin});await asyncio.sleep(min(6,a.duration/2));w.close();await w.wait_closed()
                    if a.lifecycle_faults:
                        stop={'commandId':'fault-stop','endpoint':'e1','action':{'kind':'stop'}}
                        server.stdin.write((json.dumps(stop)+'\n').encode());await server.stdin.drain();reply=await asyncio.wait_for(server.stdout.readline(),15);report['faults'].append({'kind':'停止单端点','endpoint':1,'reply':reply.decode().strip(),'atSeconds':time.monotonic()-begin})
                        await asyncio.sleep(1)
                        stop['commandId']='fault-restart';stop['action']={'kind':'start','maxClients':10};server.stdin.write((json.dumps(stop)+'\n').encode());await server.stdin.drain();reply=await asyncio.wait_for(server.stdout.readline(),15);report['faults'].append({'kind':'重启单端点','endpoint':1,'reply':reply.decode().strip(),'atSeconds':time.monotonic()-begin})
                half=asyncio.create_task(fault())
            await asyncio.gather(*workers)
            if half:await half
            elapsed=time.monotonic()-start
            phase.update(actualSeconds=round(elapsed,3),requestsPerSecond=round(phase['requests']/elapsed,3),requestLatencyP50Ms=percentile(hist,.5),requestLatencyP95Ms=percentile(hist,.95),requestLatencyP99Ms=percentile(hist,.99),unitScanP50Ms=percentile(scan_hist,.5),unitScanP95Ms=percentile(scan_hist,.95),unitScanP99Ms=percentile(scan_hist,.99),histogramResolutionMs=.1,histogramOverflowMs=10000)
            phase['healthyBefore']['latencyP95Ms']=percentile(healthy_before,.95);phase['healthyAfter']['latencyP95Ms']=percentile(healthy_after,.95)
            del phase['scanDurationsSeconds'];report['phases'].append(phase);save();print(json.dumps({'phase':period,'requests':phase['requests'],'errors':phase['exceptions']+phase['timeouts'],'output':a.output}),flush=True)
        for _,c in clients:c.close()
        stopping=True
        if sampler:await sampler
        server.stdin.write(b'quit\n');await server.stdin.drain();await asyncio.wait_for(server.wait(),120)
        report['serverExitCode']=server.returncode
        reports=[];bad=0;lines=0;stopped_count=0
        for file in (root/'runs').glob('*.jsonl'):
            expected=0;last=None
            try:
                with file.open() as f:
                    header=json.loads(next(f));assert header['kind']=='snapshot' and header['schema']==1
                    for line in f:
                        row=json.loads(line);expected+=1;assert int(row['sequence'])==expected;last=row['fact'];lines+=1
                if last and last.get('kind')=='stopped':stopped_count+=1
            except Exception as e:bad+=1;reports.append({'file':file.name,'error':str(e)})
        report['recordingIntegrity']={'files':len(list((root/'runs').glob('*.jsonl'))),'continuousFacts':lines,'invalidFiles':bad,'filesEndingStopped':stopped_count,'errors':reports}
        report['diskBytes']=sum(p.stat().st_size for p in (root/'runs').glob('*.jsonl'))
        for name in ['core','driver']:
            values=[s[name+'RssKiB'] for s in report['samples'] if name+'RssKiB' in s];report[name+'SampledPeakRssKiB']=max(values) if values else None
        report['result']='COMPLETED' if server.returncode==0 and bad==0 and stopped_count==len(list((root/'runs').glob('*.jsonl'))) else 'FAILED_INTEGRITY_OR_EXIT'
    except Exception as e:
        report.update(result='FAILED_OR_RESOURCE_LIMIT',reason=str(e));save()
    finally:
        stopping=True
        if sampler:
            try:await sampler
            except Exception:pass
        for _,c in clients:c.close()
        if server.returncode is None:
            try:server.stdin.write(b'quit\n');await server.stdin.drain();await asyncio.wait_for(server.wait(),30)
            except Exception:server.terminate();await server.wait()
        stderr.close();report['serverStderr']=(root/'server.stderr.log').read_text()[-8000:];save()
    print(json.dumps({'result':report['result'],'output':a.output}),flush=True)

if __name__=='__main__':
    p=argparse.ArgumentParser();p.add_argument('--binary',required=True);p.add_argument('--output',required=True);p.add_argument('--root');p.add_argument('--endpoints',type=int,default=10);p.add_argument('--units',type=int,default=1);p.add_argument('--points',type=int,default=100);p.add_argument('--clients',type=int,default=1);p.add_argument('--periods',type=float,nargs='+',default=[10,5,1]);p.add_argument('--duration',type=float,default=10);p.add_argument('--timeout',type=float,default=5);p.add_argument('--faults',action='store_true');p.add_argument('--lifecycle-faults',action='store_true');a=p.parse_args()
    if not 1<=a.endpoints<=1000 or not 1<=a.units<=247 or not 1<=a.clients<=10 or a.duration<=0 or any(v<=0 for v in a.periods):p.error('规模或时间参数非法')
    soft,hard=resource.getrlimit(resource.RLIMIT_NOFILE);resource.setrlimit(resource.RLIMIT_NOFILE,(min(32768,hard if hard!=resource.RLIM_INFINITY else 32768),hard))
    asyncio.run(main(a))
