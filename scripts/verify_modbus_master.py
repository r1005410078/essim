"""Rust 主站对独立 PyModbus 3.11.3 从站；RTU 使用 PTY，不验证物理 RS-485。
运行前构建 client_demo，再用安装 PyModbus 3.11.3 / pyserial 3.5 的 Python 执行。
"""
import argparse
import asyncio
import os
from pathlib import Path
import selectors
import threading
import tty

import pymodbus
import serial
from pymodbus.datastore import ModbusDeviceContext, ModbusServerContext, ModbusSequentialDataBlock
from pymodbus.server import ModbusTcpServer, ModbusSerialServer


def context():
    # PyModbus 3.11.3 DeviceContext 内部将零基协议地址 +1，因此块起点为 101。
    return ModbusServerContext(devices={7: ModbusDeviceContext(
        co=ModbusSequentialDataBlock(101, [True, False]),
        di=ModbusSequentialDataBlock(101, [False, True]),
        hr=ModbusSequentialDataBlock(101, [42, 43]),
        ir=ModbusSequentialDataBlock(101, [99, 100]),
    )}, single=False)


def trace(sending, packet):
    print(('SERVER TX ' if sending else 'SERVER RX ') + packet.hex(' ').upper(), flush=True)
    return packet


async def verify(binary, mode):
    fds = []
    stop = threading.Event()
    errors = []
    bridge = None
    if mode == 'tcp':
        server = ModbusTcpServer(context(), address=('127.0.0.1', 0), trace_packet=trace)
    else:
        a, sa = os.openpty()
        b, sb = os.openpty()
        fds = [a, sa, b, sb]
        for fd in [sa, sb]:
            tty.setraw(fd)
        # 保持两端 slave guard 存活；测试量较小，写入受控的 PTY 转发。
        def forward():
            try:
                with selectors.DefaultSelector() as selector:
                    selector.register(a, selectors.EVENT_READ, b)
                    selector.register(b, selectors.EVENT_READ, a)
                    while not stop.is_set():
                        for key, _ in selector.select(0.05):
                            data = os.read(key.fd, 4096)
                            while data:
                                n = os.write(key.data, data)
                                data = data[n:]
            except Exception as exc:
                errors.append(exc)
        bridge = threading.Thread(target=forward, daemon=True)
        bridge.start()
        target = os.ttyname(sb)
        server = ModbusSerialServer(context(), port=os.ttyname(sa), baudrate=19200,
                                   parity='N', bytesize=8, stopbits=2, trace_packet=trace)
    process = None
    try:
        await server.serve_forever(background=True)
        if mode == 'tcp':
            target = '127.0.0.1:' + str(server.transport.sockets[0].getsockname()[1])
        print(f'{mode.upper()} target={target}; Unit ID=7; address=100', flush=True)
        process = await asyncio.create_subprocess_exec(str(binary.resolve()), mode, target)
        result = await asyncio.wait_for(process.wait(), timeout=10)
        assert result == 0, f'Rust master exit={result}'
        assert not errors, errors
    finally:
        if process and process.returncode is None:
            process.kill()
            await process.wait()
        await server.shutdown()
        stop.set()
        if bridge:
            await asyncio.to_thread(bridge.join, 1)
            assert not bridge.is_alive(), 'PTY bridge did not stop'
        for fd in fds:
            os.close(fd)


async def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--binary', type=Path, default=Path('target/debug/examples/client_demo'))
    args = parser.parse_args()
    assert pymodbus.__version__ == '3.11.3'
    assert serial.VERSION == '3.5'
    print(f'PyModbus {pymodbus.__version__}; pyserial {serial.VERSION}', flush=True)
    for mode in ['tcp', 'rtu']:
        await verify(args.binary, mode)


if __name__ == '__main__':
    asyncio.run(main())
