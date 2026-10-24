import {UnitPointDraft,effectiveRows,buildOverrides} from "./unitOverrides";
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
import { ModelDraft, modelRows, fields } from "./pointDraft";
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
    const modelFolders = new Map<string,string>();
    const modelFolder = (id:string):string => {
      const cached=modelFolders.get(id);if(cached)return cached;
      const folder=catalog.navigation?.folders.find(f=>f.id===id);if(!folder)return "采集模型";
      const path=leaf(folder.parent?modelFolder(folder.parent):"采集模型",folder.name,folder.id,{kind:"folder",id:folder.id});
      modelFolders.set(id,path);paths.push(`${path}/`);targets.set(`${path}/`,{kind:"folder",id:folder.id});return path;
    };
    for(const folder of catalog.navigation?.folders??[])if(folder.scope==="models")modelFolder(folder.id);
    for (const model of catalog.models) {
      const parent=catalog.navigation?.modelMembers?.[model.id];
      paths.push(leaf(parent?modelFolder(parent):"采集模型",model.name,model.id,{kind:"model",id:model.id}));
    }
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
    for(const folder of catalog.navigation?.folders ?? []) if(folder.scope!=="models")folderPath(folder.id);
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
export type NavigationSection = "simulators" | "resources";
/** 按标签投影导航，路径只用于展示，设备引用和Unit仍指向稳定业务身份。 */
export function projectNavigation(catalog: Catalog | undefined, section: NavigationSection, userDocuments:readonly import("../../shared/api/project").ProjectDocumentInfo[] = []) {
  const tree = catalog ? navigation(catalog, section) : { paths: [], targets: new Map<string, Target>() };
  if (section === "simulators") return tree;
  const documents = resourceNavigation();
  for(const doc of userDocuments){const path=`文档资源/工程文档/${doc.name.replaceAll('/','∕')} · ${encodeURIComponent(doc.id)}`;documents.paths.push(path);documents.targets.set(path,{kind:'resource',id:`project-document:${doc.id}`});}
  return {
    paths: ["采集模型/", "设备/", ...tree.paths, ...documents.paths],
    targets: new Map([...tree.targets, ...documents.targets]),
  };
}
export function errorMessage(error: unknown): string {
  const value = error as Partial<ApiError>;
  return value?.message ?? String(error);
}
/** 后端快照与界面草稿分离；切换单元只更新选择。 */
export class WorkspaceStore {
  project: import("../../shared/api/project").ProjectInfo | null = null;
  view: WorkspaceView | null = null;
  error = "";
  lastError: ApiError | null = null;
  busy = false;
  projectBusy=false;
  private listeners = new Set<() => void>();
  private changeCount = 0;
  readonly focusModelIssue=new Map<string,(issue:ApiError)=>void>();
  readonly runtimePoint=new Map<string,string>();
  readonly observedUnitQuality=new Map<string,{sequence:string;quality:'stale'|'failed'|undefined}>();
  recordUnitQuality(unit:string,points:Record<string,{quality:string}>){
    const sequence=this.view?.sequence;if(sequence===undefined)return;
    const values=Object.values(points);if(!values.length)return;
    const quality=values.some(p=>p.quality==='failed')?'failed':values.some(p=>p.quality==='stale')?'stale':undefined;
    const previous=this.observedUnitQuality.get(unit);
    if(previous?.sequence===sequence&&previous.quality===quality)return;
    this.observedUnitQuality.set(unit,{sequence,quality});this.changed();
  }
  readonly draftMatches=new Map<string,Record<string,boolean>>();
  readonly draftBaselines=new Map<string,string>();
  readonly drafts = new Map<string, EndpointConfig>();
  readonly modelDrafts = new Map<string, ModelDraft>();
  readonly unitDrafts=new Map<string,UnitPointDraft>();
  readonly flushUnitEditors=new Map<string,()=>void>();
  readonly endpointModes=new Map<string,"configuration"|"runtime">();
  endpointDirty(id:string){return this.drafts.has(id)||[...this.unitDrafts.values()].some(d=>d.endpointId===id&&d.dirty);}
  discardEndpoint(id:string){this.draftBaselines.delete(id);this.drafts.delete(id);this.draftMatches.delete(id);for(const [key,d] of this.unitDrafts)if(d.endpointId===id)this.unitDrafts.delete(key);this.changed();}
  unitDraft(endpointId:string,unitId:string){
    const unit=this.endpoint(endpointId)?.units.find(u=>u.id===unitId);if(!unit)return;
    if(this.client.loadModel&&!this.loadedModels.has(unit.model.id)){void this.loadTarget("model",unit.model.id).catch(e=>{this.error=errorMessage(e);this.changed();});return;}
    const base=this.view?.catalog.models.find(m=>m.id===unit.model.id);if(!base)return;
    let draft=this.unitDrafts.get(unitId);
    if(draft&&!draft.dirty&&(draft.changedBase(base)||JSON.stringify(draft.sourceUnit.overrides)!==JSON.stringify(unit.overrides)||draft.sourceUnit.model.id!==unit.model.id)){this.unitDrafts.delete(unitId);draft=undefined;}
    if(!draft){draft=new UnitPointDraft(endpointId,unit,base,this.changed);this.unitDrafts.set(unitId,draft);}return draft;
  }
  rebaseUnitDraft(endpointId:string,unitId:string){
    const old=this.unitDrafts.get(unitId);const unit=this.endpoint(endpointId)?.units.find(u=>u.id===unitId);const base=this.view?.catalog.models.find(m=>m.id===unit?.model.id);if(!old||!unit||!base)throw Error("模型不可用");
    if(old.sourceUnit.model.id!==unit.model.id)throw Error("模型引用已改变，请放弃旧草稿后重新编辑。");
    const next=new UnitPointDraft(endpointId,unit,base,this.changed);
    const originals=new Map(effectiveRows(old.base,old.sourceUnit).map(r=>[r.id,r]));
    next.change(state=>{
      const rows=new Map(state.rows.map(r=>[r.id,r]));const retained=new Set(old.rows.map(r=>r.id));
      for(const [id] of originals)if(!retained.has(id))rows.delete(id);
      for(const row of old.rows){const original=originals.get(row.id);if(!original){rows.set(row.id,structuredClone(row));continue;}
        const changed=fields.filter(f=>row[f]!==original[f]);if(!changed.length)continue;
        const target=rows.get(row.id);if(!target)throw Error(`基础点 ${row.name} 已删除，请先处理该点草稿。`);
        for(const f of changed)target[f]=row[f]!;
      }
      state.rows=[...rows.values()];state.inheritedFields=[...old.inheritedFields];
      const baseRows=new Map(modelRows(base).map(r=>[r.id,r]));for(const key of old.inheritedFields){const field=fields.find(f=>key.endsWith(`:${f}`));if(!field)continue;const id=key.slice(0,-field.length-1);const row=state.rows.find(r=>r.id===id);if(row&&baseRows.get(id))row[field]=baseRows.get(id)![field]!;}
    });this.unitDrafts.set(unitId,next);this.changed();
  }
  async saveEndpointDraft(id:string){
    for(const flush of this.flushUnitEditors.values())flush();
    const endpoint=structuredClone(this.endpoint(id));if(!endpoint)throw Error("模拟器不存在");
    const original=JSON.stringify(endpoint);
    const submitted=new Map<string,{draft:UnitPointDraft;content:string}>();
    for(const unit of endpoint.units){const d=this.unitDrafts.get(unit.id);if(!d?.dirty)continue;
      const base=await this.client.loadModel?.(unit.model.id)??this.view?.catalog.models.find(m=>m.id===unit.model.id);
      if(d.changedBase(base)||d.sourceUnit.model.id!==unit.model.id)throw Error("基础模型已改变：请在配置表中处理草稿后再保存，输入已保留。");
      const content=d.content();const inherited=[...d.inheritedFields];const preview=await this.client.previewPoints?.(d.input());if(!preview?.model)throw preview?.errors[0]??Error("点表校验不可用");
      unit.overrides=buildOverrides(d.base,d.sourceUnit.overrides,preview.model,inherited);submitted.set(unit.id,{draft:d,content});
    }
    if(JSON.stringify(this.endpoint(id))!==original)throw Error("校验期间配置又有修改，请再次保存，输入已保留。");
    if(!this.draftBaselines.has(id))this.draftBaselines.set(id,JSON.stringify(this.view?.catalog.endpoints.find(e=>e.id===id)));
    this.drafts.set(id,endpoint);
    const receipt=await this.execute({kind:"saveEndpoint",endpoint});
    for(const [id,{draft,content}] of submitted){draft.sourceUnit.overrides=structuredClone(endpoint.units.find(u=>u.id===id)?.overrides);draft.saved(content);if(draft.content()===content)this.unitDrafts.delete(id);}
    this.changed();return receipt;
  }
  modelDraft(id: string) {
    let draft = this.modelDrafts.get(id);
    if (!draft) {
      if(this.client.loadModel&&!this.loadedModels.has(id)){void this.loadTarget("model",id).catch(e=>{this.error=errorMessage(e);this.changed();});return;}
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
  private loadedModels=new Set<string>();
  private loadedEndpoints=new Set<string>();
  private loading=new Map<string,Promise<void>>();
  private mergeLazy(view:WorkspaceView){
    if(!this.client.loadModel)return view;
    const old=this.view;
    const models=view.catalog.models.map(m=>this.loadedModels.has(m.id)?old?.catalog.models.find(v=>v.id===m.id)??m:m);
    const units=new Set(view.catalog.applied.flatMap(e=>this.loadedEndpoints.has(e.id)?e.units.map(u=>u.id):[]));
    return {...view,catalog:{...view.catalog,models,values:Object.fromEntries(Object.entries(old?.catalog.values??{}).filter(([id])=>units.has(id)))}};
  }
  async loadTarget(kind:"model"|"endpoint",id:string,force=false):Promise<void>{
    if(!this.client.loadModel || (kind==="model"&&this.modelDrafts.get(id)?.persisted===false))return;
    const sequence=this.view?.sequence;const key=`${sequence}:${kind}:${id}`;
    const known=kind==="model"?this.loadedModels:this.loadedEndpoints;
    if(known.has(id)&&!force)return;
    const pending=this.loading.get(key);if(pending)return pending;
    const job=(async()=>{
      if(kind==="model"){
        const model=await this.client.loadModel!(id);
        if(!this.view||this.view.sequence!==sequence)return;
        this.view={...this.view,catalog:{...this.view.catalog,models:this.view.catalog.models.map(m=>m.id===id?model:m)}};
        this.loadedModels.add(id);
      }else{
        const endpoint=this.view?.catalog.endpoints.find(e=>e.id===id);if(!endpoint)return;
        await Promise.all([...new Set(endpoint.units.map(u=>u.model.id))].map(m=>this.loadTarget("model",m,force)));
        const applied=this.view?.catalog.applied.find(e=>e.id===id);
        const values=applied?await this.client.loadEndpointValues?.(id):{};
        if(!this.view||this.view.sequence!==sequence)return;
        this.view={...this.view,catalog:{...this.view.catalog,values:{...this.view.catalog.values,...values}}};
        this.loadedEndpoints.add(id);
      }
      this.changed();
    })();
    this.loading.set(key,job);
    try{await job;}finally{this.loading.delete(key);}
  }
  private reloadLazy(){
    for(const id of this.loadedModels)void this.loadTarget("model",id,true).catch(e=>{this.error=errorMessage(e);this.changed();});
    for(const id of this.loadedEndpoints)void this.loadTarget("endpoint",id,true).catch(e=>{this.error=errorMessage(e);this.changed();});
  }
  receive = (view: WorkspaceView) => {
    if (newerView(this.view, view)) {
      this.view = this.mergeLazy(view);
      this.reloadLazy();
      this.inspectDrafts();
      this.changed();
    }
  };
  async refresh() {
    const view = await this.client.query();
    if (this.view?.sequence === view.sequence) {
      this.view = this.mergeLazy(view);
      this.reloadLazy();
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
    if(!this.draftBaselines.has(endpoint.id))this.draftBaselines.set(endpoint.id,JSON.stringify(this.view?.catalog.endpoints.find(e=>e.id===endpoint.id)));
    const snapshot=structuredClone(endpoint);this.drafts.set(endpoint.id,snapshot);
    this.inspectDraft(snapshot);this.changed();
  }
  selectUnit(endpoint: string, unit: string) {
    this.selectedUnits.set(endpoint, unit);
    this.changed();
  }
  async execute(action: Action) {
    if (this.busy||this.projectBusy)
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
      const target=action.kind==="saveEndpoint"||action.kind==="createEndpoint"||action.kind==="createEndpointWithModel"?action.endpoint.id:action.kind==="promoteOverrides"?action.plan.endpoint:action.kind==="extractUnitModel"?action.endpoint:null;
      if(target)await this.loadTarget("endpoint",target,true);
      if(action.kind==="createInFolder")await this.loadTarget(action.resource.kind,action.resource.kind==="model"?action.resource.model.id:action.resource.endpoint.id,true);
      if(action.kind==="saveModel")await this.loadTarget("model",action.model.id,true);
      if (action.kind === "saveEndpoint") {
        const current = this.drafts.get(action.endpoint.id);
        if (!current || JSON.stringify(current) === submitted) {
          this.drafts.delete(action.endpoint.id);this.draftBaselines.delete(action.endpoint.id);this.draftMatches.delete(action.endpoint.id);
        }
      }
      if (action.kind === "deleteEndpoint") {
        this.drafts.delete(action.id);this.draftBaselines.delete(action.id); this.draftMatches.delete(action.id); this.selectedUnits.delete(action.id);
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
