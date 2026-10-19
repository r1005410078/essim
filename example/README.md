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
