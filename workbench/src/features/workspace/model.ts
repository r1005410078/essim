import {resourceNavigation} from "../resources/catalog";
import type {
  Action,
  ApiError,
  Catalog,
  EndpointConfig,
  WorkspaceClient,
  WorkspaceView,
} from "../../shared/api/contracts";
import { newerView } from "../../shared/api/contracts";
import { ModelDraft, modelRows } from "./pointDraft";
export type Target = {
  kind: "endpoint" | "model" | "device" | "folder" | "resource";
  id: string;
  unit?: string;
  anchor?:string;
};
/** 路径属于树视图，业务身份通过显式映射取得。相同实例可有多个路径。 */
export function navigation(
  catalog: Catalog,
  section: "resources" | "simulators",
) {
  const targets = new Map<string, Target>();
  const leaf = (parent: string, name: string, id: string, target: Target) => {
    const label = name.replaceAll("/", "／");
    let path = parent ? `${parent}/${label}` : label;
    // 同名显示节点才追加身份以消歧，正常行不暴露冗长UUID。
    if (targets.has(path))
      path = `${parent ? `${parent}/` : ""}${label} · ${encodeURIComponent(id)}`;
    targets.set(path, target);
    return path;
  };
  const paths: string[] = [];
  if (section === "resources") {
    for (const model of catalog.models)
      paths.push(
        leaf("采集模型", model.name, model.id, { kind: "model", id: model.id }),
      );
    for (const device of catalog.devices) {
      const path = leaf("设备", device.name, device.id, {
        kind: "device",
        id: device.id,
      });
      paths.push(`${path}/`);
      targets.set(`${path}/`, { kind: "device", id: device.id });
      for (const endpoint of catalog.endpoints)
        for (const unit of endpoint.units.filter(
          (unit) => unit.device === device.id,
        ))
          paths.push(
            leaf(path, `${endpoint.name} · ${unit.name}`, unit.id, {
              kind: "endpoint",
              id: endpoint.id,
              unit: unit.id,
            }),
          );
    }
  } else {
    const folderPaths = new Map<string,string>();
    const folderPath = (id: string): string => {
      const cached=folderPaths.get(id); if(cached) return cached;
      const folder=catalog.navigation?.folders.find(f=>f.id===id); if(!folder) return "";
      const path=leaf(folder.parent ? folderPath(folder.parent) : "",folder.name,folder.id,{kind:"folder",id});
      folderPaths.set(id,path); paths.push(`${path}/`);targets.set(`${path}/`,{kind:"folder",id});return path;
    };
    for(const folder of catalog.navigation?.folders ?? []) folderPath(folder.id);
    for (const endpoint of catalog.endpoints) {
      const parent = leaf(
        folderPaths.get(catalog.navigation?.members[endpoint.id] ?? "") ?? "",
        endpoint.name,
        endpoint.id,
        { kind: "endpoint", id: endpoint.id },
      );
      paths.push(`${parent}/`);
      targets.set(`${parent}/`, { kind: "endpoint", id: endpoint.id });
      for (const unit of endpoint.units)
        paths.push(
          leaf(parent, unit.name===`Unit ${unit.station}`?unit.name:`Unit ${unit.station} · ${unit.name}`, unit.id, {
            kind: "endpoint",
            id: endpoint.id,
            unit: unit.id,
          }),
        );
    }
  }
  return { paths, targets };
}
/** 合并项目树根目录；只改显示路径，设备、文件夹和Unit身份继续来自显式映射。 */
export function projectNavigation(catalog:Catalog|undefined){
 const resources=catalog?navigation(catalog,"resources"):{paths:[],targets:new Map<string,Target>()};const simulators=catalog?navigation(catalog,"simulators"):{paths:[],targets:new Map<string,Target>()};const documents=resourceNavigation();
 return {paths:["模拟器/",...simulators.paths.map(path=>`模拟器/${path}`),"采集模型/","设备/",...resources.paths,...documents.paths],targets:new Map([...documents.targets,...resources.targets,...[...simulators.targets].map(([path,target])=>[`模拟器/${path}`,target] as const)])};
}
export function errorMessage(error: unknown): string {
  const value = error as Partial<ApiError>;
  return value?.message ?? String(error);
}
/** 后端快照与界面草稿分离；切换单元只更新选择。 */
export class WorkspaceStore {
  view: WorkspaceView | null = null;
  error = "";
  lastError: ApiError | null = null;
  busy = false;
  private listeners = new Set<() => void>();
  private changeCount = 0;
  readonly focusModelIssue=new Map<string,(issue:ApiError)=>void>();
  readonly runtimePoint=new Map<string,string>();
  readonly draftMatches=new Map<string,Record<string,boolean>>();
  readonly drafts = new Map<string, EndpointConfig>();
  readonly modelDrafts = new Map<string, ModelDraft>();
  modelDraft(id: string) {
    let draft = this.modelDrafts.get(id);
    if (!draft) {
      const model = this.view?.catalog.models.find((m) => m.id === id);
      if (!model) return;
      draft = new ModelDraft(
        id,
        model.name,
        modelRows(model),
        this.changed,
      );
      this.modelDrafts.set(id, draft);
    }
    return draft;
  }
  async saveModelDraft(id: string) {
    const draft = this.modelDraft(id);
    if (!draft || !this.client.previewPoints)
      throw new Error("模型或校验服务不可用");
    const submitted = draft.content();
    const preview = await this.client.previewPoints(draft.input());
    if (!preview.model) throw preview.errors[0] ?? new Error("点表校验失败");
    const receipt = await this.execute({
      kind: "saveModel",
      model: preview.model,
    });
    draft.saved(submitted);
    return receipt;
  }
  readonly selectedUnits = new Map<string, string>();
  readonly categories = new Map<string, string>();
  constructor(readonly client: WorkspaceClient) {}
  subscribe = (listener: () => void) => {
    this.listeners.add(listener);
    return () => {
      this.listeners.delete(listener);
    };
  };
  snapshot = () => this.changeCount;
  changed = () => {
    this.changeCount++;
    for (const listener of [...this.listeners]) listener();
  };
  receive = (view: WorkspaceView) => {
    if (newerView(this.view, view)) {
      this.view = view;
      this.inspectDrafts();
      this.changed();
    }
  };
  async refresh() {
    const view = await this.client.query();
    if (this.view?.sequence === view.sequence) {
      this.view = view;
      this.inspectDrafts();
      this.changed();
    } else this.receive(view);
  }
  async connect() {
    let closed = false;
    let cancel: (() => Promise<void>) | undefined;
    try {
      await this.refresh();
      cancel = await this.client.subscribe((view) => {
        if (!closed) this.receive(view);
      });
    } catch (error) {
      this.error = errorMessage(error);
      this.changed();
    }
    return async () => {
      closed = true;
      await cancel?.();
    };
  }
  endpoint(id: string) {
    return (
      this.drafts.get(id) ??
      this.view?.catalog.endpoints.find((endpoint) => endpoint.id === id)
    );
  }
  private inspectDraft(endpoint: EndpointConfig) {
    const sequence=this.view?.sequence;
    void this.client.inspectEndpoint?.(endpoint).then(matches=>{
      if(this.drafts.get(endpoint.id)===endpoint && this.view?.sequence===sequence){this.draftMatches.set(endpoint.id,matches);this.changed();}
    }).catch(()=>{if(this.drafts.get(endpoint.id)===endpoint && this.view?.sequence===sequence){this.draftMatches.set(endpoint.id,{});this.changed();}});
  }
  private inspectDrafts() {for(const endpoint of this.drafts.values())this.inspectDraft(endpoint);}
  edit(endpoint: EndpointConfig) {
    const snapshot=structuredClone(endpoint);this.drafts.set(endpoint.id,snapshot);
    this.inspectDraft(snapshot);this.changed();
  }
  selectUnit(endpoint: string, unit: string) {
    this.selectedUnits.set(endpoint, unit);
    this.changed();
  }
  async execute(action: Action) {
    if (this.busy)
      throw { code: "COMMAND_PENDING", message: "请等待当前配置操作完成" };
    const submitted =
      action.kind === "saveEndpoint" ? JSON.stringify(action.endpoint) : null;
    this.busy = true;
    this.error = "";
    this.lastError = null;
    this.changed();
    try {
      const result = await this.client.execute({
        commandId: crypto.randomUUID(),
        action,
      });
      await this.refresh();
      if (action.kind === "saveEndpoint") {
        const current = this.drafts.get(action.endpoint.id);
        if (!current || JSON.stringify(current) === submitted) {
          this.drafts.delete(action.endpoint.id);this.draftMatches.delete(action.endpoint.id);
        }
      }
      if (action.kind === "deleteEndpoint") {
        this.drafts.delete(action.id); this.draftMatches.delete(action.id); this.selectedUnits.delete(action.id);
      }
      return result;
    } catch (error) {
      // 停机确认可能先于配置提交失败；查询实际状态，不把它推断为仍在运行。
      if (action.kind === "apply" || action.kind === "deleteEndpoint") await this.refresh().catch(() => {});
      this.lastError = error as ApiError;
      if (this.lastError.endpointId && this.lastError.unitId) this.selectedUnits.set(this.lastError.endpointId,this.lastError.unitId);
      this.error = errorMessage(error);
      throw error;
    } finally {
      this.busy = false;
      this.changed();
    }
  }
}
/** 新文档只引用仍存在的停靠面板，欢迎页关闭后优先复用文档区。 */
export function documentPosition(panelIds: string[]) {
  const document = panelIds.find(
    (id) =>
      id === "welcome" || id.startsWith("endpoint:") || id.startsWith("model:") || id.startsWith("resource:"),
  );
  if (document)
    return { referencePanel: document, direction: "within" as const };
  if (panelIds.includes("observations"))
    return { referencePanel: "observations", direction: "above" as const };
  if (panelIds.includes("project"))
    return { referencePanel: "project", direction: "right" as const };
  return undefined;
}

/** 核心比较结果仅对当前保存配置有效；草稿由独立dirty标识表达。 */
export function isEndpointApplied(catalog: Catalog | undefined, endpoint: EndpointConfig, matches?: Record<string,Record<string,boolean>>): boolean {
  const applied = catalog?.applied.find(item => item.id === endpoint.id);
  if (!applied || applied.units.length !== endpoint.units.length) return false;
  return !!matches?.[endpoint.id] && endpoint.units.every(unit => matches[endpoint.id][unit.id]);
}
/** 最小可用站号，仅用于添加建议，核心保存仍重新校验。 */
export function nextStation(endpoint: EndpointConfig): number | undefined {
  const used = new Set(endpoint.units.map(unit => unit.station));
  for(let station=1;station<=247;station++) if(!used.has(station)) return station;
}

/** 展示执行边界报告的真实状态，不把失败或释放中显示成未运行。 */
export function applicationStateLabel(state: string | undefined): string {
 return ({stopped:"已停止",running:"运行中",releasing:"释放中",failed:"释放状态未确认"} as Record<string,string>)[state ?? ""] ?? "未运行";
}
