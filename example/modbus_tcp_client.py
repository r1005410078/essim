#!/usr/bin/env python3
"""交互控制 ESSim TCP 从站；默认地址是截图测试约定，非设备标准地址。"""

import argparse
from datetime import datetime
import math
import sys
import time


def ask_addresses(max_count):
    """读取闭区间；不拆分超出单个Modbus请求的写入，避免部分成功。"""
    while True:
        text = input("零基地址或范围(如1000-1005)> ").strip()
        parts = text.split("-")
        try:
            if len(parts) not in (1, 2):
                raise ValueError()
            start = int(parts[0], 10)
            end = int(parts[-1], 10)
            if 0 <= start <= end <= 65535 and end - start + 1 <= max_count:
                return start, end - start + 1
        except ValueError:
            pass
        print(f"请输入0～65535内的单个地址或递增范围，每次最多{max_count}个地址。")


def ask_values(count, maximum):
    while True:
        text = input(f"值(0～{maximum}，一个值统一填充，或按地址顺序输入{count}个值，逗号分隔)> ").strip()
        try:
            values = [int(item.strip(), 10) for item in text.replace("，", ",").split(",")]
            if len(values) in (1, count) and all(0 <= value <= maximum for value in values):
                return values * count if len(values) == 1 else values
        except ValueError:
            pass
        print(f"请输入一个值或恰好{count}个值，每个值必须为0～{maximum}的十进制整数。")


def command_mode(client, unit):
    print("命令：yk 遥控(coil)，yt 遥调(holding U16)，read 读取holding，help 帮助，q 退出")
    print("地址均为零基地址；写入点须允许外部写入。")
    while True:
        command = input("命令> ").strip().lower()
        if command in ("q", "quit", "exit"):
            return 0
        if command in ("help", "?"):
            print("yk：写coil，0关／1开；yt：写holding原始U16；read：读取holding。地址支持1000-1005闭区间。")
            continue
        if command not in ("yk", "yt", "read"):
            print("请输入 yk、yt、read、help 或 q。")
            continue
        address, count = ask_addresses(1968 if command == "yk" else 123 if command == "yt" else 125)
        if command == "read":
            result = client.read_holding_registers(address, count=count, device_id=unit)
            if result.isError():
                print(f"读取失败：{result}")
            else:
                for offset, value in enumerate(result.registers):
                    print(f"holding[{address + offset}] = {value}")
            continue
        values = ask_values(count, 1 if command == "yk" else 65535)
        area = "coil" if command == "yk" else "holding"
        print(f"即将写入 Unit {unit}，{area}，共{count}个地址：")
        for offset, value in enumerate(values):
            print(f"  {area}[{address + offset}] ← {value}")
        # 写请求只发送一次；异常或超时不自动重试可能已执行的操作。
        if command == "yk":
            if count == 1:
                result = client.write_coil(address, bool(values[0]), device_id=unit)
            else:
                result = client.write_coils(address, [bool(value) for value in values], device_id=unit)
        else:
            if count == 1:
                result = client.write_register(address, values[0], device_id=unit)
            else:
                result = client.write_registers(address, values, device_id=unit)
        if result.isError():
            print(f"写入未获成功确认：{result}；结果可能不确定，请读取核对，不要直接重复发送。")
        else:
            print(f"写入成功：Unit {unit}，{area}[{address}～{address + count - 1}]，共{count}个（协议确认，不代表设备动作完成）")


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--host", default="127.0.0.1", help="模拟器监听地址")
    parser.add_argument("--port", type=int, default=1502, help="TCP 端口（默认1502）")
    parser.add_argument("--unit", type=int, default=1, help="Unit ID（默认1）")
    parser.add_argument("--address", type=int, default=0, help="保持寄存器零基地址（默认0）")
    parser.add_argument("--count", type=int, default=1, help="读取寄存器数量（1～125）")
    parser.add_argument("--interval", type=float, default=1, help="两次读取之间的等待秒数（默认1）")
    mode = parser.add_mutually_exclusive_group()
    mode.add_argument("--poll", action="store_true", help="持续读取保持寄存器；默认进入交互命令模式")
    mode.add_argument("--once", action="store_true", help="只读取一次后退出")
    args = parser.parse_args()
    if not 1 <= args.port <= 65535:
        parser.error("端口应为1～65535")
    if not 1 <= args.unit <= 247:
        parser.error("Unit ID应为1～247")
    if not 1 <= args.count <= 125 or not 0 <= args.address <= 65536 - args.count:
        parser.error("数量应为1～125，且零基地址范围不能超过65535")
    if not math.isfinite(args.interval) or args.interval <= 0:
        parser.error("读取间隔必须为有限正数")

    try:
        from pymodbus.client import ModbusTcpClient
        from pymodbus.exceptions import ModbusException
    except ImportError:
        print('缺少依赖，请执行：python -m pip install "pymodbus==3.11.3"', file=sys.stderr)
        return 1

    try:
        with ModbusTcpClient(args.host, port=args.port, timeout=3, retries=0) as client:
            if not client.connect():
                print("连接失败，请确认模拟器已应用配置并启动，且地址／端口正确。", file=sys.stderr)
                return 1
            print(f"连接成功：{args.host}:{args.port}，Unit {args.unit}")
            if not args.poll and not args.once:
                return command_mode(client, args.unit)
            if not args.once:
                print(f"每次读取后等待 {args.interval:g} 秒继续，按 Ctrl+C 停止。")
            while True:
                result = client.read_holding_registers(
                    args.address, count=args.count, device_id=args.unit
                )
                if result.isError():
                    print(f"Modbus异常：{result}；请检查Unit、数据区及地址是否已定义。", file=sys.stderr)
                    return 1
                timestamp = datetime.now().strftime("%H:%M:%S")
                for address, value in enumerate(result.registers, start=args.address):
                    print(f"[{timestamp}] holding[{address}] = {value} (0x{value:04X})", flush=True)
                if args.once:
                    break
                time.sleep(args.interval)
    except (KeyboardInterrupt, EOFError):
        print("\n已退出，连接已关闭。")
    except (ModbusException, OSError) as error:
        print(f"通信失败：{error}；写入结果可能不确定，请读取核对，不自动重试。", file=sys.stderr)
        return 1
    return 0


if __name__ == "__main__":
    sys.exit(main())
