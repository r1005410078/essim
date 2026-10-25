//! 原生窗口身份到独立工程上下文的映射；所有业务 IPC 使用同一解析入口。
use crate::{
    commands::{self, Engine},
    initial_window::ProjectWindow,
    window_index::WindowIndex,
    ExitPermission,
};
use essim_core::domain::error::Error;
use essim_infrastructure::{
    batch_lifecycle::BatchCoordinator, project::RecentProject, workspace::WorkspaceEngine,
};
use std::{
    collections::HashMap,
    marker::PhantomData,
    ops::Deref,
    path::PathBuf,
    sync::{
        atomic::{AtomicBool, AtomicU64, Ordering},
        Arc, Mutex,
    },
};
use tauri::{
    ipc::{CommandArg, CommandItem, InvokeError},
    Manager,
};

pub(crate) struct ProjectContext {
    pub engine: Engine,
    pub batches: commands::batch::Batches,
    pub session: commands::project::ProjectSession,
    pub subscriptions: commands::subscriptions::Subscriptions,
    pub jobs: commands::point_table::PointFileJobs,
    pub permission: ExitPermission,
    pub layout: ProjectWindow,
    pub closing: AtomicBool,
    pub pending_window: AtomicBool,
}
impl ProjectContext {
    pub fn new(root: PathBuf, recent: Arc<RecentProject>) -> Result<Arc<Self>, Error> {
        let engine = Arc::new(WorkspaceEngine::with_tcp_runtime(root)?);
        engine.require_project();
        let batches = Arc::new(BatchCoordinator::new(engine.clone())?);
        Ok(Arc::new(Self {
            engine,
            batches,
            session: commands::project::ProjectSession {
                recent,
                startup_error: Mutex::new(None),
                operation: AtomicBool::new(false),
                prepared: Mutex::new(None),
            },
            subscriptions: Default::default(),
            jobs: Default::default(),
            permission: Default::default(),
            layout: Default::default(),
            closing: AtomicBool::new(false),
            pending_window: AtomicBool::new(false),
        }))
    }
}
pub(crate) struct Windows {
    pub contexts: Mutex<HashMap<String, Arc<ProjectContext>>>,
    pub index: Mutex<WindowIndex>,
    pub recent: Arc<RecentProject>,
    pub root: PathBuf,
    next: AtomicU64,
    pub active: Mutex<Option<String>>,
    pub quitting: AtomicBool,
    pub quit_epoch: AtomicU64,
    pub quit: Mutex<Option<QuitTransaction>>,
}
pub(crate) struct QuitTransaction {
    pub id: u64,
    pub remaining: Vec<String>,
    pub current: Option<String>,
}
impl QuitTransaction {
    pub(crate) fn waiting_for(&self, epoch: Option<u64>, label: &str) -> bool {
        Some(self.id) == epoch && self.current.as_deref() == Some(label)
    }

    pub(crate) fn acknowledge(&mut self, label: &str) -> bool {
        if self.current.as_deref() == Some(label) {
            self.current = None;
            true
        } else {
            false
        }
    }
}
impl Windows {
    pub fn new(root: PathBuf) -> Self {
        Self {
            recent: Arc::new(RecentProject::new(&root)),
            root,
            contexts: Default::default(),
            index: Default::default(),
            next: AtomicU64::new(1),
            active: Default::default(),
            quitting: AtomicBool::new(false),
            quit_epoch: AtomicU64::new(1),
            quit: Default::default(),
        }
    }
    pub fn next_label(&self) -> String {
        format!("project-{}", self.next.fetch_add(1, Ordering::Relaxed))
    }
    pub fn context(&self, label: &str) -> Result<Arc<ProjectContext>, Error> {
        self.contexts
            .lock()
            .map_err(|_| Error::new("WINDOW_STATE", "窗口注册表不可用"))?
            .get(label)
            .cloned()
            .ok_or_else(|| Error::new("WINDOW_CLOSED", "工程窗口已关闭"))
    }
    pub fn ensure_open(&self) -> Result<(), Error> {
        if self.quitting.load(Ordering::Acquire) {
            Err(Error::new("APPLICATION_EXITING", "应用正在退出"))
        } else {
            Ok(())
        }
    }
    pub fn active_label(&self) -> Option<String> {
        self.active
            .lock()
            .ok()?
            .clone()
            .filter(|v| self.context(v).is_ok())
            .or_else(|| self.contexts.lock().ok()?.keys().next().cloned())
    }
}
/// 上下文成员解析不接受请求体提供的窗口 label。
pub(crate) trait WindowValue: Send + Sync + 'static {
    fn value(context: &ProjectContext) -> &Self;
}
macro_rules! member {
    ($ty:ty,$field:ident) => {
        impl WindowValue for $ty {
            fn value(c: &ProjectContext) -> &Self {
                &c.$field
            }
        }
    };
}
member!(Engine, engine);
member!(commands::batch::Batches, batches);
member!(commands::project::ProjectSession, session);
member!(commands::subscriptions::Subscriptions, subscriptions);
member!(commands::point_table::PointFileJobs, jobs);
member!(ExitPermission, permission);
member!(ProjectWindow, layout);
pub(crate) struct WindowState<'a, T: WindowValue> {
    context: Arc<ProjectContext>,
    marker: PhantomData<&'a T>,
}
impl<T: WindowValue> WindowState<'_, T> {
    pub fn inner(&self) -> &T {
        T::value(&self.context)
    }
}
impl<T: WindowValue> Deref for WindowState<'_, T> {
    type Target = T;
    fn deref(&self) -> &T {
        self.inner()
    }
}
impl<'a, T: WindowValue> CommandArg<'a, tauri::Wry> for WindowState<'a, T> {
    fn from_command(item: CommandItem<'a, tauri::Wry>) -> Result<Self, InvokeError> {
        let webview = item.message.webview_ref();
        let context = webview
            .state::<Windows>()
            .context(webview.label())
            .map_err(InvokeError::from)?;
        Ok(Self {
            context,
            marker: PhantomData,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn separate_contexts_keep_sessions_and_lifecycle_independent() {
        let root = std::env::temp_dir().join(format!("essim-windows-test-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(&root).unwrap();
        let windows = Windows::new(root.clone());
        let a = ProjectContext::new(root.join("runs-a"), windows.recent.clone()).unwrap();
        let b = ProjectContext::new(root.join("runs-b"), windows.recent.clone()).unwrap();
        let pa = a.engine.create_project(&root.join("a"), "A").unwrap();
        let pb = b.engine.create_project(&root.join("b"), "B").unwrap();
        windows
            .contexts
            .lock()
            .unwrap()
            .insert("a".into(), a.clone());
        windows
            .contexts
            .lock()
            .unwrap()
            .insert("b".into(), b.clone());
        assert!(windows.context("invented").is_err());
        assert!(a.engine.project_client(&pb.session).is_err());
        assert!(b.engine.project_client(&pa.session).is_err());
        let lease = a.engine.project_client(&pa.session).unwrap();
        assert!(a.engine.close_project().is_err());
        b.engine.close_project().unwrap();
        assert_eq!(
            a.engine.project_info().unwrap().unwrap().session,
            pa.session
        );
        drop(lease);
        let binfo = b
            .engine
            .create_project(&root.join("b-retry"), "B retry")
            .unwrap();
        let blocked = b.engine.project_client(&binfo.session).unwrap();
        assert!(crate::window_lifecycle::prepare_all(&[a.clone(), b.clone()]).is_err());
        assert!(a.engine.project_info().unwrap().is_some());
        assert!(b.engine.project_info().unwrap().is_some());
        assert!(!a.permission.confirmed.load(Ordering::Acquire));
        assert!(!b.permission.confirmed.load(Ordering::Acquire));
        drop(blocked);
        b.engine.close_project().unwrap();
        a.engine.prepare_project_exit().unwrap();
        assert!(a.engine.project_info().unwrap().is_some());
        a.engine.close_project().unwrap();
        drop(windows);
        drop(a);
        drop(b);
        std::fs::remove_dir_all(root).unwrap();
    }
}

#[cfg(test)]
mod quit_tests {
    use super::*;
    #[test]
    fn exit_acknowledgement_cannot_skip_other_windows_or_repeat() {
        let mut plan = QuitTransaction {
            id: 1,
            remaining: vec!["b".into()],
            current: Some("a".into()),
        };
        assert!(!plan.acknowledge("b"));
        assert_eq!(plan.current.as_deref(), Some("a"));
        assert!(plan.acknowledge("a"));
        assert!(!plan.acknowledge("a"));
        assert_eq!(plan.remaining, vec!["b".to_string()]);
        plan.id = 2;
        plan.current = Some("b".into());
        assert!(!plan.waiting_for(Some(1), "b"));
        assert!(!plan.waiting_for(Some(2), "a"));
        assert!(plan.waiting_for(Some(2), "b"));
        assert!(plan.acknowledge("b"));
        assert!(!plan.waiting_for(Some(2), "b"));
    }
}
