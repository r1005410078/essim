import { pointCategory } from "./telecontrol.js";

export const types = [
  "PCS",
  "BMS",
  "EMS",
  "箱变测控",
  "电表",
  "温控设备",
  "消防设备",
  "自定义",
];
export const protocols = ["Modbus TCP", "IEC 60870-5-104", "IEC 61850 MMS"];

export function defaultRole(type) {
  return type === "EMS" ? "主站" : "从站";
}
export function validateDevice(d) {
  if (!d.name?.trim()) return "请输入设备名称";
  if (d.type === "自定义" && !d.customType?.trim())
    return "请输入自定义设备类型";
  return "";
}
export function validateTemplate(t) {
  if (!t.name?.trim()) return "请输入采集表名称";
  if (!protocols.includes(t.protocol)) return "请选择协议";
  return "";
}
export function validateComm(c, template) {
  if (!c.name?.trim()) return "请输入通讯名称";
  const ip = String(c.host ?? "").split(".");
  if (
    ip.length !== 4 ||
    ip.some((v) => !/^\d{1,3}$/.test(v) || Number(v) > 255)
  )
    return "请输入有效 IPv4 地址";
  if (!Number.isInteger(Number(c.port)) || c.port < 1 || c.port > 65535)
    return "端口应为 1–65535 的整数";
  if (!Number.isFinite(Number(c.interval)) || c.interval < 100)
    return "采集周期至少为 100 ms";
  if (c.role !== "从站" && c.role !== "主站") return "请选择通信角色";
  if (!protocols.includes(c.protocol)) return "请选择协议";
  if (template && template.protocol !== c.protocol)
    return "采集表协议需与通讯协议一致";
  return "";
}
export function endpointLabel(c) {
  return `${c.role === "从站" ? "监听" : "目标"} ${c.host}:${c.port}`;
}
export function commLabel(c) {
  return `${c.name} · ${endpointLabel(c)}`;
}
export const pointFields = [
  { id: "name", name: "点名称", type: "text", width: 180 },
  { id: "address", name: "地址 / 引用", type: "text", width: 170 },
  { id: "type", name: "数据类型", type: "text", width: 120 },
  { id: "unit", name: "单位", type: "text", width: 90 },
  { id: "scale", name: "倍率", type: "number", width: 90 },
  { id: "value", name: "初始值", type: "number", width: 110 },
  { id: "access", name: "访问", type: "text", width: 90 },
];
export function demoPoints(type = "PCS") {
  return (
    type === "BMS"
      ? [
          ["SOC", "%", 65],
          ["总电压", "V", 768],
          ["总电流", "A", 0],
          ["最高温度", "°C", 28],
        ]
      : [
          ["有功功率", "kW", 0],
          ["无功功率", "kVar", 0],
          ["交流电压", "V", 400],
          ["运行状态", "", 1],
        ]
  )
    .map(([name, unit, value], i) => ({
      id: crypto.randomUUID(),
      name,
      address: String(100 + i * 2),
      category: name === "运行状态" ? "遥信" : "遥测",
      type: name === "运行状态" ? "Bool" : "Float32",
      unit,
      scale: 1,
      value,
      access: "只读",
    }))
    .concat([
      {
        id: crypto.randomUUID(),
        name: "启动控制",
        category: "遥控",
        address: "200",
        type: "Bool",
        unit: "",
        scale: 1,
        value: 0,
        access: "读写",
      },
      {
        id: crypto.randomUUID(),
        name: "目标功率",
        category: "遥调",
        address: "300",
        type: "Float32",
        unit: "kW",
        scale: 1,
        value: 0,
        access: "读写",
      },
    ]);
}
export function blankTemplate(partial = {}) {
  return {
    id: crypto.randomUUID(),
    name: "新采集表",
    protocol: "Modbus TCP",
    deviceType: "PCS",
    points: [],
    ...partial,
  };
}
export function blankComm(type, partial = {}) {
  const role = partial.role || defaultRole(type);
  return {
    id: crypto.randomUUID(),
    name: role === "主站" ? "连接" : "Modbus TCP 从站",
    protocol: "Modbus TCP",
    role,
    host: "192.168.86.43",
    port: 502,
    interval: 1000,
    unitId: 1,
    commonAddress: 1,
    iedName: "ESSimIED",
    templateId: "",
    ...partial,
  };
}
export function templatePoints(project, templateId) {
  return project.templates.find((t) => t.id === templateId)?.points || [];
}
export function templateRefs(project, templateId) {
  const refs = [];
  for (const device of project.devices || []) {
    for (const comm of device.comms || []) {
      if (comm.templateId === templateId)
        refs.push({
          deviceId: device.id,
          deviceName: device.name,
          commId: comm.id,
          commName: comm.name,
        });
    }
  }
  return refs;
}
export function templateDeviceNames(project, templateId) {
  return [
    ...new Set(templateRefs(project, templateId).map((r) => r.deviceName)),
  ];
}
export function peersFor(project, comm) {
  if (!comm) return [];
  const peers = [];
  for (const device of project.devices || []) {
    for (const other of device.comms || []) {
      if (
        other.id !== comm.id &&
        other.role !== comm.role &&
        other.protocol === comm.protocol &&
        other.host === comm.host &&
        Number(other.port) === Number(comm.port)
      )
        peers.push({ device, comm: other });
    }
  }
  return peers;
}
function normalizeComm(c) {
  return {
    ...c,
    id: c.id || crypto.randomUUID(),
    name: c.name || "通讯",
    role: c.role?.includes("主站") ? "主站" : "从站",
    protocol: c.protocol || "Modbus TCP",
    host: c.host || "127.0.0.1",
    port: c.port ?? 502,
    interval: c.interval ?? 1000,
    templateId: c.templateId || "",
  };
}
function normalizeTemplate(t) {
  return {
    ...t,
    id: t.id || crypto.randomUUID(),
    name: t.name || "采集表",
    protocol: t.protocol || "Modbus TCP",
    deviceType: t.deviceType || "PCS",
    points: (t.points || []).map((p) => ({
      ...p,
      id: p.id || crypto.randomUUID(),
      category: pointCategory(p),
    })),
  };
}
export function migrateProject(raw) {
  const created = [];
  const devices = (raw.devices || []).map((device) => {
    if (Array.isArray(device.comms)) {
      const { groups, ...rest } = device;
      return { ...rest, comms: device.comms.map(normalizeComm) };
    }
    const comms = (device.groups || []).map((g) => {
      const template = normalizeTemplate({
        id: crypto.randomUUID(),
        name: `${device.name} ${g.name || "采集表"}`.trim(),
        protocol: g.protocol,
        deviceType: device.type || "自定义",
        points: g.points || [],
      });
      created.push(template);
      const { points, ...comm } = g;
      return normalizeComm({ ...comm, templateId: template.id });
    });
    const { groups, ...rest } = device;
    return { ...rest, comms };
  });
  return {
    ...raw,
    templates: [...(raw.templates || []).map(normalizeTemplate), ...created],
    devices,
  };
}
export function demoWorkspace() {
  const pcs = blankTemplate({
    id: "tpl-pcs-modbus",
    name: "PCS Modbus 标准表",
    protocol: "Modbus TCP",
    deviceType: "PCS",
    points: demoPoints("PCS"),
  });
  const bms = blankTemplate({
    id: "tpl-bms-modbus",
    name: "BMS Modbus 标准表",
    protocol: "Modbus TCP",
    deviceType: "BMS",
    points: demoPoints("BMS"),
  });
  return {
    templates: [pcs, bms],
    devices: [
      {
        id: "demo-pcs",
        name: "PCS 01",
        type: "PCS",
        description: "储能变流器 · 将电池直流电与电网交流电互相转换",
        comms: [
          blankComm("PCS", {
            id: "comm-pcs-1",
            name: "Modbus TCP 从站",
            role: "从站",
            host: "192.168.86.43",
            port: 502,
            templateId: pcs.id,
          }),
        ],
      },
      {
        id: "demo-pcs-2",
        name: "PCS 02",
        type: "PCS",
        description: "储能变流器 · 与 PCS 01 使用同一张 Modbus 采集表",
        comms: [
          blankComm("PCS", {
            id: "comm-pcs-2",
            name: "Modbus TCP 从站",
            role: "从站",
            host: "192.168.86.45",
            port: 502,
            templateId: pcs.id,
          }),
        ],
      },
      {
        id: "demo-bms",
        name: "BMS 01",
        type: "BMS",
        description: "电池管理系统 · 监测电池状态与充放电边界",
        comms: [
          blankComm("BMS", {
            id: "comm-bms-1",
            name: "Modbus TCP 从站",
            role: "从站",
            host: "192.168.86.44",
            port: 502,
            templateId: bms.id,
          }),
        ],
      },
      {
        id: "demo-ems",
        name: "EMS 01",
        type: "EMS",
        description: "能量管理系统 · 作为主站连接 PCS 与 BMS",
        comms: [
          blankComm("EMS", {
            id: "comm-ems-pcs",
            name: "连接 PCS",
            role: "主站",
            host: "192.168.86.43",
            port: 502,
            templateId: pcs.id,
          }),
          blankComm("EMS", {
            id: "comm-ems-bms",
            name: "连接 BMS",
            role: "主站",
            host: "192.168.86.44",
            port: 502,
            templateId: bms.id,
          }),
        ],
      },
    ],
  };
}
