"""独立PyModbus验证通用TCP从站。点地址和值均为本脚本测试约定。
运行：python verify_tcp_slave_runtime.py --binary target/debug/examples/tcp_slave
"""
import argparse,json,socket,subprocess,tempfile,time,platform,resource
from pathlib import Path
from pymodbus.client import ModbusTcpClient

def port():
    with socket.socket() as s:
        s.bind(('127.0.0.1',0));return s.getsockname()[1]
def point(identity,area,address,value,writable=False):
    bit=area in ('coil','discrete')
    return dict(id=identity,name=identity,category='measurement',area=area,address=address,valueType='bool' if bit else 'u16',scale='1',unit='',initial=dict(kind='bool' if bit else 'unsigned',value=bool(value) if bit else str(value)),initialEngineering=str(bool(value)).lower() if bit else str(value),byteOrder='big',wordOrder='big',note='',slaveWritable=writable,masterPeriodMs=1000)
def snapshot(identity,listen,units=2,points=None):
    model=dict(id='shared-model',name='互操作测试模型',points=[point('h','holding',10,7,True),point('c','coil',0,True,True),point('d','discrete',0,False),point('i','input',0,33)])
    if points is not None:model['points']=[point(f'p{i}','holding',i,i%65536,True) for i in range(points)]
    unitlist=[dict(id=f'{identity}-u{i}',name=f'U{i}',station=i,enabled=True,model=dict(id=model['id']),device=None) for i in range(1,units+1)]
    return dict(endpoint=dict(id=identity,name=identity,role='slave',transport=dict(kind='tcp',address='127.0.0.1',port=listen),units=unitlist),values={u['id']:dict(model=model,raw={p['id']:p['initial'] for p in model['points']},stale=False) for u in unitlist})
def main():
    parser=argparse.ArgumentParser();parser.add_argument('--binary',required=True);args=parser.parse_args()
    with tempfile.TemporaryDirectory(prefix='essim-m04-interop-') as root:
        root=Path(root);listen=port();source=root/'snapshot.json';source.write_text(json.dumps([snapshot('e',listen)]))
        server=subprocess.Popen([args.binary,str(source),str(root/'runs')],stdin=subprocess.PIPE,stdout=subprocess.PIPE,stderr=subprocess.PIPE,text=True)
        try:
            assert server.stdout.readline().strip().startswith('{')
            assert server.stdout.readline().strip()=='READY'
            a=ModbusTcpClient('127.0.0.1',port=listen,timeout=2);b=ModbusTcpClient('127.0.0.1',port=listen,timeout=2)
            assert a.connect() and b.connect()
            assert a.read_holding_registers(10,count=1,device_id=1).registers==[7]
            assert a.read_input_registers(0,count=1,device_id=1).registers==[33]
            assert a.read_coils(0,count=1,device_id=1).bits[0]
            assert not a.read_discrete_inputs(0,count=1,device_id=1).bits[0]
            assert not a.write_register(10,42,device_id=1).isError()
            assert b.read_holding_registers(10,count=1,device_id=1).registers==[42]
            assert b.read_holding_registers(10,count=1,device_id=2).registers==[7]
            assert not a.write_registers(10,[99],device_id=1).isError()
            assert not a.write_coil(0,False,device_id=1).isError()
            assert not a.write_coils(0,[True],device_id=1).isError()
            assert a.read_holding_registers(11,count=1,device_id=1).isError()
            assert a.read_holding_registers(10,count=1,device_id=3).isError()
            a.close();b.close()
            server.stdin.write('quit\n');server.stdin.flush();server.wait(timeout=15)
            assert server.returncode==0,server.stderr.read()
            records=list((root/'runs').glob('*.jsonl'));assert len(records)==1
            rows=[json.loads(line) for line in records[0].read_text().splitlines()]
            assert rows[0]['kind']=='snapshot'
            assert len(rows[0]['models'])==1 and len(rows[0]['units'])==2, 'shared model must be stored only once'
            assert rows[-1]['fact']['kind']=='stopped'
            print(json.dumps(dict(result='PASS',client='PyModbus',version=__import__('pymodbus').__version__,platform=platform.platform(),endpoints=1,units=2,clients=2,functions=['01','02','03','04','05','06','0F','10'],checks=['unit isolation','shared writes','address exception','unknown unit','reliable finalized record','headless shutdown']),ensure_ascii=False))
        finally:
            if server.poll() is None:server.kill();server.wait()
if __name__=='__main__':main()
