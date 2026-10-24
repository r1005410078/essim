//! 工程切换使用同一应用服务和运行所有者，成功后才替换权威仓储。
use super::{BudgetRepository, ProjectRepository};
use crate::workspace::{MemoryRepository, WorkspaceEngine};
use essim_core::domain::error::Error;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
/// 当前工程入口和显示信息；路径仅在工程管理界面展示。
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProjectInfo {
    /// 工程中的用户文档目录，不包含应用内置资料。
    pub documents: Vec<ProjectDocumentInfo>,
    /// 当前打开会话的临时凭证，复制工程后也不能重放旧界面请求。
    pub session: String,
    /// 工程稳定身份。
    pub id: String,
    /// 显示名称。
    pub name: String,
    /// 唯一打开入口绝对路径。
    pub entry: PathBuf,
}
/// 工程文档的显示信息；物理路径仅由基础设施使用。
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProjectDocumentInfo {
    /// 稳定文档身份。
    pub id: String,
    /// 显示名称，不含存储后缀。
    pub name: String,
    /// 工程相对路径，不发送给界面显示。
    #[serde(skip)]
    pub(crate) path: String,
}
/// 保持单个桌面请求的工程身份有效，生命周期切换不得跨越活跃请求。
pub struct ProjectClientGuard(std::sync::Arc<std::sync::atomic::AtomicUsize>);
impl Drop for ProjectClientGuard {
    fn drop(&mut self) {
        self.0.fetch_sub(1, std::sync::atomic::Ordering::AcqRel);
    }
}
impl WorkspaceEngine {
    /// 为正式桌面启用工程门禁；内存测试和无界面执行器默认不启用。
    pub fn require_project(&self) {
        self.project_required
            .store(true, std::sync::atomic::Ordering::Release);
    }
    pub(crate) fn project_operation(&self) -> Result<std::sync::RwLockReadGuard<'_, ()>, Error> {
        let guard = self.project_gate.read().map_err(poison)?;
        self.ensure_project()?;
        Ok(guard)
    }
    pub(crate) fn ensure_project(&self) -> Result<(), Error> {
        if self
            .project_required
            .load(std::sync::atomic::Ordering::Acquire)
            && self.project_info()?.is_none()
        {
            return Err(Error::new("PROJECT_REQUIRED", "请先打开或新建工程"));
        }
        Ok(())
    }
    /// 将桌面请求绑定到当前打开会话，直到请求完成前禁止替换工程。
    /// # Errors
    /// 未打开工程或请求来自旧会话。
    pub fn project_client(&self, session: &str) -> Result<ProjectClientGuard, Error> {
        // IPC 参数解析不能等待停机线程持有的写锁，否则会阻塞原生事件循环。
        let _gate = self.project_gate.try_read().map_err(|error| match error {
            std::sync::TryLockError::WouldBlock => {
                Error::new("PROJECT_BUSY", "工程正在切换，请稍后重试")
            }
            std::sync::TryLockError::Poisoned(_) => Error::new("STATE_POISONED", "工程状态不可用"),
        })?;
        let info = self
            .project_info()?
            .ok_or_else(|| Error::new("PROJECT_REQUIRED", "请先打开或新建工程"))?;
        if info.session != session {
            return Err(Error::new("PROJECT_CHANGED", "工程已切换，请刷新后重试"));
        }
        self.project_clients
            .fetch_add(1, std::sync::atomic::Ordering::AcqRel);
        Ok(ProjectClientGuard(self.project_clients.clone()))
    }
    fn ensure_no_clients(&self) -> Result<(), Error> {
        if self
            .project_clients
            .load(std::sync::atomic::Ordering::Acquire)
            > 0
        {
            return Err(Error::new("PROJECT_BUSY", "工程请求尚未完成，请稍后重试"));
        }
        Ok(())
    }

    /// 返回当前已打开工程；正式桌面无工程时只提供引导。
    /// # Errors
    /// 工程状态锁不可用。
    pub fn project_info(&self) -> Result<Option<ProjectInfo>, Error> {
        Ok(self.project_root.lock().map_err(poison)?.clone())
    }
    /// 读取需要停机保护的运行摘要，不复制模型、应用快照或点值。
    /// # Errors
    /// 目录或运行状态不可读。
    pub fn project_requires_stop(&self) -> Result<bool, Error> {
        #[cfg(feature = "tcp-runtime")]
        {
            let endpoints: Vec<_> = self
                .workspace
                .lock()
                .map_err(poison)?
                .catalog_ref()
                .endpoints
                .iter()
                .map(|e| (e.id.clone(), e.role))
                .collect();
            for (endpoint, role) in endpoints {
                if role == essim_core::domain::endpoint::Role::Slave {
                    if let Some(runtime) = &self.runtime {
                        use essim_core::application::queries::slave::{SlaveQuery, SlaveState};
                        let view = runtime.query(SlaveQuery {
                            endpoint,
                            unit: None,
                            points: vec![],
                        })?;
                        if !matches!(view.state, SlaveState::Idle | SlaveState::Stopped) {
                            return Ok(true);
                        }
                    }
                } else if let Some(master) = &self.master {
                    let view =
                        master.query(essim_core::application::queries::master::MasterQuery {
                            endpoint,
                            unit: None,
                            points: vec![],
                        })?;
                    if view.state != "disconnected" || view.polling {
                        return Ok(true);
                    }
                }
            }
        }
        Ok(false)
    }
    /// 独立 SQLite 运行记录的积压与故障，不执行数据库查询或磁盘 IO。
    pub fn project_runtime_data(&self) -> Option<super::RuntimeDataView> {
        #[cfg(feature = "tcp-runtime")]
        {
            self.runtime
                .as_ref()
                .and_then(|r| r.project_recording_view())
        }
        #[cfg(not(feature = "tcp-runtime"))]
        {
            None
        }
    }
    /// 按当前打开凭证和资源身份读取用户 Markdown，不接受前端路径。
    /// # Errors
    /// 工程已切换、资源不存在、内容超过 16 MiB 或 UTF-8 格式非法。
    pub fn read_project_document(&self, session: &str, id: &str) -> Result<String, Error> {
        let _project = self.project_gate.read().map_err(poison)?;
        let info = self
            .project_info()?
            .filter(|p| p.session == session)
            .ok_or_else(|| Error::new("PROJECT_CHANGED", "工程已切换"))?;
        let doc = info
            .documents
            .iter()
            .find(|d| d.id == id)
            .ok_or_else(|| Error::new("PROJECT_DOCUMENT", "工程文档不存在"))?;
        let path = super::storage::checked(info.entry.parent().unwrap(), &doc.path)?;
        if std::fs::metadata(&path).map_err(super::storage::io)?.len() > 16 * 1024 * 1024 {
            return Err(Error::new(
                "PROJECT_DOCUMENT_LIMIT",
                "文档预览超过 16 MiB 限制",
            ));
        }
        use std::io::Read;
        let mut text = String::new();
        std::fs::File::open(path)
            .map_err(super::storage::io)?
            .take(16 * 1024 * 1024 + 1)
            .read_to_string(&mut text)
            .map_err(super::storage::io)?;
        if text.len() > 16 * 1024 * 1024 {
            return Err(Error::new(
                "PROJECT_DOCUMENT_LIMIT",
                "文档预览超过 16 MiB 限制",
            ));
        }
        Ok(text)
    }
    /// 在空目录创建全新空工程；当前工程的脏草稿由调用方先保护。
    /// # Errors
    /// 目录非空、配置不合法、当前操作未结束或存储失败。
    pub fn create_project(&self, root: &Path, name: &str) -> Result<ProjectInfo, Error> {
        let _project = self.project_gate.write().map_err(poison)?;
        self.ensure_no_clients()?;
        let (endpoints, permits) = {
            let w = self.workspace.lock().map_err(poison)?;
            let permits = self.stop_permits(w.catalog_ref())?;
            (w.catalog_ref().endpoints.clone(), permits)
        };
        let repository = ProjectRepository::create(root, name, Default::default())?;
        self.replace_project(repository, endpoints, permits)
    }
    /// 打开工程，先完整预检目标，再停止旧工程通信；等待停机时不持有目录锁。
    /// # Errors
    /// 目标不合法、被占用、旧操作未结束、停止或替换失败。
    pub fn open_project(&self, entry: &Path) -> Result<ProjectInfo, Error> {
        if let Some(info) = self.project_info()? {
            if entry.canonicalize().ok().as_ref() == Some(&info.entry) {
                return Ok(info);
            }
        }
        let repository = ProjectRepository::open(entry)?;
        self.activate_project(repository)
    }
    /// 激活已预检且持锁的工程；失败保留当前工程。
    /// # Errors
    /// 工程操作忙碌、停止或可靠记录刷新失败。
    pub fn activate_project(&self, repository: ProjectRepository) -> Result<ProjectInfo, Error> {
        let _project = self.project_gate.write().map_err(poison)?;
        self.ensure_no_clients()?;
        let (endpoints, permits) = {
            let w = self.workspace.lock().map_err(poison)?;
            (
                w.catalog_ref().endpoints.clone(),
                self.stop_permits(w.catalog_ref())?,
            )
        };
        self.replace_project(repository, endpoints, permits)
    }
    fn stop_permits(
        &self,
        catalog: &essim_core::application::catalog::Catalog,
    ) -> Result<Vec<crate::operation_gates::OperationPermit>, Error> {
        catalog
            .endpoints
            .iter()
            .map(|e| self.operation_gates().acquire(&e.id))
            .collect()
    }
    fn replace_project(
        &self,
        repository: ProjectRepository,
        endpoints: Vec<essim_core::domain::endpoint::EndpointConfig>,
        permits: Vec<crate::operation_gates::OperationPermit>,
    ) -> Result<ProjectInfo, Error> {
        static NEXT_SESSION: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(1);
        let info = ProjectInfo {
            documents: repository
                .manifest()
                .resources
                .iter()
                .filter(|r| r.kind == super::ResourceKind::Document)
                .map(|r| ProjectDocumentInfo {
                    id: r.id.clone(),
                    name: r.display_name.clone(),
                    path: r.path.clone(),
                })
                .collect(),
            session: format!(
                "{}:{}",
                std::process::id(),
                NEXT_SESSION.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
            ),
            id: repository.manifest().project_id.clone(),
            name: repository.manifest().display_name.clone(),
            entry: repository.root().join("project.workspace"),
        };
        let repository = BudgetRepository::new(repository, self.resources.clone())?;
        self.stop_project(&endpoints, &permits)?;
        #[cfg(feature = "tcp-runtime")]
        if let Some(runtime) = &self.runtime {
            runtime.flush_project_recording()?;
            runtime.set_project_recording(Some(info.entry.parent().unwrap().to_path_buf()))?;
        }
        let mut w = self.workspace.lock().map_err(poison)?;
        w.replace_repository(Box::new(repository))?;
        self.partition_plans.0.lock().map_err(poison)?.clear();
        *self.project_root.lock().map_err(poison)? = Some(info.clone());
        Ok(info)
    }
    /// 关闭工程，确认通信资源和记录刷新后回到工程引导。
    /// # Errors
    /// 活动操作、停机或刷新未确认时保留旧工程；已停止的通信不自动重启。
    pub fn close_project(&self) -> Result<(), Error> {
        let _project = self.project_gate.write().map_err(poison)?;
        self.ensure_no_clients()?;
        let (endpoints, permits) = {
            let w = self.workspace.lock().map_err(poison)?;
            (
                w.catalog_ref().endpoints.clone(),
                self.stop_permits(w.catalog_ref())?,
            )
        };
        self.stop_project(&endpoints, &permits)?;
        #[cfg(feature = "tcp-runtime")]
        if let Some(runtime) = &self.runtime {
            runtime.flush_project_recording()?;
            runtime.set_project_recording(None)?;
        }
        let mut w = self.workspace.lock().map_err(poison)?;
        w.replace_repository(Box::new(MemoryRepository::with_resources(
            self.resources.clone(),
        )))?;
        self.partition_plans.0.lock().map_err(poison)?.clear();
        *self.project_root.lock().map_err(poison)? = None;
        Ok(())
    }
    fn stop_project(
        &self,
        endpoints: &[essim_core::domain::endpoint::EndpointConfig],
        permits: &[crate::operation_gates::OperationPermit],
    ) -> Result<(), Error> {
        #[cfg(feature = "tcp-runtime")]
        if self.runtime.is_some() || self.master.is_some() {
            for (e, permit) in endpoints.iter().zip(permits) {
                use essim_core::{
                    application::commands::slave::{SlaveAction, SlaveCommand},
                    domain::endpoint::Role,
                };
                if e.role == Role::Slave {
                    if let Some(runtime) = &self.runtime {
                        runtime.execute_with_permit(
                            SlaveCommand {
                                command_id: "project-switch-stop".into(),
                                endpoint: e.id.clone(),
                                action: SlaveAction::Stop,
                            },
                            None,
                            Some(permit),
                        )?;
                        runtime.forget_with_permit(&e.id, Some(permit))?;
                    }
                } else if let Some(master) = &self.master {
                    master.forget_permitted(&e.id, Some(permit))?;
                }
            }
            return Ok(());
        }
        let _ = permits;
        let mut boundary = self.boundary.lock().map_err(poison)?;
        for e in endpoints {
            boundary.stop(e)?;
        }
        Ok(())
    }
}

fn poison<T>(_: T) -> Error {
    Error::new("STATE_POISONED", "工程状态不可用")
}
