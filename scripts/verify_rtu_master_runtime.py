"""M05 RTU主站独立PTY互操作；不验证真实RS-485电气、方向或时序。"""
import asyncio
import argparse
import json
import os
import selectors
import tempfile
import threading
import time
import tty
from pathlib import Path
import pymodbus
from pymodbus.server import ModbusSerialServer
from verify_tcp_master_runtime import model, context

async def main():
    parser=argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--binary',type=Path,default=Path('target/debug/examples/tcp_master'))
    args=parser.parse_args()
    assert pymodbus.__version__=='3.11.3'
    a,sa=os.openpty();b,sb=os.openpty();tty.setraw(sa);tty.setraw(sb)
    path=os.ttyname(sa);server_path=os.ttyname(sb)
    diagnostics={}
    stop=threading.Event();fault={'next':None};frames=[];errors=[]
    def forward():
        try:
            with selectors.DefaultSelector() as sel:
                sel.register(a,selectors.EVENT_READ,b);sel.register(b,selectors.EVENT_READ,a)
                while not stop.is_set():
                    for key,_ in sel.select(.01):
                        data=os.read(key.fd,512)
                        if not data:continue
                        if key.fd==b and fault['next']:
                            mode=fault['next'];fault['next']=None
                            if mode=='crc':data=data[:-1]+bytes([data[-1]^1])
                            elif mode=='late':time.sleep(.13)
                            elif mode=='partial':data=data[:4]
                        while data:
                            n=os.write(key.data,data);data=data[n:]
        except Exception as exc:
            if not stop.is_set():errors.append(str(exc))
    thread=threading.Thread(target=forward,daemon=True);thread.start()
    def trace(sending,packet):
        frames.append({'direction':'TX' if sending else 'RX','hex':packet.hex(' ')})
        return packet
    server=ModbusSerialServer(context(),port=server_path,baudrate=19200,parity='N',bytesize=8,stopbits=2,ignore_missing_devices=True,trace_packet=trace)
    process=None
    with tempfile.TemporaryDirectory(prefix='essim-rtu-master-') as temp:
        alias=Path(temp)/'serial-device';alias.symlink_to(path)
        snapshot=dict(endpoint=dict(id='e',name='RTU软件互操作',role='master',transport=dict(kind='rtu',path=str(alias),baud=19200,parity='none',dataBits=8,stopBits=2),master=dict(connectTimeoutMs=300,requestTimeoutMs=80,reconnectMs=100,staleMultiplier=3),units=[dict(id=f'u{i}',name=f'U{i}',station=i,enabled=True,model=dict(id='m'),device=None) for i in [1,2,3]]),values={f'u{i}':dict(model=model(),raw={},stale=True) for i in [1,2,3]})
        source=Path(temp)/'snapshot.json';source.write_text(json.dumps(snapshot))
        await server.serve_forever(background=True)
        process=await asyncio.create_subprocess_exec(str(args.binary.resolve()),str(source),str(Path(temp)/'runs'),stdin=asyncio.subprocess.PIPE,stdout=asyncio.subprocess.PIPE)
        async def rpc(value):
            process.stdin.write((json.dumps(value)+'\n').encode());await process.stdin.drain()
            result=json.loads(await asyncio.wait_for(process.stdout.readline(),5));assert 'Ok'in result,result;return result['Ok']
        async def query(unit='u1'):
            return await rpc(dict(query=dict(endpoint='e',unit=unit,points=[p['id']for p in model()['points']])))
        async def command(kind,**kw):
            result=await rpc(dict(endpoint='e',commandId=f'rtu-{time.monotonic_ns()}',action=dict(kind=kind,**kw)))
            for _ in range(400):
                v=await query()
                if not v['busy']:return v
                await asyncio.sleep(.01)
            raise AssertionError('主站操作未结束')
        try:
            v=await command('connect');assert v['state']=='connected',v
            for unit,value in [('u1','7'),('u2','14')]:
                await command('read',unit=unit,points=[])
                assert (await query(unit))['points']['holding0']['value']['value']==value
            for ids,kind,value in [(['coil0'],'bool',True),(['coil0','coil1','coil2','coil3'],'bool',True),(['holding0'],'unsigned','42'),(['holding0','holding1','holding2','holding3'],'unsigned','43')]:
                v=await command('write',unit='u1',values=[[id,dict(kind=kind,value=value)]for id in ids]);assert all(v['points'][id]['writeResult']=='协议已确认，回验一致'for id in ids),v
            await command('read',unit='u3',points=['holding0']);v=await query('u3');assert v['points']['holding0']['quality']=='failed'
            await command('read',unit='u2',points=['holding0']);assert (await query('u2'))['points']['holding0']['value']['value']=='14'
            before=sum(1 for f in frames if f['direction']=='RX'and int(f['hex'].split()[1],16)==6)
            fault['next']='late'
            v=await command('write',unit='u1',values=[['holding0',dict(kind='unsigned',value='99')],['holding2',dict(kind='unsigned',value='100')]])
            assert '不确定'in v['points']['holding0']['writeResult']and v['points']['holding2']['writeResult']=='未执行',v
            assert sum(1 for f in frames if f['direction']=='RX'and int(f['hex'].split()[1],16)==6)==before+1
            v=await command('read',unit='u1',points=['holding0']);assert v['points']['holding0']['value']['value']=='99'
            for mode in ['crc','late','partial']:
                fault['next']=mode
                v=await command('read',unit='u1',points=['holding0']);assert v['points']['holding0']['quality']=='failed',(mode,v)
                await command('read',unit='u2',points=['holding0']);assert (await query('u2'))['points']['holding0']['value']['value']=='14'
            for fc in [7,8,11,12,17]:
                v=await command('diagnose',unit='u1',function=fc,sub=0,data=4660 if fc==8 else 0)
                assert v.get('diagnostic')or v['error'],(fc,v)
                diagnostics[str(fc)]={'response':v.get('diagnostic'),'error':v['error']}
                if fc==8:assert v['diagnostic']and'12 34'in v['diagnostic'],v
            await command('setUnitPolling',unit='u3',enabled=False)
            await command('startPolling');await asyncio.sleep(.25);v=await query();assert v['polling']and v['state']=='connected'
            stop.set();await asyncio.to_thread(thread.join,1);os.close(a);os.close(sa)
            await asyncio.sleep(.35);v=await query();assert v['polling']and v['state']in ['reconnecting','connecting','failed'],v
            a,sa=os.openpty();tty.setraw(sa);path=os.ttyname(sa);alias.unlink();alias.symlink_to(path)
            stop.clear();thread=threading.Thread(target=forward,daemon=True);thread.start()
            for _ in range(150):
                v=await query()
                if v['state']=='connected'and v['points']['holding0']['quality']=='valid':break
                await asyncio.sleep(.02)
            assert v['state']=='connected'and v['points']['holding0']['quality']=='valid',v
            await command('stopPolling');await asyncio.sleep(.2);count=(await query())['requests'];await asyncio.sleep(.2);assert (await query())['requests']==count
            await command('disconnect');await asyncio.sleep(.1);v=await query();assert v['state']=='disconnected'
            assert (await command('connect'))['state']=='connected';await command('disconnect');await asyncio.sleep(.1)
            records=[json.loads(line)for path in (Path(temp)/'runs').glob('*.jsonl')for line in path.read_text().splitlines()]
            facts=[r.get('fact',{})for r in records];assert any(f.get('kind')=='rtuFrame'and f.get('discarded')for f in facts)
            assert any(f.get('kind')=='rtuFrame'and f.get('complete')and not f.get('crcValid')for f in facts)
            fcs=sorted({int(f['hex'].split()[1],16)for f in frames if f['direction']=='RX'})
            assert all(fc in fcs for fc in [1,2,3,4,5,6,15,16]),fcs
            assert not errors,errors
            print(json.dumps(dict(pymodbus=pymodbus.__version__,transport='PTY RTU',activeUnits=2,missingUnit=3,pointsPerUnit=16,functionCodes=fcs,crcFailureIsolated=True,lateResponseDiscarded=True,missingUnitIsolated=True,reopened=True,stopPollingNoBacklog=True,writeTimeoutNotRetried=True,partialFrameIsolated=True,channelReopened=True,diagnostics=diagnostics,hardwareValidated=False),ensure_ascii=False))
        finally:
            if process and process.returncode is None:
                process.stdin.write(b'quit\n');await process.stdin.drain();await asyncio.wait_for(process.wait(),5)
            await server.shutdown();stop.set();await asyncio.to_thread(thread.join,1)
            for fd in [a,sa,b,sb]:os.close(fd)
if __name__=='__main__':asyncio.run(main())
