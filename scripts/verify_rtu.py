"""独立 RTU 软件互操作：两对 Unix PTY 转发，不替代真实 RS-485 验收。
依赖固定 PyModbus 3.11.3 / pyserial 3.5；--binary 自动验证 headless，
--desktop 打印串口路径后按 stdin 的 650 / 700 / quit 命令验证桌面。
"""
import argparse
import os
from pathlib import Path
import selectors
import subprocess
import threading
import time
import tty
import pymodbus
import serial
from pymodbus.client import ModbusSerialClient

parser = argparse.ArgumentParser()
group = parser.add_mutually_exclusive_group(required=True)
group.add_argument('--binary', type=Path)
group.add_argument('--desktop', action='store_true')
args = parser.parse_args()
assert pymodbus.__version__ == '3.11.3'
assert serial.VERSION == '3.5'
master_a, slave_a = os.openpty()
master_b, slave_b = os.openpty()
for fd in [slave_a, slave_b]:
    tty.setraw(fd)
for fd in [master_a, master_b]:
    os.set_blocking(fd, False)
server_port, client_port = os.ttyname(slave_a), os.ttyname(slave_b)
finished = threading.Event()
bridge_errors = []

def forward():
    try:
        with selectors.DefaultSelector() as selector:
            selector.register(master_a, selectors.EVENT_READ, master_b)
            selector.register(master_b, selectors.EVENT_READ, master_a)
            while not finished.is_set():
                for key, _ in selector.select(0.05):
                    data = os.read(key.fd, 4096)
                    while data and not finished.is_set():
                        try:
                            n = os.write(key.data, data)
                            data = data[n:]
                        except BlockingIOError:
                            time.sleep(0.001)
    except Exception as exc:
        bridge_errors.append(exc)

thread = threading.Thread(target=forward, daemon=True)
thread.start()
print(f'PyModbus={pymodbus.__version__} pyserial={serial.VERSION}; 19200 N8 2; slave=7; address=100', flush=True)
print(f'SERVER_PORT={server_port}', flush=True)
print(f'CLIENT_PORT={client_port}', flush=True)

def trace(sending, packet):
    print(('TX ' if sending else 'RX ') + packet.hex(' ').upper(), flush=True)
    return packet

client = ModbusSerialClient(client_port, baudrate=19200, parity='N', bytesize=8, stopbits=2,
                           timeout=0.2, retries=0, trace_packet=trace)
process = None

def check(expected):
    assert client.connect(), '客户端串口打开失败'
    result = client.read_holding_registers(100, count=1, device_id=7)
    assert not result.isError() and result.registers == [expected], result
    print(f'PASS SOC={expected}', flush=True)
    for result, code in [
        (client.read_holding_registers(101, count=1, device_id=7), 2),
        (client.write_register(100, 123, device_id=7), 2),
        (client.write_registers(100, [123], device_id=7), 2),
        (client.read_input_registers(100, count=1, device_id=7), 1),
    ]:
        assert result.isError() and result.exception_code == code, result
    # 广播写入不应修改只读 SOC；no_response_expected 避免主站等待广播回复。
    client.write_register(100, 123, device_id=0, no_response_expected=True)
    time.sleep(0.06)
    result = client.read_holding_registers(100, count=1, device_id=7)
    assert not result.isError() and result.registers == [expected], result
    print('PASS exceptions and broadcast did not mutate SOC', flush=True)
    assert not bridge_errors, bridge_errors

def output_line(expected):
    with selectors.DefaultSelector() as selector:
        selector.register(process.stdout, selectors.EVENT_READ)
        assert selector.select(3), '无界面入口未按时输出'
    line = process.stdout.readline().strip()
    print('SERVER ' + line, flush=True)
    assert expected in line, line

try:
    if args.binary:
        process = subprocess.Popen([str(args.binary.resolve()), server_port, '19200', 'N', '8', '2', '7', '50'],
                                   stdin=subprocess.PIPE, stdout=subprocess.PIPE, text=True, bufsize=1)
        output_line('OPEN RTU')
        check(650)
        process.stdin.write('700\n'); process.stdin.flush(); output_line('ACK SOC=700')
        check(700)
        process.stdin.write('quit\n'); process.stdin.flush()
        started = time.monotonic()
        assert process.wait(timeout=1) == 0
        print(f'PASS STOP elapsed={time.monotonic()-started:.3f}s', flush=True)
    else:
        print('桌面配置以上 SERVER_PORT、站号7和19200/N/8/2，启动后输入期望值650或700；quit退出验证桥。', flush=True)
        import sys
        for line in sys.stdin:
            if line.strip() == 'quit':
                break
            check(int(line))
            print('READY', flush=True)
finally:
    client.close()
    if process and process.poll() is None:
        process.terminate()
        process.wait(timeout=3)
    finished.set(); thread.join(timeout=1)
    for fd in [master_a, slave_a, master_b, slave_b]:
        os.close(fd)
