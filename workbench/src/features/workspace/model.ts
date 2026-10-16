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
  kind: "endpoint" | "model" | "device";
  id: string;
  unit?: string;
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
    const latest = new Map(catalog.models.map((model) => [model.id, model]));
    for (const model of latest.values())
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
  } else
    for (const endpoint of catalog.endpoints) {
      const address =
        endpoint.transport.kind === "tcp"
          ? `${endpoint.transport.address}:${endpoint.transport.port}`
          : endpoint.transport.path;
      const parent = leaf(
        "",
        `${endpoint.transport.kind.toUpperCase()} ${endpoint.role === "slave" ? "从站" : "主站"} - ${address}`,
        endpoint.id,
        { kind: "endpoint", id: endpoint.id },
      );
      paths.push(`${parent}/`);
      targets.set(`${parent}/`, { kind: "endpoint", id: endpoint.id });
      for (const unit of endpoint.units)
        paths.push(
          leaf(parent, `Unit ${unit.station} · ${unit.name}`, unit.id, {
            kind: "endpoint",
            id: endpoint.id,
            unit: unit.id,
          }),
        );
    }
  return { paths, targets };
}
export function errorMessage(error: unknown): string {
  const value = error as Partial<ApiError>;
  return value?.message ?? String(error);
}
/** 后端快照与界面草稿分离；切换单元只更新选择。 */
export class WorkspaceStore {
  view: WorkspaceView | null = null;
  error = "";
  busy = false;
  private listeners = new Set<() => void>();
  private version = 0;
  readonly drafts = new Map<string, EndpointConfig>();
  readonly modelDrafts = new Map<string, ModelDraft>();
  modelDraft(id: string) {
    let draft = this.modelDrafts.get(id);
    if (!draft) {
      const model = this.view?.catalog.models.filter((m) => m.id === id).at(-1);
      if (!model) return;
      draft = new ModelDraft(
        id,
        model.name,
        model.version,
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
    const expectedRevision = draft.version;
    const preview = await this.client.previewPoints(draft.input());
    if (!preview.model) throw preview.errors[0] ?? new Error("点表校验失败");
    if (draft.version !== expectedRevision)
      throw new Error("模型版本已变化，请重新保存当前草稿。");
    const receipt = await this.execute({
      kind: "saveModel",
      model: preview.model,
      expectedRevision,
    });
    draft.saved(receipt.revision, submitted);
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
  snapshot = () => this.version;
  changed = () => {
    this.version++;
    for (const listener of [...this.listeners]) listener();
  };
  receive = (view: WorkspaceView) => {
    if (newerView(this.view, view)) {
      this.view = view;
      this.changed();
    }
  };
  async refresh() {
    const view = await this.client.query();
    if (this.view?.sequence === view.sequence) {
      this.view = view;
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
  edit(endpoint: EndpointConfig) {
    this.drafts.set(endpoint.id, structuredClone(endpoint));
    this.changed();
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
          this.drafts.delete(action.endpoint.id);
        } else {
          // 用户在IPC确认前继续编辑：保留新内容，仅推进已确认的基准版本。
          this.drafts.set(current.id, {
            ...current,
            revision: result.revision,
          });
        }
      }
      return result;
    } catch (error) {
      // 停机确认可能先于配置提交失败；查询实际状态，不把它推断为仍在运行。
      if (action.kind === "apply") await this.refresh().catch(() => {});
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
      id === "welcome" || id.startsWith("endpoint:") || id.startsWith("model:"),
  );
  if (document)
    return { referencePanel: document, direction: "within" as const };
  if (panelIds.includes("observations"))
    return { referencePanel: "observations", direction: "above" as const };
  if (panelIds.includes("project"))
    return { referencePanel: "project", direction: "right" as const };
  return undefined;
}
