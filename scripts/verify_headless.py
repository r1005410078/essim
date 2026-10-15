"""复用独立 PyModbus 检查脚本验证 headless 的更新确认及退出端口释放。"""
import argparse
from pathlib import Path
import socket
import subprocess
import sys
parser=argparse.ArgumentParser()
parser.add_argument('binary',type=Path)
parser.add_argument('--port',type=int,default=1503)
args=parser.parse_args()
root=Path(__file__).resolve().parents[1]
process=subprocess.Popen([str(args.binary.resolve()),str(args.port)],stdin=subprocess.PIPE,stdout=subprocess.PIPE,text=True)
try:
    line=process.stdout.readline().strip()
    print(line,flush=True)
    assert line.startswith('LISTENING'),line
    for value in [650,700]:
        if value==700:
            process.stdin.write('700\n');process.stdin.flush()
            ack=process.stdout.readline().strip();print(ack,flush=True);assert ack=='ACK SOC=700'
        subprocess.run([sys.executable,str(root/'scripts/verify_modbus.py'),'--port',str(args.port),'--expected',str(value)],check=True)
    process.stdin.write('quit\n');process.stdin.flush()
    assert process.stdout.readline().strip()=='STOPPED'
    assert process.wait(timeout=2)==0
    with socket.socket() as probe:
        probe.bind(('127.0.0.1',args.port))
    print('PASS orderly stop and port released',flush=True)
finally:
    if process.poll() is None:
        process.kill();process.wait()
