"""独立互操作验证。依赖固定 PyModbus 3.11.3；连接本机工作台或 headless 示例。"""
import argparse
import pymodbus
from pymodbus.client import ModbusTcpClient
parser = argparse.ArgumentParser()
parser.add_argument('--port', type=int, default=1502)
parser.add_argument('--expected', type=int, default=650)
parser.add_argument('--unit-id', type=int, choices=range(1, 248), default=1)
args = parser.parse_args()
assert pymodbus.__version__ == '3.11.3'
print(f'PyModbus {pymodbus.__version__}; 127.0.0.1:{args.port}; Unit ID={args.unit_id}; expected={args.expected}')
def trace(sending, packet):
    print(('TX ' if sending else 'RX ') + packet.hex(' ').upper())
    return packet
client = ModbusTcpClient('127.0.0.1', port=args.port, timeout=1, retries=0, trace_packet=trace)
try:
    assert client.connect()
    read = client.read_holding_registers(100, count=1, device_id=args.unit_id)
    assert not read.isError() and read.registers == [args.expected], read
    print(f'PASS SOC={read.registers[0]}')
    for name, response, code in [
        ('out-of-range', client.read_holding_registers(101, count=1, device_id=args.unit_id), 2),
        ('read-only FC06', client.write_register(100, 123, device_id=args.unit_id), 2),
        ('read-only FC16', client.write_registers(100, [123], device_id=args.unit_id), 2),
        ('unsupported FC04', client.read_input_registers(100, count=1, device_id=args.unit_id), 1),
    ]:
        assert response.isError() and response.exception_code == code, response
        print(f'PASS {name}: exception {code}')
    client.close()
    # Server polls disconnected sockets; retry connect/read rather than assuming
    # that a second connection can immediately replace a still-active client.
    import time
    time.sleep(0.05)
    assert client.connect()
    response = client.read_holding_registers(100, count=1, device_id=args.unit_id)
    assert not response.isError() and response.registers == [args.expected], response
    print('PASS reconnect; rejected writes did not mutate SOC')
finally:
    client.close()
