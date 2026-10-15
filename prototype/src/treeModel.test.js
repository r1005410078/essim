import test from "node:test";
import assert from "node:assert/strict";
import { demoWorkspace } from "./domain.js";
import {
  buildResourceTree,
  canonicalTreePath,
  sanitizeTreeLabel,
  selectionPath,
  visibleTreeLabel,
} from "./treeModel.js";

function labelsUnder(tree, parent) {
  const prefix = parent ? `${parent}/` : "";
  const depth = parent ? parent.split("/").length : 0;
  const seen = new Set();
  const labels = [];
  for (const raw of tree.paths) {
    const path = raw.replace(/\/$/, "");
    if (parent && path !== parent && !path.startsWith(prefix)) continue;
    const parts = path.split("/");
    if (parts.length <= depth) continue;
    const child = parts.slice(0, depth + 1).join("/");
    if (seen.has(child)) continue;
    seen.add(child);
    labels.push(visibleTreeLabel(child));
  }
  return labels;
}

test("示例树按项目顺序显示短名称", () => {
  const tree = buildResourceTree(demoWorkspace(), "device");
  assert.deepEqual(labelsUnder(tree, ""), ["点表库", "模拟设备"]);
  assert.deepEqual(labelsUnder(tree, "点表库"), [
    "PCS Modbus 标准表",
    "BMS Modbus 标准表",
  ]);
  assert.deepEqual(labelsUnder(tree, "模拟设备"), [
    "PCS 01",
    "PCS 02",
    "BMS 01",
    "EMS 01",
  ]);
  const pcs = tree.device.get("demo-pcs");
  assert.deepEqual(labelsUnder(tree, pcs), ["192.168.86.43:502"]);
  const comm = tree.comm.get("comm-pcs-1");
  assert.deepEqual(labelsUnder(tree, comm), ["PCS Modbus 标准表"]);
  const ems = tree.device.get("demo-ems");
  assert.deepEqual(labelsUnder(tree, ems), [
    "192.168.86.43:502",
    "192.168.86.44:502",
  ]);
  assert.equal(
    tree.actions.get(comm).title,
    "Modbus TCP · 监听 192.168.86.43:502",
  );
  for (const path of tree.paths) assert.equal(visibleTreeLabel(path).includes("demo-"), false);
});

test("同名端点和斜线名称不会撞路径", () => {
  const tree = buildResourceTree(
    {
      templates: [],
      devices: [
        {
          id: "d",
          name: "PCS/01",
          type: "PCS",
          comms: [
            { id: "a", name: "甲", protocol: "Modbus TCP", role: "从站", host: "10.0.0.1", port: 502, templateId: "" },
            { id: "b", name: "乙", protocol: "Modbus TCP", role: "从站", host: "10.0.0.1", port: 502, templateId: "" },
          ],
        },
      ],
    },
    "device",
  );
  assert.equal(visibleTreeLabel(tree.device.get("d")), "PCS／01");
  assert.equal(tree.device.get("d").split("/").length, 2);
  assert.notEqual(tree.comm.get("a"), tree.comm.get("b"));
  assert.equal(visibleTreeLabel(tree.comm.get("a")), "10.0.0.1:502");
  assert.equal(visibleTreeLabel(tree.bound.get("a")), "未绑定");
  assert.equal(tree.actions.get(tree.bound.get("a")).open.sel.initialTab, "通信配置");
  assert.equal(sanitizeTreeLabel("  "), "未命名");
});

test("协议标记只放在通讯段上", () => {
  const tree = buildResourceTree(
    {
      templates: [{ id: "t", name: "104表", protocol: "IEC 60870-5-104" }],
      devices: [
        {
          id: "d",
          name: "测控",
          type: "箱变测控",
          comms: [
            {
              id: "c",
              name: "104从站",
              protocol: "IEC 60870-5-104",
              role: "从站",
              host: "10.0.0.8",
              port: 2404,
              templateId: "t",
            },
          ],
        },
      ],
    },
    "device",
  );
  const comm = tree.comm.get("c");
  const table = tree.bound.get("c");
  assert.equal(comm.includes("\u200c"), true);
  assert.equal(visibleTreeLabel(table).includes("\u200c"), false);
  assert.equal(tree.actions.get(comm).title, "IEC 60870-5-104 · 监听 10.0.0.8:2404");
  assert.equal(tree.actions.get(tree.library.get("t")).title, "IEC 60870-5-104");
});

test("选中路径区分点表库和通讯下的同一张表", () => {
  const tree = buildResourceTree(demoWorkspace(), "device");
  const library = selectionPath(tree, {
    focus: "template",
    templateId: "tpl-pcs-modbus",
  });
  const bound = selectionPath(tree, {
    focus: "template",
    templateId: "tpl-pcs-modbus",
    deviceId: "demo-pcs",
    commId: "comm-pcs-1",
  });
  assert.equal(library.startsWith("点表库/"), true);
  assert.equal(bound.startsWith("模拟设备/"), true);
  assert.equal(visibleTreeLabel(library), visibleTreeLabel(bound));
  assert.equal(
    selectionPath(tree, { focus: "device", deviceId: "demo-bms" }),
    tree.device.get("demo-bms"),
  );
});

test("结构签名忽略点值，策略树保留脚本名", () => {
  const project = demoWorkspace();
  const first = buildResourceTree(project, "device");
  project.templates[0].points = [{ id: "p", name: "有功功率", value: 123 }];
  const second = buildResourceTree(project, "device");
  assert.equal(first.signature, second.signature);
  project.devices[0].comms[0].host = "10.1.1.1";
  assert.notEqual(first.signature, buildResourceTree(project, "device").signature);
  const strategy = buildResourceTree(
    { scripts: [{ id: "peak", name: "peak_valley.js" }] },
    "strategy",
  );
  assert.equal(visibleTreeLabel(strategy.script.get("peak")), "peak_valley.js");
  assert.deepEqual(labelsUnder(strategy, ""), [
    "peak_valley.js",
    "场景数据 24 h",
    "运行记录",
  ]);
});

test("目录视图路径带结尾斜杠时仍对应同一动作", () => {
  const tree = buildResourceTree(demoWorkspace(), "device");
  const devicePath = tree.device.get("demo-pcs");
  const commPath = tree.comm.get("comm-pcs-1");
  assert.equal(canonicalTreePath(`${devicePath}/`), devicePath);
  assert.equal(tree.actions.get(canonicalTreePath(`${devicePath}/`)).open.sel.focus, "device");
  assert.equal(tree.actions.get(canonicalTreePath(`${commPath}/`)).title, "Modbus TCP · 监听 192.168.86.43:502");
  assert.equal(visibleTreeLabel(`${commPath}/`), "192.168.86.43:502");
  assert.equal(canonicalTreePath("点表库"), "点表库");
});

test('用户文件树保留嵌套空文件夹，同名文件独立，移动不改变模型引用', async () => {
  const { buildProjectFileTree } = await import('./treeModel.js');
  const p = {folders:[{id:'f',name:'场站'},{id:'sub',name:'空目录',parentId:'f'}], templates:[{id:'a',name:'同名',folderId:'f'},{id:'b',name:'同名',folderId:'f'}],devices:[{id:'s',name:'模拟器',comms:[{templateId:'a'}]}]};
  const tree = buildProjectFileTree(p);
  assert.equal(tree.actions.size,5);
  assert.equal(tree.directories.length,2);
  assert.equal(new Set(tree.paths).size,5);
  const moved = buildProjectFileTree({...p,templates:p.templates.map(t => ({...t,folderId:''}))});
  assert.equal([...moved.actions.values()].find(a => a.kind==='device').item.comms[0].templateId,'a');
});
