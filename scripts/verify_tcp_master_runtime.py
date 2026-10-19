"""ESSim M05 主站对独立 PyModbus 3.11.3 TCP 从站互操作。
教学地址：四区零基100起，Unit 1/2；不验证真实EMS或设备动作语义。
"""
import argparse
import asyncio
import json
from pathlib import Path
import tempfile
import pymodbus
from pymodbus.datastore import ModbusDeviceContext, ModbusServerContext, ModbusSequentialDataBlock
from pymodbus.server import ModbusTcpServer


def model():
    points = []
    for area, kind, initial, category in [('coil', 'bool', False, 'control'), ('discrete', 'bool', False, 'indication'), ('holding', 'u16', 0, 'adjustment'), ('input', 'u16', 0, 'measurement')]:
        for i in range(4):
            points.append(dict(id=f'{area}{i}', name=f'{area}{i}', category=category,
                area=area, address=100+i, valueType=kind, scale='1', unit='',
                initial=dict(kind='bool' if kind == 'bool' else 'unsigned', value=initial if kind == 'bool' else str(initial)),
                initialEngineering=str(initial).lower(), byteOrder='big', wordOrder='big', note='',
                slaveWritable=False, masterPeriodMs=100))
    return dict(id='m', name='独立互操作模型', points=points)


def context():
    return ModbusServerContext(devices={i: ModbusDeviceContext(
        co=ModbusSequentialDataBlock(101, [False]*4), di=ModbusSequentialDataBlock(101, [True]*4),
        hr=ModbusSequentialDataBlock(101, [7*i]*4), ir=ModbusSequentialDataBlock(101, [9*i]*4)) for i in [1, 2]}, single=False)


async def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--binary', type=Path, default=Path('target/debug/examples/tcp_master'))
    args = parser.parse_args()
    assert pymodbus.__version__ == '3.11.3'
    frames = []
    def trace(sending, packet):
        frames.append(dict(direction='TX' if sending else 'RX', hex=packet.hex(' ')))
        return packet
    server = ModbusTcpServer(context(), address=('127.0.0.1', 0), trace_packet=trace)
    await server.serve_forever(background=True)
    port = server.transport.sockets[0].getsockname()[1]
    process = None
    with tempfile.TemporaryDirectory(prefix='essim-master-') as temp:
        snapshot = dict(endpoint=dict(id='e', name='互操作主站', role='master', transport=dict(kind='tcp', address='127.0.0.1', port=port),
            master=dict(connectTimeoutMs=300, requestTimeoutMs=200, reconnectMs=100, staleMultiplier=3),
            units=[dict(id=f'u{i}', name=f'U{i}', station=i, enabled=True, model=dict(id='m'), device=None) for i in [1,2]]),
            values={f'u{i}': dict(model=model(), raw={}, stale=True) for i in [1,2]})
        path = Path(temp)/'snapshot.json'; path.write_text(json.dumps(snapshot))
        process = await asyncio.create_subprocess_exec(str(args.binary.resolve()), str(path), str(Path(temp)/'runs'), stdin=asyncio.subprocess.PIPE, stdout=asyncio.subprocess.PIPE)
        async def rpc(data):
            process.stdin.write((json.dumps(data)+'\n').encode()); await process.stdin.drain()
            result = json.loads(await asyncio.wait_for(process.stdout.readline(), 5))
            assert 'Ok' in result, result
            return result['Ok']
        async def query(unit='u1'):
            return await rpc(dict(query=dict(endpoint='e',unit=unit, points=[p['id'] for p in model()['points']])))
        async def done():
            for _ in range(200):
                v = await query()
                if not v['busy']: return v
                await asyncio.sleep(.02)
            raise AssertionError('操作未结束')
        async def command(kind, **kw):
            v = await rpc(dict(commandId=f'check-{len(frames)}', endpoint='e', action=dict(kind=kind, **kw)))
            if kind not in ['disconnect','stopPolling']: v = await done()
            return v
        try:
            assert (await command('connect'))['state'] == 'connected'
            v = await command('read', unit='u1', points=[])
            assert v['points']['holding0']['value']['value'] == '7', v
            assert v['points']['input0']['value']['value'] == '9'
            assert v['points']['discrete0']['value']['value'] is True
            await command('read', unit='u2', points=[])
            assert (await query('u2'))['points']['holding0']['value']['value'] == '14'
            # 单写和多写均应使用主站规则，不依赖从站slaveWritable字段。
            for ids, kind, value in [(['coil0'],'bool',True),(['coil0','coil1','coil2','coil3'],'bool',True),(['holding0'],'unsigned','42'),(['holding0','holding1','holding2','holding3'],'unsigned','43')]:
                v=await command('write', unit='u1', values=[[id,dict(kind=kind,value=value)] for id in ids])
                assert all(v['points'][id]['writeResult']=='协议已确认，回验一致' for id in ids), v
            assert (await query('u2'))['points']['holding0']['value']['value'] == '14'
            await command('setUnitPolling', unit='u1', enabled=False)
            def unit_frames(station):
                return sum(1 for f in frames if f['direction']=='RX' and int(f['hex'].split()[6],16)==station)
            count1=unit_frames(1); count2=unit_frames(2)
            await command('startPolling'); await asyncio.sleep(.35)
            assert unit_frames(1)==count1 and unit_frames(2)>count2
            await command('setUnitPolling', unit='u1', enabled=True)
            await asyncio.sleep(.2)
            v=await query(); assert v['polling'] and v['requests'] > 12
            await command('stopPolling'); await asyncio.sleep(.25)
            before=(await query())['requests']; await asyncio.sleep(.2)
            assert (await query())['requests']==before
            # 断开服务器，再启动采集，验证读失败与恢复，绝不重发控制。
            await command('startPolling'); await server.shutdown(); await asyncio.sleep(.4)
            v=await query(); assert v['polling'] and v['state'] in ['reconnecting','connecting','failed']
            server=ModbusTcpServer(context(),address=('127.0.0.1',port),trace_packet=trace)
            await server.serve_forever(background=True)
            for _ in range(100):
                v=await query()
                if v['state']=='connected' and v['points']['holding0']['quality']=='valid': break
                await asyncio.sleep(.05)
            assert v['state']=='connected' and v['points']['holding0']['quality']=='valid',v
            await command('disconnect'); await asyncio.sleep(.3)
            v=await query(); assert v['state']=='disconnected' and not v['polling']
            fcs=sorted({int(f['hex'].split()[7],16) for f in frames if f['direction']=='RX'})
            assert fcs==[1,2,3,4,5,6,15,16],fcs
            records=list((Path(temp)/'runs').glob('*.jsonl'));assert records
            rows=[json.loads(line) for line in records[0].read_text().splitlines()]
            assert rows[0]['kind']=='masterSnapshot' and 'cache' in rows[0] and any(r.get('fact',{}).get('kind')=='frame' for r in rows)
            assert [int(r['sequence']) for r in rows[1:]]==list(range(1,len(rows))), '可靠记录序号必须连续'
            print(json.dumps(dict(pymodbus=pymodbus.__version__,units=2,pointsPerUnit=16,functionCodes=fcs,wireFrames=len(frames),requests=v['requests'],stopPollingNoBacklog=True,reconnected=True,recorded=True,unitPauseIsolated=True),ensure_ascii=False))
        finally:
            if process and process.returncode is None:
                process.stdin.write(b'quit\n');await process.stdin.drain();await asyncio.wait_for(process.wait(),5)
            await server.shutdown()

if __name__=='__main__':asyncio.run(main())
