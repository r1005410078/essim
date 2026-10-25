import { beforeEach, expect, it, vi } from "vitest";
const mocks = vi.hoisted(() => ({
  desktop: true,
  minimize: vi.fn(), maximize: vi.fn(), close: vi.fn(), drag: vi.fn(),
  invoke: vi.fn(),
  eventTargets: new Map<string, unknown>(),
  windowHandler: null as
    | null
    | ((event: { preventDefault: () => void }) => void),
  quitHandler: null as null | (() => void),
  cancelWindow: vi.fn(),
  cancelExit: vi.fn(),
  editHandler: null as
    | null
    | ((event: { payload: "undo" | "redo" | "selectAll" }) => void),
}));
vi.mock("@tauri-apps/api/core", () => ({
  isTauri: () => mocks.desktop,
  invoke: mocks.invoke,
}));
vi.mock("@tauri-apps/api/window", () => ({
  getCurrentWindow: () => ({
    label: "test-a",
    minimize:mocks.minimize,toggleMaximize:mocks.maximize,close:mocks.close,startDragging:mocks.drag,
    onCloseRequested: async (handler: typeof mocks.windowHandler) => {
      mocks.windowHandler = handler;
      return mocks.cancelWindow;
    },
  }),
}));
vi.mock("@tauri-apps/api/event", () => ({
  listen: async (name: string, handler: () => void, options?: unknown) => {
    mocks.eventTargets.set(name, options);
    if (name === "workbench-edit") mocks.editHandler = handler;
    else if (name === "workbench-exit-requested") mocks.quitHandler = handler;
    return mocks.cancelExit;
  },
}));
import { armCloseGuard, closeWorkbenchWindow, armEditCommands } from "./window";
beforeEach(() => {vi.clearAllMocks();mocks.desktop=true;});
it("窗口关闭与macOS退出均保护整个会话，只有明确确认才退出", async () => {
  const request = vi.fn();
  const preventDefault = vi.fn();
  const cancel = await armCloseGuard(() => true, request);
  mocks.windowHandler?.({ preventDefault });
  mocks.quitHandler?.();
  expect(preventDefault).toHaveBeenCalledOnce();
  expect(request).toHaveBeenCalledTimes(2);
  expect(mocks.invoke).not.toHaveBeenCalledWith("exit_workbench");
  await closeWorkbenchWindow();
  expect(mocks.invoke).toHaveBeenCalledWith("exit_workbench");
  cancel();
  expect(mocks.cancelWindow).toHaveBeenCalledOnce();
  expect(mocks.cancelExit).toHaveBeenCalledTimes(3);
});

it("弹窗关闭焦点落在body时原生撤销仍访问当前文档，普通输入框保留原生撤销", async () => {
  class Target {
    dataset = {};
    editing = false;
    matches() {
      return this.editing;
    }
    dispatchEvent = vi.fn();
  }
  const target = new Target();
  const execCommand = vi.fn();
  vi.stubGlobal("HTMLElement", Target);
  vi.stubGlobal("document", { activeElement: target, execCommand });
  try {
    const documentAction = vi.fn(() => true);
    await armEditCommands(documentAction);
    mocks.editHandler?.({ payload: "undo" });
    expect(documentAction).toHaveBeenCalledExactlyOnceWith("undo");
    expect(target.dispatchEvent).not.toHaveBeenCalled();
    target.editing = true;
    mocks.editHandler?.({ payload: "redo" });
    expect(execCommand).toHaveBeenCalledExactlyOnceWith("redo");
    expect(documentAction).toHaveBeenCalledTimes(1);
  } finally {
    vi.unstubAllGlobals();
  }
});


it("浏览器不调用窗口API，桌面关闭仍走close-requested入口", async()=>{
 const {operateWindow,windowPlatform}=await import("./window");
 mocks.desktop=false;expect(windowPlatform()).toBe("browser");
 for(const action of ["drag","minimize","maximize","close"] as const)await operateWindow(action);
 expect(mocks.close).not.toHaveBeenCalled();expect(mocks.drag).not.toHaveBeenCalled();
 mocks.desktop=true;
 await operateWindow("close");expect(mocks.close).toHaveBeenCalledOnce();expect(mocks.invoke).not.toHaveBeenCalled();
 await operateWindow("drag");await operateWindow("minimize");await operateWindow("maximize");
 expect(mocks.drag).toHaveBeenCalledOnce();expect(mocks.minimize).toHaveBeenCalledOnce();expect(mocks.maximize).toHaveBeenCalledOnce();
});

it('窗口模式交给后端工程状态决定，浏览器不发出原生命令', async () => {
  const { syncProjectWindow } = await import('./window');
  await syncProjectWindow();
  expect(mocks.invoke).toHaveBeenCalledWith('sync_project_window');
  mocks.invoke.mockClear(); mocks.desktop = false;
  await syncProjectWindow();
  expect(mocks.invoke).not.toHaveBeenCalled();
});

it('工程业务事件只监听当前窗口，退出取消保持进程广播', async () => {
  const { armSaveCommand, listenWindowEvent } = await import('./window');
  await armCloseGuard(() => true, vi.fn());
  await armSaveCommand(vi.fn());
  await listenWindowEvent('workbench-project', vi.fn());
  for (const event of ['workbench-save', 'workbench-project', 'workbench-exit-requested', 'workbench-window-error']) {
    expect(mocks.eventTargets.get(event)).toEqual({target:{kind:'Window',label:'test-a'}});
  }
  expect(mocks.eventTargets.get('workbench-exit-cancelled')).toBeUndefined();
});
