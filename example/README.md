# PyModbus TCP连接示例

先在ESSim中保存点表和模拟器配置，点击「应用配置」，再点击 ▶ 启动。

在仓库根目录执行：

```bash
python3 -m venv .venv
source .venv/bin/activate
python -m pip install -r example/requirements.txt
python example/modbus_tcp_client.py
```

默认连接 `127.0.0.1:1502`、Unit 1，保持连接并等待命令：

- `yk`：遥控，写coil，值为0（关）或1（开）。
- `yt`：遥调，写holding，值为原始U16整数0～65535。
- `read`：输入地址或范围，读取holding寄存器。
- `help`：帮助；`q`：退出。也可按 `Ctrl+C` 关闭连接。

例如输入 `yt` → 地址 `1000` → 值 `25` ，填完值按回车即发送，无需额外确认。
所有地址都是零基地址，必须与模型中的数据区、类型和地址一致。写入点须勾选「允许外部写入」，保存并应用配置。
命令按上述数据区发送，不会根据工作台的四遥分类自动推断地址或控制语义。
`yt`不转换倍率、字序、浮点数或多寄存器类型；只用于单寄存器原始U16测试。
协议写入确认不代表真实设备动作完成；超时后不要盲目重复写入，先读取核对。

参数示例：

```bash
python example/modbus_tcp_client.py --host 127.0.0.1 --port 1502 --unit 1
# 持续读取holding地址1、2，每次读取后等待1秒
python example/modbus_tcp_client.py --poll --address 1 --count 2 --interval 1
# 只读一次
python example/modbus_tcp_client.py --once --address 1 --count 2
```

地址和值是测试约定，不是设备标准地址。连续读取的整个地址范围必须已在模型中定义。
交互模式的协议异常会显示并返回命令提示；连接失败或通信异常会退出。关闭PyModbus自动请求重试，写入不自动重发。

地址支持闭区间，例如 `1000-1005`（包含两端，共6个地址）。
写入时输入一个值会填充整个范围；也可输入 `1,2,3,4,5,6`，按地址顺序赋值。
填完值按回车即发送，同时逐地址显示本次写入内容，无需输入 y/n。

每次使用一个Modbus请求：`yt`最多123个寄存器（0x10），`yk`最多1968个coil（0x0F），`read`最多125个寄存器（0x03）。
单地址仍使用0x06／0x05。超限范围要求重新填写，不自动拆成多批；整个范围都必须已定义且允许写入。

交互测试：`python3 -m unittest discover -s example`（不访问设备）。

## RTU 串口交互测试

安装上面的依赖（包含 `pymodbus==3.11.3`、`pyserial==3.5`）后运行：

```sh
python example/modbus_rtu_client.py
```

脚本先显示串口路径和系统设备名称，输入序号选择；输入 `r` 刷新，也可直接填写完整路径，输入 `q` 退出。不会自动打开列表中的串口。

```sh
# 只列出设备，不打开
python example/modbus_rtu_client.py --list
# 直接指定实际转换器路径；双方参数必须一致
python example/modbus_rtu_client.py --port /dev/cu.usbserial-实际编号 --baud 19200 --parity E --stopbits 1 --unit 1
# 与ESSim新建默认参数19200/8/N/1匹配
python example/modbus_rtu_client.py --port /dev/cu.usbserial-实际编号 --stopbits 1
```

脚本默认 `19200/8/N/2`、Unit 1、请求超时1秒；数据位固定8。打开成功只代表本机端口打开，需读取来确认从站响应。ESSim和脚本应分别使用两个已接线转换器的串口；不能同时打开同一个串口。在ESSim中保存并应用RTU从站配置，再点击“打开串口”。

| 命令 | 操作 |
| --- | --- |
| `read` / `rh` | 读取holding原始U16，最多125个寄存器 |
| `ri` | 读取input原始U16，最多125个寄存器 |
| `rc` | 读取coil，最多2000位 |
| `rd` | 读取discrete，最多2000位 |
| `yk` | 写coil，0或1，最多1968位 |
| `yt` | 写holding原始U16，0～65535，最多123个寄存器 |
| `help` / `q` | 帮助／退出 |

例如 `yt` → `1000-1003` → `11,22,33,44`，值填完回车立即发送，不再输入y/n；输入一个值则填充整个范围。随后 `read` → `1000-1003` 核对。地址为零基闭区间，示例数值是测试约定，必须与已应用点表一致。单点使用05／06，范围使用0F／10；不拆批，不自动重试，不发送广播。`yt`不做倍率、字序或浮点转换。

协议拒绝会显示异常并返回命令提示；通信异常或超时结束脚本并关闭串口，写入可能已执行，先读取核对。`q`、EOF或Ctrl+C均关闭串口。

软件验证：`python -m unittest discover -s example`，8项通过；实际运行本脚本连接ESSim正式RTU从站的PTY，完成四区读取、单点／范围写入、只读点拒绝与原值保留、未知地址异常、EOF关闭重开和错误站号超时。复现：

```sh
cargo build -p essim-infrastructure --features tcp-runtime,vendored --example tcp_slave --offline
python scripts/verify_rtu_example.py
```

这些是PTY软件证据，不代表已验证真实RS-485转换器、接线和时序。
