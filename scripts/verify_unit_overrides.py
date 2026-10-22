"""单层覆盖独立 PyModbus TCP/PTY 验证；零基100/110/112为测试约定，不访问真实设备。"""
import asyncio, copy, json, os, selectors, socket, tempfile, threading, time, tty
from pathlib import Path
from pymodbus.client import ModbusTcpClient, ModbusSerialClient
from pymodbus.datastore import ModbusDeviceContext, ModbusServerContext, ModbusSequentialDataBlock
from pymodbus.server import ModbusTcpServer, ModbusSerialServer

ROOT=Path(__file__).resolve().parents[1]
def model():
    return dict(id='m',name='覆盖验证',points=[dict(id='p',name='测试电压',category='measurement',area='holding',address=100,valueType='u16',scale='1',unit='V',initial={'kind':'unsigned','value':'12'},initialEngineering='12',byteOrder='big',wordOrder='big',note='',slaveWritable=True,masterPeriodMs=10000)])
def endpoint(transport,role,units=2):
    return dict(id='e',name='覆盖验证端点',role=role,transport=transport,units=[dict(id=f'u{i}',name=f'Unit {i}',station=i,enabled=True,model={'id':'m'},device=None,overrides={'pointOverrides':{'p':{'address':110,'initialEngineering':'20'}} if i==1 else {},'addedPoints':[],'removedPointIds':[]}) for i in range(1,units+1)])
class Bridge:
    def __init__(self):
        self.a,self.sa=os.openpty();self.b,self.sb=os.openpty();tty.setraw(self.sa);tty.setraw(self.sb)
        self.paths=[os.ttyname(self.sa),os.ttyname(self.sb)];self.stop=threading.Event();self.thread=threading.Thread(target=self.forward,daemon=True);self.thread.start()
    def forward(self):
        with selectors.DefaultSelector() as sel:
            sel.register(self.a,1,self.b);sel.register(self.b,1,self.a)
            while not self.stop.is_set():
                for key,_ in sel.select(.01):
                    try:os.write(key.data,os.read(key.fd,4096))
                    except OSError:
                        if not self.stop.is_set():raise
    def close(self):
        self.stop.set();self.thread.join(2)
        for fd in [self.a,self.sa,self.b,self.sb]:os.close(fd)
def rtu(path):return dict(kind='rtu',path=path,baud=19200,parity='none',dataBits=8,stopBits=2)
class Harness:
    async def start(self,root):
        self.process=await asyncio.create_subprocess_exec(str(ROOT/'target/debug/examples/unit_overrides'),str(root),stdin=asyncio.subprocess.PIPE,stdout=asyncio.subprocess.PIPE)
        return self
    async def rpc(self,op,**kw):
        expected_error=kw.pop('expected_error',None)
        # 主站完成视图与工作线程归还操作门闩之间允许短暂差距；仅重试明确未受理的忙拒绝。
        for attempt in range(100):
            self.process.stdin.write((json.dumps(dict(op=op,**kw))+'\n').encode());await self.process.stdin.drain()
            line=await asyncio.wait_for(self.process.stdout.readline(),15)
            assert line, f'harness exited {self.process.returncode}'
            result=json.loads(line)
            if str(result.get('Err','')).startswith('ENDPOINT_BUSY:'):
                await asyncio.sleep(.02);continue
            if expected_error:
                assert str(result.get('Err','')).startswith(expected_error+':'),result;return result
            assert 'Ok' in result,result;return result['Ok']
        raise AssertionError('endpoint operation did not release its permit')
    async def config(self,kind,**kw):return await self.rpc('execute',command=dict(commandId=str(time.monotonic_ns()),action=dict(kind=kind,**kw)))
    async def command(self,role,kind,**kw):return await self.rpc(role,command=dict(commandId=str(time.monotonic_ns()),endpoint='e',action=dict(kind=kind,**kw)))
    async def close(self):
        self.process.stdin.write(b'quit\n');await self.process.stdin.drain();await asyncio.wait_for(self.process.wait(),10)
async def slave_suite(kind,root):
    bridge=Bridge() if kind=='rtu' else None
    if bridge:transport=rtu(bridge.paths[0])
    else:
        with socket.socket() as probe:probe.bind(('127.0.0.1',0));port=probe.getsockname()[1]
        transport=dict(kind='tcp',address='127.0.0.1',port=port)
    h=await Harness().start(root);client=None
    try:
        e=endpoint(transport,'slave',1 if bridge else 2)
        first=copy.deepcopy(e);first['units']=first['units'][:1]
        await h.config('createEndpointWithModel',endpoint=first,model=model());await h.config('saveEndpoint',endpoint=e);await h.config('apply',id='e',resetIncompatible=False)
        await h.command('slave','start',maxClients=4)
        client=ModbusSerialClient(bridge.paths[1],baudrate=19200,parity='N',stopbits=2,timeout=1,retries=0) if bridge else ModbusTcpClient('127.0.0.1',port=port,timeout=1,retries=0)
        assert await asyncio.to_thread(client.connect)
        async def read(address,unit=1):
            value=await asyncio.to_thread(client.read_holding_registers,address,count=1,device_id=unit);assert not value.isError(),value;return value.registers[0]
        assert await read(110)==20
        if not bridge:assert await read(100,2)==12
        await h.command('slave','setValues',unit='u1',values=[['p',{'kind':'unsigned','value':'33'}]])
        captured=await h.rpc('capture',endpoint='e',unit='u1',points=['p']);assert captured[0]['engineering']=='33'
        e['units'][0]['overrides']['pointOverrides']['p']={'address':112,'initialEngineering':'30'}
        await h.config('saveEndpoint',endpoint=e);assert await read(110)==33
        await h.rpc('capture',endpoint='e',unit='u1',points=['p'],expected_error='MAPPING_PENDING')
        await h.config('apply',id='e',resetIncompatible=False);client.close();await h.command('slave','start',maxClients=4);assert await asyncio.to_thread(client.connect)
        assert await read(112)==33
        await h.command('slave','resetValues',unit='u1',points=['p']);assert await read(112)==30
        e['units'][0]['overrides']['pointOverrides']['p']['initialEngineering']='40';await h.config('saveEndpoint',endpoint=e)
        plan=await h.rpc('preview',endpoint='e',unit='u1',selection={'fields':{'p':['initialEngineering']}});await h.config('promoteOverrides',plan=plan)
        assert await read(112)==30
        if not bridge:assert await read(100,2)==12
        await h.config('apply',id='e',resetIncompatible=False);client.close();await h.command('slave','start',maxClients=4);assert await asyncio.to_thread(client.connect)
        await h.command('slave','resetValues',unit='u1',points=['p']);assert await read(112)==40
        if not bridge:
            assert await read(100,2)==12
            await h.command('slave','resetValues',unit='u2',points=['p']);assert await read(100,2)==40
        await h.command('slave','stop')
        return dict(transport=kind,role='slave',checks=['distinct effective addresses/initials','capture confirmed value','save/promotion does not apply','compatible value migration','applied effective reset','shared unit isolation'])
    finally:
        if client:client.close()
        await h.close()
        if bridge:bridge.close()
async def master_suite(kind,root):
    bridge=Bridge() if kind=='rtu' else None
    values=[0]*40;values[0]=7;values[10]=0;values[11]=42;values[12]=55
    context=ModbusServerContext(devices={i:ModbusDeviceContext(hr=ModbusSequentialDataBlock(101,values.copy())) for i in [1,2]},single=False)
    frames=[]
    def trace(sending,packet):
        if not sending:frames.append(bytes(packet))
        return packet
    server=ModbusSerialServer(context,port=bridge.paths[1],baudrate=19200,parity='N',stopbits=2,trace_packet=trace) if bridge else ModbusTcpServer(context,address=('127.0.0.1',0),trace_packet=trace)
    await server.serve_forever(background=True)
    transport=rtu(bridge.paths[0]) if bridge else dict(kind='tcp',address='127.0.0.1',port=server.transport.sockets[0].getsockname()[1])
    h=await Harness().start(root)
    try:
        e=endpoint(transport,'master');e['units'][0]['overrides']['pointOverrides']['p']['valueType']='u32'
        first=copy.deepcopy(e);first['units']=first['units'][:1]
        await h.config('createEndpointWithModel',endpoint=first,model=model());await h.config('saveEndpoint',endpoint=e);await h.config('apply',id='e',resetIncompatible=False)
        async def query(unit='u1'):return await h.rpc('masterQuery',query=dict(endpoint='e',unit=unit,points=['p']))
        async def done(unit='u1'):
            for _ in range(200):
                q=await query(unit)
                if not q['busy']:return q
                await asyncio.sleep(.02)
            raise AssertionError('master operation timeout')
        await h.command('master','connect');assert (await done())['state']=='connected'
        await h.rpc('capture',endpoint='e',unit='u1',points=['p'],expected_error='VALUE_UNCONFIRMED')
        for unit,expected in [('u1','42'),('u2','7')]:
            await h.command('master','read',unit=unit,points=['p']);q=await done(unit);assert q['points']['p']['value']['value']==expected,q
        samples=await h.rpc('capture',endpoint='e',unit='u1',points=['p']);assert samples[0]['engineering']=='42'
        e['units'][0]['overrides']['pointOverrides']['p']['initialEngineering']='42';await h.config('saveEndpoint',endpoint=e)
        plan=await h.rpc('preview',endpoint='e',unit='u1',selection={'fields':{'p':['initialEngineering']}});await h.config('promoteOverrides',plan=plan)
        # Fetch saved source after promotion cleared the initial override.
        e=(await h.rpc('query'))['catalog']['endpoints'][0];e['units'][0]['overrides']['pointOverrides']['p']['address']=112;e['units'][0]['overrides']['pointOverrides']['p']['valueType']='u16'
        await h.config('saveEndpoint',endpoint=e);await h.config('apply',id='e',resetIncompatible=False)
        c=(await h.rpc('query'))['catalog'];assert 'p' not in c['values']['u1']['raw']
        await h.command('master','connect');await done();await h.command('master','read',unit='u1',points=['p']);q=await done();assert q['points']['p']['value']['value']=='55'
        await h.command('master','disconnect');await done()
        await h.rpc('capture',endpoint='e',unit='u1',points=['p'],expected_error='VALUE_UNCONFIRMED')
        await h.config('resetUnit',id='e',unit='u1')
        functions=[f[1 if bridge else 7] for f in frames if len(f)>(1 if bridge else 7)]
        assert functions and all(f==3 for f in functions),functions
        return dict(transport=kind,role='master',requests=len(functions),checks=['override address/type read','independent unit result','quality-gated capture','promotion/reset sends no writes','mapping application invalidates old result'])
    finally:
        await h.close();await server.shutdown()
        if bridge:bridge.close()
async def main():
    results=[]
    with tempfile.TemporaryDirectory(prefix='essim-overrides-') as root:
        for kind in ['tcp','rtu']:
            for role,suite in [('slave',slave_suite),('master',master_suite)]:
                result=await suite(kind,Path(root)/f'{kind}-{role}');results.append(result);print(json.dumps(result,ensure_ascii=False),flush=True)
    print(json.dumps(dict(status='passed',suites=results),ensure_ascii=False))
if __name__=='__main__':asyncio.run(main())
