/** 应用生命周期适配；业务视图不直接访问Tauri窗口。 */
import { invoke, isTauri } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { getCurrentWindow } from "@tauri-apps/api/window";
export async function armCloseGuard(
  hasSessionData: () => boolean,
  request: () => void,
): Promise<() => void> {
  if (!isTauri()) return () => {};
  const cancelWindow = await getCurrentWindow().onCloseRequested((event) => {
    if (hasSessionData()) {
      event.preventDefault();
      request();
    }
  });
  const cancelExit = await listen("workbench-exit-requested", () => {
    if (hasSessionData()) request();
    else void closeWorkbenchWindow();
  });
  await invoke("set_exit_guard_ready", { ready: true });
  return () => {
    cancelWindow();
    cancelExit();
    void invoke("set_exit_guard_ready", { ready: false });
  };
}
export async function closeWorkbenchWindow() {
  if (isTauri()) await invoke("exit_workbench");
}

/** 原生菜单历史命令走相同DOM键盘路径；正在编辑普通输入框时保留浏览器原生撤销。 */
export async function armEditCommands(
  handleDocument?: (action: "undo" | "redo" | "selectAll") => boolean,
): Promise<() => void> {
  if (!isTauri()) return () => {};
  return listen<"undo" | "redo" | "selectAll">("workbench-edit", (event) => {
    const target = document.activeElement;
    if (!(target instanceof HTMLElement)) return;
    if (
      target.matches("input,textarea,[contenteditable=true]") &&
      target.dataset.pointKeyboard !== "true"
    ) {
      document.execCommand(event.payload);
      return;
    }
    if (handleDocument?.(event.payload)) return;
    target.dispatchEvent(
      new KeyboardEvent("keydown", {
        key: event.payload === "selectAll" ? "a" : "z",
        metaKey: true,
        ctrlKey: true,
        shiftKey: event.payload === "redo",
        bubbles: true,
        cancelable: true,
      }),
    );
  });
}
