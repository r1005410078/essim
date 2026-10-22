"""M04正式RTU从站：独立PyModbus/PTY功能验证，不代表RS485硬件时序。"""
import json,os,sys,tty,selectors,threading,tempfile,subprocess,time
from pathlib import Path
from pymodbus.client import ModbusSerialClient
from verify_tcp_slave_runtime import snapshot,point

def crc(data):
    value=0xffff
    for byte in data:
        value^=byte
        for _ in range(8):value=(value>>1)^0xa001 if value&1 else value>>1
    return value.to_bytes(2,'little')
def main():
    a,sa=os.openpty();b,sb=os.openpty();tty.setraw(sa);tty.setraw(sb)
    stop=threading.Event()
    def bridge():
        with selectors.DefaultSelector() as sel:
            sel.register(a,1,b);sel.register(b,1,a)
            while not stop.is_set():
                for k,_ in sel.select(.01):os.write(k.data,os.read(k.fd,512))
    thread=threading.Thread(target=bridge,daemon=True);thread.start()
    client=None;server=None
    try:
        with tempfile.TemporaryDirectory(prefix='essim-rtu-slave-') as root:
            root=Path(root);s=snapshot('e',0,units=1)
            s['endpoint']['transport']=dict(kind='rtu',path=os.ttyname(sa),baud=19200,parity='none',dataBits=8,stopBits=2)
            model=s['values']['e-u1']['model']
            for area in ['coil','holding']:
                for i in range(4):model['points'].append(point(f'{area}{i}',area,100+i,False if area=='coil' else i,True))
            model['points'].append(point('protected','holding',200,11,False))
            source=root/'snapshot.json';source.write_text(json.dumps([s]))
            server=subprocess.Popen([str(Path('target/debug/examples/tcp_slave').resolve()),str(source),str(root/'runs')],stdin=subprocess.PIPE,stdout=subprocess.PIPE,stderr=subprocess.PIPE,text=True)
            assert json.loads(server.stdout.readline())['state']=='running'
            assert server.stdout.readline().strip()=='READY'
            def rpc(action):
                server.stdin.write(json.dumps(action)+'\n');server.stdin.flush();v=json.loads(server.stdout.readline());assert 'Ok'in v,v;return v['Ok']
            def cmd(kind,**kw):return rpc(dict(commandId=f'test-{time.monotonic_ns()}',endpoint='e',action=dict(kind=kind,**kw)))
            def query():return rpc(dict(query=dict(endpoint='e',unit='e-u1',points=['holding0','holding1','holding2','holding3'])))
            client=ModbusSerialClient(os.ttyname(sb),baudrate=19200,parity='N',stopbits=2,timeout=.4,retries=0);assert client.connect()
            assert client.read_holding_registers(10,count=1,device_id=1).registers==[7]
            assert client.read_input_registers(0,count=1,device_id=1).registers==[33]
            assert client.read_coils(0,count=1,device_id=1).bits[0]
            assert not client.read_discrete_inputs(0,count=1,device_id=1).bits[0]
            assert not client.write_register(100,42,device_id=1).isError()
            assert not client.write_registers(100,[1,2,3,4],device_id=1).isError()
            assert not client.write_coil(100,True,device_id=1).isError()
            assert not client.write_coils(100,[True,False,True,False],device_id=1).isError()
            assert client.read_coils(100,count=4,device_id=1).bits[:4]==[True,False,True,False]
            assert client.read_holding_registers(100,count=4,device_id=1).registers==[1,2,3,4]
            assert [query()['values'][f'holding{i}']['value']for i in range(4)]==['1','2','3','4']
            assert client.read_holding_registers(99,count=2,device_id=1).exception_code==2
            assert client.write_register(200,99,device_id=1).exception_code==2
            assert client.read_holding_registers(200,count=1,device_id=1).registers==[11]
            assert not client.diag_query_data(0x1234,device_id=1).isError()
            assert not client.report_device_id(device_id=1).isError()
            assert client.read_exception_status(device_id=1).exception_code==1
            assert client.diag_clear_counters(device_id=1).exception_code==1
            cmd('setValues',unit='e-u1',values=[['holding0',dict(kind='unsigned',value='55')]])
            assert client.read_holding_registers(100,count=1,device_id=1).registers==[55]
            try:rpc(dict(commandId='invalid-batch',endpoint='e',action=dict(kind='setValues',unit='e-u1',values=[['holding0',dict(kind='unsigned',value='60')],['absent',dict(kind='unsigned',value='1')]])))
            except AssertionError:pass
            else:raise AssertionError('非法批次没有拒绝')
            assert client.read_holding_registers(100,count=1,device_id=1).registers==[55]
            # 独立原始帧用pyserial发送，检验无响应场景及后续恢复。
            wire=client.socket
            def raw(payload,reply=True,corrupt=False):
                wire.reset_input_buffer();packet=payload+crc(payload)
                if corrupt:packet=packet[:-1]+bytes([packet[-1]^1])
                wire.write(packet);wire.flush();time.sleep(.12)
                result=wire.read(wire.in_waiting)
                assert bool(result)==reply,(payload.hex(),result.hex())
                return result
            raw(bytes([0,6,0,100,0,77]),False)
            assert client.read_holding_registers(100,count=1,device_id=1).registers==[77]
            raw(bytes([0,3,0,100,0,1]),False)
            raw(bytes([2,6,0,100,0,88]),False)
            raw(bytes([1,6,0,100,0,99]),False,True)
            wire.write(bytes([1,3,0]));wire.flush();time.sleep(.12);assert wire.in_waiting==0
            assert client.read_holding_registers(100,count=1,device_id=1).registers==[77]
            # 分段与合并请求。
            request=bytes([1,3,0,100,0,1]);packet=request+crc(request)
            wire.reset_input_buffer();wire.write(packet[:3]);time.sleep(.002);wire.write(packet[3:]);wire.flush();time.sleep(.12);assert wire.read(wire.in_waiting)==bytes([1,3,2,0,77])+crc(bytes([1,3,2,0,77]))
            wire.write(packet+packet);wire.flush();time.sleep(.2);assert len(wire.read(wire.in_waiting))==14
            assert cmd('stop')['state']=='stopped';assert cmd('start',maxClients=10)['state']=='running'
            assert client.read_holding_registers(100,count=1,device_id=1).registers==[77]
            cmd('resetValues',unit='e-u1',points=['holding0']);assert client.read_holding_registers(100,count=1,device_id=1).registers==[0]
            assert cmd('stop')['state']=='stopped'
            server.stdin.write('quit\n');server.stdin.flush();server.wait(timeout=10);assert server.returncode==0,server.stderr.read()
            facts=[json.loads(line).get('fact',{})for p in (root/'runs').glob('*.jsonl')for line in p.read_text().splitlines()]
            frames=[f for f in facts if f.get('kind')=='rtuFrame']
            assert any(f.get('stage')=='driverWritten'for f in frames)
            assert any(not f.get('crcValid')for f in frames)
            assert any(not f.get('complete')for f in frames)
            print(json.dumps(dict(transport='PTY RTU',units=1,basicFunctionCodes=[1,2,3,4,5,6,15,16],diagnostics=[8,17],unsupportedDiagnosticsRejected=True,broadcastSilent=True,crcAndShortFrameIsolated=True,fragmentedAndCombinedRequests=True,externalWriteVisible=True,atomicLocalValues=True,restartPreservesValues=True,resetVerified=True,hardwareValidated=False),ensure_ascii=False))
    finally:
        if client:client.close()
        if server and server.poll()is None:server.terminate();server.wait(timeout=5)
        stop.set();thread.join(1)
        for fd in [a,sa,b,sb]:os.close(fd)
if __name__=='__main__':main()
