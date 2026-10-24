import {ModelDraft} from "./pointDraft";
import { expect, it, vi } from "vitest";
import { WorkspaceStore, projectNavigation, navigation, documentPosition, isEndpointApplied } from "./model";
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
  role: "slave",
  transport: { kind: "tcp", address: "127.0.0.1", port: 1502 },
  units: [
    {
      id: "unit-a",
      name: "单元A",
      station: 1,
      enabled: true,
      model: { id: model.id, },
      device: "device-a",
    },
    {
      id: "unit-b",
      name: "单元B",
      station: 2,
      enabled: true,
      model: { id: model.id, },
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

  });
  expect(adapter.execute).toHaveBeenCalledTimes(1);
  expect(adapter.execute).toHaveBeenCalledWith(
    expect.objectContaining({
      action: expect.objectContaining({
        kind: "saveEndpoint",

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
    code: "SAVE_FAILED",
    message: "存储失败",
  });
  const store = new WorkspaceStore(adapter);
  store.edit(endpoint);
  await expect(
    store.execute({ kind: "saveEndpoint", endpoint,  }),
  ).rejects.toMatchObject({ code: "SAVE_FAILED" });
  expect(store.drafts.has(endpoint.id)).toBe(true);
  expect(store.error).toBe("存储失败");
  expect(store.busy).toBe(false);
});
it("应用确认刷新只读值，保留本地草稿", () => {
  const sheets = new SheetViews();
  const data = sheets.get("a", "measurement", model);
  data.updateCell(0, "draft", 77);
  expect(
    sheets.get("a", "measurement", model, {
      model,
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
    catalog: { ...view().catalog, endpoints: [{ ...endpoint, }] },
  });
  const store = new WorkspaceStore(adapter);
  store.edit(endpoint);
  const pending = store.execute({
    kind: "saveEndpoint",
    endpoint,

  });
  store.edit({ ...endpoint, name: "保存期间继续编辑" });
  confirm({ objectId: endpoint.id, sequence: "2" });
  await pending;
  expect(store.endpoint(endpoint.id)?.name).toBe("保存期间继续编辑");
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
it("保存提交当前模型且不携带版本，等待期间的后续编辑仍未保存", async () => {
  const adapter = client();
  let complete!: (value: unknown) => void;
  adapter.previewPoints = vi.fn(() => new Promise(resolve => { complete = resolve; })) as WorkspaceClient["previewPoints"];
  const store = new WorkspaceStore(adapter);
  await store.refresh();
  const draft = store.modelDraft(model.id)!;
  draft.change(s => { s.name = "保存的内容"; });
  const task = store.saveModelDraft(model.id);
  draft.change(s => { s.name = "后续编辑"; });
  complete({ model: { ...model, name: "保存的内容" }, errors: [], previews: [] });
  await task;
  expect(adapter.execute).toHaveBeenCalledWith(expect.objectContaining({ action: { kind: "saveModel", model: { ...model, name: "保存的内容" } } }));
  expect(draft.name).toBe("后续编辑");
  expect(draft.dirty).toBe(true);
});
it("应用已停机后失败也刷新同配置序号的实际状态", async () => {
  const adapter = client();
  const store = new WorkspaceStore(adapter);
  await store.refresh();
  vi.mocked(adapter.execute).mockRejectedValue({
    code: "SAVE_FAILED",
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

      resetIncompatible: false,
    }),
  ).rejects.toMatchObject({ code: "SAVE_FAILED" });
  expect(store.view?.applicationStates?.[endpoint.id]).toBe("stopped");
});

it("已应用状态比较实际内容，覆盖保存模型后提示待应用", () => {
  const catalog = view().catalog;
  catalog.applied = [structuredClone(endpoint)];
  for (const unit of endpoint.units) catalog.values[unit.id] = { model: structuredClone(model), raw: {}, stale: false };
  expect(isEndpointApplied(catalog, endpoint,{[endpoint.id]:Object.fromEntries(endpoint.units.map(u=>[u.id,true]))})).toBe(true);
  catalog.models[0]!.points[0]!.address = 10;
  expect(isEndpointApplied(catalog, endpoint,{[endpoint.id]:Object.fromEntries(endpoint.units.map(u=>[u.id,false]))})).toBe(false);
  expect(catalog.values["unit-a"]!.model!.points[0]!.address).toBe(0);
});

it("同名模拟器在文件夹移动后仍以稳定身份定位，并保留草稿和选中Unit",()=>{
 const catalog=view().catalog;const copy=structuredClone(catalog.endpoints[0]!);copy.id="other";copy.units=copy.units.map(u=>({...u,id:`other-${u.id}`}));catalog.endpoints.push(copy);
 catalog.navigation={folders:[{id:"folder",name:"目录",parent:null}],members:{}};
 const before=navigation(catalog,"simulators");expect(new Set(before.paths).size).toBe(before.paths.length);
 catalog.navigation.members[copy.id]="folder";const after=navigation(catalog,"simulators");expect([...after.targets].find(([p,t])=>t.id===copy.id && !t.unit)?.[0]).toContain("目录/");
 expect([...after.targets.values()].filter(t=>t.id===copy.id && t.unit).map(t=>t.unit)).toEqual(copy.units.map(u=>u.id));
});

it("核心草稿比较的迟到结果不能覆盖新的映射或展示编辑",async()=>{
 let finishFirst!:(v:Record<string,boolean>)=>void;let finishSecond!:(v:Record<string,boolean>)=>void;
 const adapter={...client(),inspectEndpoint:vi.fn().mockImplementationOnce(()=>new Promise(resolve=>{finishFirst=resolve;})).mockImplementationOnce(()=>new Promise(resolve=>{finishSecond=resolve;}))};
 const store=new WorkspaceStore(adapter);await store.refresh();
 store.edit({...endpoint,name:"展示名"});store.edit({...endpoint,units:endpoint.units.map(u=>({...u,station:10}))});
 finishSecond({"unit-a":false});await Promise.resolve();finishFirst({"unit-a":true});await Promise.resolve();expect(store.draftMatches.get(endpoint.id)).toEqual({"unit-a":false});
});

it("模拟器与资源分别导航，文件夹、Unit和设备引用保留稳定身份",()=>{
 const catalog=view().catalog;
 catalog.navigation={folders:[{id:"folder",name:"现场",parent:null}],members:{[endpoint.id]:"folder"}};
 const simulators=projectNavigation(catalog,"simulators");
 const resources=projectNavigation(catalog,"resources");
 expect(simulators.targets.get("现场/端点A/")).toEqual({kind:"endpoint",id:endpoint.id});
 expect(simulators.targets.get("现场/端点A/Unit 1 · 单元A")).toEqual({kind:"endpoint",id:endpoint.id,unit:"unit-a"});
 expect(simulators.paths.some(path=>/^(模拟器|采集模型|设备|文档资源)\//.test(path))).toBe(false);
 expect(resources.targets.get("设备/PCS/端点A · 单元A")).toEqual({kind:"endpoint",id:endpoint.id,unit:"unit-a"});
 expect(resources.targets.get("采集模型/模型A")).toEqual({kind:"model",id:model.id});
 expect(resources.targets.has("现场/端点A/")).toBe(false);
 expect([...resources.targets.values()].some(target=>target.kind==="resource")).toBe(true);
});

it("无后端时模拟器显示空态，资源仍可访问内置文档",()=>{
 expect(projectNavigation(undefined,"simulators").paths).toEqual([]);
 const resources=projectNavigation(undefined,"resources");
 expect(resources.paths).toContain("采集模型/");
 expect(resources.paths).toContain("设备/");
 expect([...resources.targets.values()].some(target=>target.kind==="resource")).toBe(true);
});

it("分组模型与端点可同归文件夹，模型入口保留稳定身份",()=>{
 const catalog=view().catalog;
 catalog.navigation={folders:[{id:"batch",name:"分组",parent:null}],members:{},modelMembers:{[model.id]:"batch"}};
 const tree=projectNavigation(catalog,"resources");
 expect(tree.targets.get("采集模型/分组/模型A")).toEqual({kind:"model",id:model.id});
 expect(tree.targets.has("采集模型/模型A")).toBe(false);
});

it("导航摘要不创建空模型草稿，打开时按需加载并忽略过期异步内容",async()=>{
 const adapter=client();const summary=view();summary.catalog.models[0]!.points=[];
 adapter.query=vi.fn(async()=>summary);
 let resolve:((m:CollectionModel)=>void)|undefined;
 adapter.loadModel=vi.fn(()=>new Promise<CollectionModel>(r=>{resolve=r;}));
 const store=new WorkspaceStore(adapter);await store.refresh();
 expect(store.modelDraft(model.id)).toBeUndefined();
 expect(adapter.loadModel).toHaveBeenCalledTimes(1);
 const pending=store.loadTarget("model",model.id);
 const newer=view("2");newer.catalog.models[0]!.points=[];store.receive(newer);
 resolve?.({...model,name:"过期内容"});await pending;
 expect(store.view?.catalog.models[0]?.name).toBe(model.name);
 expect(store.view?.catalog.models[0]?.points).toEqual([]);
 adapter.loadModel=vi.fn(async()=>structuredClone(model));await store.loadTarget("model",model.id);
 expect(store.modelDraft(model.id)?.name).toBe(model.name);
 expect(store.view?.catalog.models[0]?.points).toHaveLength(1);
});

it("只加载打开端点的模型和值，重新接收导航时保留页面内容并刷新缓存",async()=>{
 const adapter=client();const summary=view();summary.catalog.models[0]!.points=[];summary.catalog.applied=[structuredClone(endpoint)];
 adapter.query=vi.fn(async()=>summary);adapter.loadModel=vi.fn(async()=>structuredClone(model));
 adapter.loadEndpointValues=vi.fn(async()=>({"unit-a":{model:structuredClone(model),raw:{voltage:{kind:"unsigned" as const,value:"20"}},stale:false}}));
 const store=new WorkspaceStore(adapter);await store.refresh();expect(adapter.loadModel).not.toHaveBeenCalled();
 await store.loadTarget("endpoint",endpoint.id);expect(adapter.loadEndpointValues).toHaveBeenCalledWith(endpoint.id);
 expect(store.view?.catalog.values["unit-a"]?.raw.voltage?.value).toBe("20");
 const next=structuredClone(summary);next.sequence="2";store.receive(next);
 expect(store.view?.catalog.models[0]?.points).toHaveLength(1);
 await store.loadTarget("endpoint",endpoint.id,true);
 expect(store.view?.catalog.values["unit-a"]?.raw.voltage?.value).toBe("20");
});

it("本地新模型草稿不发已保存模型查询",async()=>{
 const adapter=client();adapter.loadModel=vi.fn(async()=>{throw Error("不应读取尚未保存的模型");});
 const store=new WorkspaceStore(adapter);await store.refresh();
 store.modelDrafts.set("local",new ModelDraft("local","本地草稿",[],store.changed,false));
 await store.loadTarget("model","local");expect(adapter.loadModel).not.toHaveBeenCalled();expect(store.error).toBe("");
});
it('keeps empty model folders separate and preserves stable identities after rename',()=>{
 const c=view().catalog;c.navigation={folders:[{id:'models',name:'模型目录',scope:'models',parent:null,icon:'pcs',color:'#399f91'},{id:'sim',name:'模拟器目录',scope:'simulators',parent:null}],members:{'endpoint-a':'sim'},modelMembers:{'model-a':'models'}};
 const resources=navigation(c,'resources'),simulators=navigation(c,'simulators');
 expect([...resources.targets.values()].some(t=>t.kind==='folder'&&t.id==='models')).toBe(true);
 expect([...simulators.targets.values()].some(t=>t.kind==='folder'&&t.id==='models')).toBe(false);
 c.navigation.folders[0]!.name='重命名';c.navigation.folders[0]!.color='#c58d36';
 const next=navigation(c,'resources');expect([...next.targets.values()].find(t=>t.kind==='model')).toEqual({kind:'model',id:'model-a'});
 expect([...next.targets.keys()].some(p=>p.includes('重命名'))).toBe(true);
});
