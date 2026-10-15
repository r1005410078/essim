import test from "node:test";
import assert from "node:assert/strict";
import {
  validateComm,
  validateDevice,
  demoWorkspace,
  migrateProject,
  peersFor,
  templateDeviceNames,
} from "./domain.js";

test("设备名称和自定义类型必填", () => {
  assert.ok(validateDevice({ name: " ", type: "PCS" }));
  assert.ok(validateDevice({ name: "设备", type: "自定义", customType: "" }));
  assert.equal(validateDevice({ name: "PCS 01", type: "PCS" }), "");
});
test("拒绝越界端口和无效IP，保留草稿", () => {
  const base = {
    name: "从站",
    role: "从站",
    protocol: "Modbus TCP",
    host: "127.0.0.1",
    port: 502,
    interval: 1000,
  };
  assert.ok(validateComm({ ...base, host: "999.1.1.1" }));
  assert.ok(validateComm({ ...base, port: 0 }));
  assert.equal(validateComm(base), "");
});
test("采集表协议与通讯不一致时拒绝保存", () => {
  assert.match(
    validateComm(
      {
        name: "从站",
        role: "从站",
        protocol: "Modbus TCP",
        host: "192.168.86.43",
        port: 502,
        interval: 1000,
      },
      { protocol: "IEC 60870-5-104" },
    ),
    /协议/,
  );
});
test("两台 PCS 共用一张采集表，地址和运行归属仍然分开", () => {
  const project = demoWorkspace();
  const pcs = project.devices.filter((d) => d.name.startsWith("PCS"));
  assert.equal(pcs[0].comms[0].templateId, pcs[1].comms[0].templateId);
  assert.notEqual(pcs[0].comms[0].host, pcs[1].comms[0].host);
  assert.deepEqual(templateDeviceNames(project, pcs[0].comms[0].templateId), [
    "PCS 01",
    "PCS 02",
    "EMS 01",
  ]);
});
test("主站按目标地址匹配正在监听的从站", () => {
  const project = demoWorkspace();
  const ems = project.devices.find((d) => d.type === "EMS");
  const peers = peersFor(project, ems.comms[0]);
  assert.equal(peers.length, 1);
  assert.equal(peers[0].device.name, "PCS 01");
  assert.equal(peersFor(project, ems.comms[1])[0].device.name, "BMS 01");
});
test("旧采集组迁移为通讯，点表抽成模板", () => {
  const project = migrateProject({
    devices: [
      {
        id: "d",
        name: "PCS 01",
        type: "PCS",
        groups: [
          {
            id: "g",
            name: "主采集组",
            protocol: "Modbus TCP",
            role: "客户端 / 主站",
            host: "127.0.0.1",
            port: 1502,
            interval: 1000,
            points: [{ id: "p", name: "有功功率" }],
          },
        ],
      },
    ],
  });
  assert.equal(project.devices[0].groups, undefined);
  assert.equal(project.devices[0].comms[0].role, "主站");
  assert.equal(project.devices[0].comms[0].templateId, project.templates[0].id);
  assert.equal(project.templates[0].points[0].category, "遥测");
  assert.equal(project.templates[0].points[0].name, "有功功率");
});
