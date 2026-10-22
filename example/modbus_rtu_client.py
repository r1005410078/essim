#!/usr/bin/env python3
"""交互式Modbus RTU主站；只访问用户选择的串口，地址均为零基测试地址。"""
import argparse
import math
import sys

from modbus_tcp_client import ask_addresses, ask_values


def select_port(list_ports):
    """显示系统设备信息；刷新不打开串口，也允许手动路径。"""
    while True:
        ports = sorted(list_ports(), key=lambda port: port.device)
        print("\n可用串口：")
        for index, port in enumerate(ports, 1):
            details = " · ".join(str(value) for value in
                                 [port.description, getattr(port, "manufacturer", None),
                                  getattr(port, "serial_number", None)] if value and value != "n/a")
            print(f"  {index}. {port.device} — {details or '名称未知'}")
        if not ports:
            print("  未发现设备；虚拟串口可手动填写完整路径。")
        choice = input("选择序号／完整串口路径，r 刷新，q 退出> ").strip()
        if choice.lower() in ("q", "quit", "exit"):
            return None
        if choice.lower() == "r":
            continue
        if choice.isdecimal():
            if 1 <= int(choice) <= len(ports):
                return ports[int(choice) - 1].device
            print("序号不存在，请重新选择。")
        elif choice:
            return choice


def command_mode(client, unit):
    from pymodbus.exceptions import ModbusException
    reads = {
        "read": ("holding", client.read_holding_registers, 125),
        "rh": ("holding", client.read_holding_registers, 125),
        "ri": ("input", client.read_input_registers, 125),
        "rc": ("coil", client.read_coils, 2000),
        "rd": ("discrete", client.read_discrete_inputs, 2000),
    }
    help_text = ("read/rh 读holding，ri 读input，rc 读coil，rd 读discrete；"
                 "yk 写coil，yt 写holding原始U16；help 帮助，q 退出")
    print(help_text)
    print("地址支持闭区间（如1000-1003）；写值填完回车即发送，不再确认。写入不自动重试。")
    while True:
        command = input("命令> ").strip().lower()
        if command in ("q", "quit", "exit"):
            return 0
        if command in ("help", "?"):
            print(help_text)
            continue
        if command not in reads and command not in ("yk", "yt"):
            print("未知命令。" + help_text)
            continue
        maximum = reads[command][2] if command in reads else 1968 if command == "yk" else 123
        address, count = ask_addresses(maximum)
        try:
            if command in reads:
                area, read, _ = reads[command]
                result = read(address, count=count, device_id=unit)
                if result.isError():
                    print(f"读取失败：{result}")
                else:
                    values = result.bits[:count] if area in ("coil", "discrete") else result.registers
                    for offset, value in enumerate(values):
                        print(f"{area}[{address + offset}] = {value}")
                continue
            values = ask_values(count, 1 if command == "yk" else 65535)
            area = "coil" if command == "yk" else "holding"
            for offset, value in enumerate(values):
                print(f"  Unit {unit} {area}[{address + offset}] ← {value}")
            # 单次请求，不拆批、不自动重发；由库生成RTU帧及CRC。
            if command == "yk":
                result = (client.write_coil(address, bool(values[0]), device_id=unit) if count == 1
                          else client.write_coils(address, list(map(bool, values)), device_id=unit))
            else:
                result = (client.write_register(address, values[0], device_id=unit) if count == 1
                          else client.write_registers(address, values, device_id=unit))
            if result.isError():
                if getattr(result, "exception_code", None) is not None:
                    print(f"写入被拒绝：{result}；请核对地址、权限和类型。")
                else:
                    print(f"写入结果不确定：{result}；请读取核对，不要直接重发。")
            else:
                print(f"写入成功（协议确认）：Unit {unit} {area}[{address}～{address + count - 1}]，共{count}个；不代表设备动作完成。")
        except (ModbusException, OSError) as error:
            print(f"通信失败：{error}；写入可能已执行，请读取核对，不自动重发。", file=sys.stderr)
            return 1


def main(argv=None):
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--port", help="完整串口路径；省略时交互选择")
    parser.add_argument("--baud", type=int, default=19200, help="波特率（默认19200）")
    parser.add_argument("--parity", type=str.upper, choices=["N", "E", "O"], default="N")
    parser.add_argument("--stopbits", type=int, choices=[1, 2], default=2)
    parser.add_argument("--unit", type=int, default=1, help="站号1～247，不发送广播")
    parser.add_argument("--timeout", type=float, default=1, help="请求超时秒数（默认1）")
    parser.add_argument("--list", action="store_true", help="列出串口后退出，不打开设备")
    args = parser.parse_args(argv)
    if not 1 <= args.unit <= 247:
        parser.error("Unit须为1～247")
    if args.baud <= 0 or not math.isfinite(args.timeout) or args.timeout <= 0:
        parser.error("波特率须为正整数，超时须为有限正数")
    if args.port is not None and not args.port.strip():
        parser.error("串口路径不能为空")
    try:
        from serial.tools.list_ports import comports
        from pymodbus.client import ModbusSerialClient
        from pymodbus.exceptions import ModbusException
    except ImportError:
        print("缺少依赖，请执行：python -m pip install -r example/requirements.txt", file=sys.stderr)
        return 1
    try:
        if args.list:
            for port in sorted(comports(), key=lambda port: port.device):
                print(f"{port.device} — {port.description} [{port.hwid}]")
            return 0
        port = args.port or select_port(comports)
        if port is None:
            return 0
        with ModbusSerialClient(port, baudrate=args.baud, bytesize=8, parity=args.parity,
                                stopbits=args.stopbits, timeout=args.timeout, retries=0) as client:
            if not client.connect():
                print("串口打开失败，请检查路径、权限及是否已被其他程序占用。", file=sys.stderr)
                return 1
            print(f"串口已打开：{port}，{args.baud}/8/{args.parity}/{args.stopbits}，Unit {args.unit}，超时{args.timeout:g}s")
            print("打开串口不代表从站已响应；请执行读取验证。参数必须与从站完全一致。")
            return command_mode(client, args.unit)
    except (KeyboardInterrupt, EOFError):
        print("\n已退出，串口已关闭。")
        return 0
    except (ModbusException, OSError) as error:
        print(f"串口通信失败：{error}", file=sys.stderr)
        return 1


if __name__ == "__main__":
    sys.exit(main())
