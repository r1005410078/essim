#!/usr/bin/env python3
"""独立字节级 IEC 104 封装验证，不依赖同库客户端；不代表完整兼容性认证。

CA=47、IOA=100、值=12.5 是教学约定。报文格式依据官方 lib60870 文档及示例。
"""
import argparse
import json
import platform
import selectors
import socket
import subprocess
from pathlib import Path


def exact(peer, count):
    data = bytearray()
    while len(data) < count:
        chunk = peer.recv(count - len(data))
        if not chunk:
            raise RuntimeError("peer disconnected before complete APDU")
        data.extend(chunk)
    return bytes(data)


def apdu(peer):
    header = exact(peer, 2)
    if header[0] != 0x68 or not 4 <= header[1] <= 253:
        raise RuntimeError(f"invalid APDU header: {header.hex()}")
    return exact(peer, header[1])


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--binary', required=True)
    args = parser.parse_args()
    with socket.socket() as reservation:
        reservation.bind(('127.0.0.1', 0))
        port = reservation.getsockname()[1]
    process = subprocess.Popen([str(Path(args.binary).resolve()), f'127.0.0.1:{port}'],
                               stdin=subprocess.PIPE, stdout=subprocess.PIPE, stderr=subprocess.PIPE)
    try:
        with selectors.DefaultSelector() as selector:
            selector.register(process.stdout, selectors.EVENT_READ)
            if not selector.select(10):
                raise RuntimeError('server readiness timed out')
            ready = process.stdout.readline().decode().strip()
            if not ready.startswith('READY '):
                raise RuntimeError(f'server failed before ready: {ready}')
        frames = []
        with socket.create_connection(('127.0.0.1', port), timeout=3) as peer:
            peer.settimeout(3)
            peer.sendall(bytes.fromhex('68 04 07 00 00 00'))  # STARTDT act
            assert apdu(peer) == bytes.fromhex('0b 00 00 00')
            request = bytes.fromhex('68 0e 00 00 00 00 64 01 06 00 2f 00 00 00 00 14')
            peer.sendall(request[:5])
            peer.settimeout(0.05)
            try:
                unexpected = peer.recv(1)
                raise AssertionError(f'response before complete request: {unexpected.hex()}')
            except socket.timeout:
                pass
            peer.settimeout(3)
            peer.sendall(request[5:])
            while len(frames) < 3:
                frame = apdu(peer)
                if frame[0] & 1 == 0:
                    frames.append(frame[4:])
            assert frames == [bytes.fromhex(s) for s in (
                '64 01 07 00 2f 00 00 00 00 14',
                '0d 01 14 00 2f 00 64 00 00 00 00 48 41 00',
                '64 01 0a 00 2f 00 00 00 00 14')]
            # 确认三条 I 帧，再将 TESTFR 与 STOPDT 合并写入同一 TCP 字节流。
            peer.sendall(bytes.fromhex('68 04 01 00 06 00 68 04 43 00 00 00 68 04 13 00 00 00'))
            controls = set()
            while len(controls) < 2:
                frame = apdu(peer)
                if frame[0] & 3 == 3:
                    controls.add(frame)
            assert controls == {bytes.fromhex('83 00 00 00'), bytes.fromhex('23 00 00 00')}
        process.stdin.write(b'quit\n')
        process.stdin.flush()
        _, stderr = process.communicate(timeout=5)
        assert process.returncode == 0, stderr.decode()
        with socket.socket() as rebound:
            rebound.bind(('127.0.0.1', port))
        print(json.dumps({'result': 'PASS', 'platform': platform.platform(),
                          'scope': ['STARTDT', 'fragmented interrogation', 'ACT_CON', 'M_ME_NC_1', 'ACT_TERM', 'merged TESTFR/STOPDT', 'quit/rebind'],
                          'asdus': [frame.hex(' ') for frame in frames],
                          'limit': 'raw-byte subset; no real EMS, no complete conformance or scale claim'}, ensure_ascii=False))
    finally:
        if process.poll() is None:
            process.kill()
            process.communicate(timeout=5)


if __name__ == '__main__':
    main()
