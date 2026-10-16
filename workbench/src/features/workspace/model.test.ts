import { expect, it, vi } from "vitest";
import { WorkspaceStore, navigation, documentPosition } from "./model";
import { SheetViews } from "./sheets";
import type {
  CollectionModel,
  EndpointConfig,
  WorkspaceClient,
  WorkspaceView,
} from "../../shared/api/contracts";
const model: CollectionModel = {
  id: "model-a",
  name: "模型A",
  version: "1",
  points: [
    {
      id: "voltage",
      name: "测试电压",
      category: "measurement",
      area: "holding",
      address: 0,
      valueType: "u16",
      scale: "0.1",
      unit: "V",
      initial: { kind: "unsigned", value: "10" },
      initialEngineering: "1",
      byteOrder: "big",
      wordOrder: "big",
      note: "",
      slaveWritable: false,
      masterPeriodMs: 1000,
    },
  ],
};
const endpoint: EndpointConfig = {
  id: "endpoint-a",
  name: "端点A",
  revision: "1",
  appliedRevision: null,
  role: "slave",
  transport: { kind: "tcp", address: "127.0.0.1", port: 1502 },
  units: [
    {
      id: "unit-a",
      name: "单元A",
      station: 1,
      enabled: true,
      model: { id: model.id, version: "1" },
      device: "device-a",
    },
    {
      id: "unit-b",
      name: "单元B",
      station: 2,
      enabled: true,
      model: { id: model.id, version: "1" },
      device: null,
    },
  ],
};
function view(sequence = "1"): WorkspaceView {
  return {
    sequence,
    catalog: {
      models: [structuredClone(model)],
      endpoints: [structuredClone(endpoint)],
      devices: [{ id: "device-a", name: "PCS", kind: "PCS" }],
      applied: [],
      values: {},
    },
    capabilities: {
      configuration: true,
      execution: false,
      recording: false,
      replay: false,
      sessionOnly: true,
      reason: "未接入",
    },
  };
}
function client(): WorkspaceClient {
  return {
    query: vi.fn(async () => view()),
    execute: vi.fn(async () => ({
      objectId: endpoint.id,
      revision: "2",
      sequence: "2",
    })),
    subscribe: vi.fn(async () => vi.fn(async () => {})),
  };
}
it("Unit切换不发命令，独立草稿不会被查询覆盖，保存与应用分开", async () => {
  const adapter = client();
  const store = new WorkspaceStore(adapter);
  await store.refresh();
  const draft = structuredClone(endpoint);
  draft.name = "未保存";
  store.edit(draft);
  store.selectUnit(endpoint.id, "unit-b");
  store.selectUnit(endpoint.id, "unit-a");
  expect(adapter.execute).not.toHaveBeenCalled();
  await store.refresh();
  expect(store.endpoint(endpoint.id)?.name).toBe("未保存");
  await store.execute({
    kind: "saveEndpoint",
    endpoint: draft,
    expectedRevision: "1",
  });
  expect(adapter.execute).toHaveBeenCalledTimes(1);
  expect(adapter.execute).toHaveBeenCalledWith(
    expect.objectContaining({
      action: expect.objectContaining({
        kind: "saveEndpoint",
        expectedRevision: "1",
      }),
    }),
  );
  expect(store.drafts.size).toBe(0);
  expect(store.view?.catalog.applied).toEqual([]);
});
it("树路径变化和设备快捷入口仍定位同一个端点与稳定单元", () => {
  const catalog = view().catalog;
  const resource = navigation(catalog, "resources");
  const simulator = navigation(catalog, "simulators");
  const shortcut = [...resource.targets.values()].find(
    (target) => target.unit === "unit-a",
  );
  expect(shortcut).toEqual(
    [...simulator.targets.values()].find((target) => target.unit === "unit-a"),
  );
  const originalPath = [...simulator.targets].find(
    ([, target]) => target.unit === "unit-a",
  )?.[0];
  catalog.endpoints[0]!.units[0]!.name = "改名";
  const next = navigation(catalog, "simulators");
  expect(next.targets.has(originalPath!)).toBe(false);
  expect(
    [...next.targets.values()].find((target) => target.unit === "unit-a"),
  ).toEqual(shortcut);
});
it("超过JS整数精度的快照只接受较新完整目录", () => {
  const store = new WorkspaceStore(client());
  store.receive(view("9007199254740993"));
  store.receive(view("9007199254740992"));
  expect(store.view?.sequence).toBe("9007199254740993");
  expect(store.view?.capabilities.execution).toBe(false);
  expect(store.view?.capabilities.recording).toBe(false);
});
it("不同单元和四遥保留自己的NovaSheet草稿，查询与分类不会重置编辑", () => {
  const sheets = new SheetViews();
  const a = sheets.get("unit-a", "measurement", model);
  const b = sheets.get("unit-b", "measurement", model);
  a.updateCell(0, "draft", 77);
  expect(a.getRows(0, 0)[0]?.draft).toBe(77);
  expect(b.getRows(0, 0)[0]?.draft).toBe("10");
  expect(sheets.get("unit-a", "indication", model).getRows(0, 0)).toEqual([]);
  expect(sheets.get("unit-a", "measurement", model)).toBe(a);
  expect(a.getRows(0, 0)[0]?.draft).toBe(77);
  expect(a.getRows(0, 0)[0]?.confirmed).toBeNull();
});
it("保存失败保留草稿和明确错误，不显示假成功", async () => {
  const adapter = client();
  vi.mocked(adapter.execute).mockRejectedValue({
    code: "REVISION_CONFLICT",
    message: "版本冲突",
  });
  const store = new WorkspaceStore(adapter);
  store.edit(endpoint);
  await expect(
    store.execute({ kind: "saveEndpoint", endpoint, expectedRevision: "0" }),
  ).rejects.toMatchObject({ code: "REVISION_CONFLICT" });
  expect(store.drafts.has(endpoint.id)).toBe(true);
  expect(store.error).toBe("版本冲突");
  expect(store.busy).toBe(false);
});
it("应用确认刷新只读值，保留本地草稿", () => {
  const sheets = new SheetViews();
  const data = sheets.get("a", "measurement", model);
  data.updateCell(0, "draft", 77);
  expect(
    sheets.get("a", "measurement", model, {
      model: { id: model.id, version: "1" },
      raw: { voltage: { kind: "unsigned", value: "10" } },
      stale: false,
    }),
  ).toBe(data);
  expect(data.getRows(0, 0)[0]?.confirmed).toBe("10");
  expect(data.getRows(0, 0)[0]?.draft).toBe(77);
});
it("保存等待期间的新编辑不被较早的保存确认清除", async () => {
  const adapter = client();
  let confirm!: (value: {
    objectId: string;
    revision: string;
    sequence: string;
  }) => void;
  vi.mocked(adapter.execute).mockImplementation(
    () =>
      new Promise((resolve) => {
        confirm = resolve;
      }),
  );
  vi.mocked(adapter.query).mockResolvedValue({
    ...view("2"),
    catalog: { ...view().catalog, endpoints: [{ ...endpoint, revision: "2" }] },
  });
  const store = new WorkspaceStore(adapter);
  store.edit(endpoint);
  const pending = store.execute({
    kind: "saveEndpoint",
    endpoint,
    expectedRevision: "1",
  });
  store.edit({ ...endpoint, name: "保存期间继续编辑" });
  confirm({ objectId: endpoint.id, revision: "2", sequence: "2" });
  await pending;
  expect(store.endpoint(endpoint.id)?.name).toBe("保存期间继续编辑");
  expect(store.endpoint(endpoint.id)?.revision).toBe("2");
  expect(store.drafts.has(endpoint.id)).toBe(true);
});
it("关闭欢迎页后，新文档不再引用已关闭的面板", () => {
  expect(documentPosition(["project", "inspector", "observations"])).toEqual({
    referencePanel: "observations",
    direction: "above",
  });
  expect(documentPosition(["project", "endpoint:a", "observations"])).toEqual({
    referencePanel: "endpoint:a",
    direction: "within",
  });
});
it("主从与四遥共同确定运行草稿权限，主站没有伪造的采集或待发值", () => {
  const sheets = new SheetViews();
  const types = (
    role: "master" | "slave",
    category: (typeof model.points)[number]["category"],
  ) =>
    sheets.get(
      role,
      category,
      { ...model, points: model.points.map((p) => ({ ...p, category })) },
      undefined,
      role,
    );
  expect(
    types("master", "measurement")
      .getSchema()
      .fields.find((f) => f.id === "draft")?.type,
  ).toBe("readonly");
  expect(
    types("slave", "control")
      .getSchema()
      .fields.find((f) => f.id === "draft")?.type,
  ).toBe("readonly");
  expect(
    types("slave", "measurement")
      .getSchema()
      .fields.find((f) => f.id === "draft")?.type,
  ).toBe("text");
  expect(types("master", "adjustment").getCell(0, "draft")).toBe("");
  expect(types("master", "measurement").getCell(0, "draft")).toBeNull();
});
it("保存预览期间版本推进时拒绝用旧内容创建新版本", async () => {
  const adapter = client();
  let complete!: (value: unknown) => void;
  adapter.previewPoints = vi.fn(
    () =>
      new Promise((resolve) => {
        complete = resolve;
      }),
  ) as WorkspaceClient["previewPoints"];
  const store = new WorkspaceStore(adapter);
  await store.refresh();
  const draft = store.modelDraft(model.id)!;
  draft.change((s) => {
    s.name = "未保存";
  });
  const task = store.saveModelDraft(model.id);
  draft.saved("2", draft.content());
  complete({ model, errors: [], previews: [] });
  await expect(task).rejects.toThrow("版本已变化");
  expect(adapter.execute).not.toHaveBeenCalled();
});
it("应用已停机后失败也刷新同配置序号的实际状态", async () => {
  const adapter = client();
  const store = new WorkspaceStore(adapter);
  await store.refresh();
  vi.mocked(adapter.execute).mockRejectedValue({
    code: "REVISION_CONFLICT",
    message: "停机期间配置变化",
  });
  vi.mocked(adapter.query).mockResolvedValue({
    ...view(),
    applicationStates: { [endpoint.id]: "stopped" },
  });
  await expect(
    store.execute({
      kind: "apply",
      id: endpoint.id,
      expectedRevision: "1",
      resetIncompatible: false,
    }),
  ).rejects.toMatchObject({ code: "REVISION_CONFLICT" });
  expect(store.view?.applicationStates?.[endpoint.id]).toBe("stopped");
});
