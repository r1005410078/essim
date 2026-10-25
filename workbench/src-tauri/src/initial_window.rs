//! 约束首次窗口及工程引导尺寸；进入工作台后恢复用户的窗口边界。
use tauri::{LogicalSize, PhysicalPosition, PhysicalRect, PhysicalSize};

#[derive(Debug, PartialEq)]
struct InitialSize {
    size: LogicalSize<f64>,
    minimum: LogicalSize<f64>,
}

fn fit_size(preferred: LogicalSize<f64>, work: PhysicalSize<u32>, scale: f64) -> InitialSize {
    let available = work.to_logical::<f64>(scale);
    // 给窗口装饰和屏幕边缘留出空间，工作区已排除系统菜单栏与 Dock／任务栏。
    let size = LogicalSize::new(
        preferred.width.min((available.width - 64.0).max(1.0)),
        preferred.height.min((available.height - 64.0).max(1.0)),
    );
    InitialSize {
        minimum: LogicalSize::new(900.0_f64.min(size.width), 600.0_f64.min(size.height)),
        size,
    }
}

fn centered_position(
    work: &PhysicalRect<i32, u32>,
    outer: PhysicalSize<u32>,
) -> PhysicalPosition<i32> {
    PhysicalPosition::new(
        work.position.x + (work.size.width.saturating_sub(outer.width) / 2) as i32,
        work.position.y + (work.size.height.saturating_sub(outer.height) / 2) as i32,
    )
}

/// 根据显示器可用区域调整并居中尚未显示的主窗口。
///
/// # Errors
/// 读取显示器、窗口尺寸或设置原生窗口边界失败时返回错误。
pub(crate) fn configure(window: &tauri::WebviewWindow) -> tauri::Result<()> {
    let monitor = window.current_monitor()?.or(window.primary_monitor()?);
    if let Some(monitor) = monitor {
        let work = monitor.work_area();
        if work.size.width == 0 || work.size.height == 0 {
            return window.center();
        }
        let preferred = window.inner_size()?.to_logical(monitor.scale_factor());
        let fitted = fit_size(preferred, work.size, monitor.scale_factor());
        window.set_min_size(Some(fitted.minimum))?;
        window.set_size(fitted.size)?;
        window.set_position(centered_position(work, window.outer_size()?))?;
    } else {
        window.center()?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn retina_work_area_uses_logical_pixels() {
        let fitted = fit_size(
            LogicalSize::new(1280.0, 800.0),
            PhysicalSize::new(2880, 1750),
            2.0,
        );
        assert_eq!(fitted.size, LogicalSize::new(1280.0, 800.0));
        assert_eq!(fitted.minimum, LogicalSize::new(900.0, 600.0));
    }

    #[test]
    fn short_display_leaves_room_for_frame_and_system_chrome() {
        let fitted = fit_size(
            LogicalSize::new(1280.0, 800.0),
            PhysicalSize::new(1366, 728),
            1.0,
        );
        assert!(fitted.size.height <= 664.0);
        assert_eq!(fitted.size.width, 1280.0);
    }

    #[test]
    fn small_display_is_not_forced_past_work_area_by_minimum_size() {
        let fitted = fit_size(
            LogicalSize::new(1280.0, 800.0),
            PhysicalSize::new(900, 560),
            1.0,
        );
        assert!(fitted.minimum.width <= fitted.size.width);
        assert!(fitted.minimum.height <= fitted.size.height);
        assert!(fitted.size.width < 900.0 && fitted.size.height < 560.0);
    }

    #[test]
    fn secondary_monitor_uses_its_work_area_origin() {
        let work = PhysicalRect {
            position: PhysicalPosition::new(-1920, 40),
            size: PhysicalSize::new(1920, 1040),
        };
        assert_eq!(
            centered_position(&work, PhysicalSize::new(1280, 828)),
            PhysicalPosition::new(-1600, 146)
        );
    }
}

/// 同一窗口进出引导页时保留工作台边界，避免固定启动尺寸覆盖工作台布局。
#[derive(Default)]
pub(crate) struct ProjectWindow(std::sync::Mutex<Option<WorkspaceBounds>>);
struct WorkspaceBounds {
    size: PhysicalSize<u32>,
    position: PhysicalPosition<i32>,
    maximized: bool,
}

impl ProjectWindow {
    pub(crate) fn apply(&self, window: &tauri::WebviewWindow, launcher: bool) -> tauri::Result<()> {
        let mut saved = self
            .0
            .lock()
            .map_err(|_| tauri::Error::Io(std::io::Error::other("窗口布局锁不可用")))?;
        if launcher {
            if saved.is_some() {
                return Ok(());
            }
            let bounds = WorkspaceBounds {
                size: window.inner_size()?,
                position: window.outer_position()?,
                maximized: window.is_maximized()?,
            };
            if window.is_fullscreen()? {
                window.set_fullscreen(false)?;
            }
            window.unmaximize()?;
            window.set_resizable(true)?;
            window.set_min_size(None::<LogicalSize<f64>>)?;
            let preferred = LogicalSize::new(960.0, 720.0);
            let size =
                if let Some(monitor) = window.current_monitor()?.or(window.primary_monitor()?) {
                    fit_size(preferred, monitor.work_area().size, monitor.scale_factor()).size
                } else {
                    preferred
                };
            window.set_size(size)?;
            window.center()?;
            window.set_maximizable(false)?;
            window.set_resizable(false)?;
            *saved = Some(bounds);
        } else if let Some(bounds) = saved.as_ref() {
            window.set_resizable(true)?;
            window.set_maximizable(true)?;
            window.set_size(bounds.size)?;
            window.set_position(bounds.position)?;
            let logical = bounds.size.to_logical::<f64>(window.scale_factor()?);
            window.set_min_size(Some(LogicalSize::new(
                900.0_f64.min(logical.width),
                600.0_f64.min(logical.height),
            )))?;
            if bounds.maximized {
                window.maximize()?;
            }
            *saved = None;
        }
        Ok(())
    }
}

/// 由后端工程状态决定启动页固定尺寸或工作台可缩放模式。
#[tauri::command]
pub(crate) fn sync_project_window(
    window: tauri::WebviewWindow,
    engine: crate::window_context::WindowState<
        '_,
        std::sync::Arc<essim_infrastructure::workspace::WorkspaceEngine>,
    >,
    layout: crate::window_context::WindowState<'_, ProjectWindow>,
) -> Result<(), String> {
    let project = engine.project_info().map_err(|e| e.to_string())?;
    let launcher = project.is_none();
    window
        .set_title(
            &project
                .as_ref()
                .map(|p| format!("ESSim · {}", p.name))
                .unwrap_or_else(|| "ESSim".into()),
        )
        .map_err(|e| e.to_string())?;
    layout
        .apply(&window, launcher)
        .and_then(|_| window.show())
        .map_err(|e| e.to_string())?;
    use tauri::Manager;
    let windows = window.state::<crate::window_context::Windows>();
    let context = windows.context(window.label()).map_err(|e| e.to_string())?;
    if context
        .pending_window
        .swap(false, std::sync::atomic::Ordering::AcqRel)
    {
        if let Some(info) = engine.project_info().map_err(|e| e.to_string())? {
            windows
                .index
                .lock()
                .unwrap()
                .publish(info.entry, window.label());
        }
        window.set_focus().map_err(|e| e.to_string())?;
        crate::window_lifecycle::focused(window.app_handle(), window.label());
    }
    Ok(())
}
