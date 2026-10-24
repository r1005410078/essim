//! 分组预览的会话缓存和本机IP校验；生成配置由core一次事务完成。
use crate::workspace::WorkspaceEngine;
use essim_core::{
    application::{
        commands::{
            partition::{PartitionPlan, PartitionRequest},
            Receipt,
        },
        queries::partition::PartitionPreview,
    },
    domain::error::Error,
};
use serde::Serialize;
use std::{
    collections::BTreeMap,
    sync::Mutex,
    time::{Duration, Instant},
};
/// 一个尚未配置的目标IP；仅阻止创建，保留分组预览。
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LocalIssue {
    /// 冲突分组IP。
    pub ip: String,
    /// 可定位的冲突说明。
    pub message: String,
}
/// 返回界面的有界预览，不发送源完整点表。
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Preview {
    /// 会话内一次规划身份，提交后失效。
    pub token: String,
    /// 全部分组通过当前本机IP检查才允许创建。
    pub can_create: bool,
    /// 未配置的IP及定位原因。
    pub local_issues: Vec<LocalIssue>,
    /// 分组摘要。
    pub preview: PartitionPreview,
    /// 本机已配置的IPv4。
    pub local_ips: Vec<String>,
    /// 本次源／生成对象暂存估计，不含无关目录深复制或RSS。
    pub estimated_memory_bytes: u64,
}
#[derive(Default)]
pub(crate) struct Plans(
    pub Mutex<BTreeMap<String, (Instant, PartitionPlan, crate::resources::Reservation)>>,
);
fn inspect_local(plan: &PartitionPlan) -> Result<(Vec<String>, Vec<LocalIssue>), Error> {
    let ips = crate::local_interfaces::ipv4_addresses()
        .map_err(|e| Error::new("INTERFACE_QUERY_FAILED", e))?;
    let mut issues = Vec::new();
    for group in &plan.preview().groups {
        let ip = group
            .ip
            .parse()
            .map_err(|_| Error::new("INVALID_IPV4", "无效分组IP"))?;
        if !ips.contains(&ip) {
            issues.push(LocalIssue {
                ip: group.ip.clone(),
                message: format!("{}：IP {}未配置在本机", group.name, group.ip),
            });
        }
    }
    Ok((ips.into_iter().map(|ip| ip.to_string()).collect(), issues))
}
fn check_local(plan: &PartitionPlan) -> Result<(), Error> {
    let (_, issues) = inspect_local(plan)?;
    if let Some(issue) = issues.first() {
        return Err(Error::new("IP_NOT_LOCAL", issue.message.clone()));
    }
    Ok(())
}
impl WorkspaceEngine {
    /// 规划并缓存一次分组预览；最多保留一个计划，5分钟失效。
    /// # Errors
    /// 参数、目录冲突、接口枚举失败、预览过大或状态不可用时拒绝；未配置IP随预览返回。
    pub fn preview_partition(&self, request: PartitionRequest) -> Result<Preview, Error> {
        let _project = self.project_operation()?;
        let mut plans = self
            .partition_plans
            .0
            .lock()
            .map_err(|_| Error::new("STATE_POISONED", "规划状态不可用"))?;
        plans.clear();
        let (plan, lease, estimated_memory_bytes) = {
            let workspace = self
                .workspace
                .lock()
                .map_err(|_| Error::new("STATE_POISONED", "配置不可用"))?;
            let estimated_memory_bytes =
                crate::resources::serialized_size(workspace.saved_model(&request.source)?)?
                    .saturating_mul(8);
            let lease = self
                .resources
                .reserve(essim_core::domain::resources::ResourceUsage {
                    memory_bytes: estimated_memory_bytes,
                    ..Default::default()
                })?;
            (
                workspace.prepare_partition(request)?,
                lease,
                estimated_memory_bytes,
            )
        };
        let (local_ips, local_issues) = inspect_local(&plan)?;
        let can_create = local_issues.is_empty();
        let preview = plan.preview().clone();
        let token = plan.request().batch_id.clone();
        let response = Preview {
            token: token.clone(),
            can_create,
            local_issues,
            preview,
            local_ips,
            estimated_memory_bytes,
        };
        if crate::resources::serialized_size(&response)?
            > self.resources.policy().preview_bytes as u64
        {
            return Err(Error::new("PREVIEW_TOO_LARGE", "预览超过8MiB，请调整分组"));
        }
        if can_create {
            plans.insert(token, (Instant::now(), plan, lease));
        }
        Ok(response)
    }

    /// 丢弃会话预览并释放其暂存额度；重复调用无副作用。
    /// # Errors
    /// 规划状态不可用。
    pub fn discard_partition(&self, token: &str) -> Result<(), Error> {
        self.partition_plans
            .0
            .lock()
            .map_err(|_| Error::new("STATE_POISONED", "规划状态不可用"))?
            .remove(token);
        Ok(())
    }
    /// 消费一次预览并原子创建分组；IP、源模型及冲突再次校验。
    /// # Errors
    /// 预览缺失／过期或提交失败，原配置保持不变。
    pub fn create_partition(&self, token: &str) -> Result<Receipt, Error> {
        let _project = self.project_operation()?;
        let (created, plan, _lease) = self
            .partition_plans
            .0
            .lock()
            .map_err(|_| Error::new("STATE_POISONED", "规划状态不可用"))?
            .remove(token)
            .ok_or_else(|| Error::new("PREVIEW_EXPIRED", "规划不存在，请重新预览"))?;
        if created.elapsed() > Duration::from_secs(300) {
            return Err(Error::new("PREVIEW_EXPIRED", "规划已过期，请重新预览"));
        }
        check_local(&plan)?;
        let (receipt, _) = self
            .workspace
            .lock()
            .map_err(|_| Error::new("STATE_POISONED", "配置不可用"))?
            .commit_partition(plan, format!("partition:{token}"))?;
        Ok(receipt)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use essim_core::{
        application::commands::{Action, Command},
        domain::resources::ResourceUsage,
    };
    fn engine() -> WorkspaceEngine {
        let e = WorkspaceEngine::default();
        let model=serde_json::from_value(serde_json::json!({"id":"m","name":"M","points":[{"id":"p","name":"P","category":"measurement","area":"holding","address":0,"valueType":"u16","scale":"1","unit":"","initial":{"kind":"unsigned","value":"7"},"initialEngineering":"7","byteOrder":"big","wordOrder":"big","note":"","slaveWritable":true,"masterPeriodMs":1000}]})).unwrap();
        e.execute(Command {
            command_id: "save".into(),
            action: Action::SaveModel { model },
        })
        .unwrap();
        e
    }
    fn request(batch: &str) -> PartitionRequest {
        serde_json::from_value(serde_json::json!({"source":"m","batchId":batch,"prefix":"组","groupSize":1000,"startIp":"127.0.0.1","port":1080,"station":1,"folder":null})).unwrap()
    }
    #[test]
    fn exhausted_budget_leaves_no_objects_or_extra_reservations() {
        let e = engine();
        let before = e.query().unwrap().catalog;
        let _busy = e
            .resources
            .reserve(ResourceUsage {
                memory_bytes: e.resources.policy().limit.memory_bytes
                    - e.resources.query().unwrap().current.memory_bytes,
                ..Default::default()
            })
            .unwrap();
        assert_eq!(
            e.preview_partition(request("b")).unwrap_err().code,
            "RESOURCE_LIMIT"
        );
        assert_eq!(e.query().unwrap().catalog, before);
        assert_eq!(e.resources.query().unwrap().reservations, 2);
    }
    #[test]
    fn stale_close_cannot_release_a_new_preview_and_error_releases_lease() {
        let e = engine();
        let old = e.preview_partition(request("old")).unwrap();
        let new = e.preview_partition(request("new")).unwrap();
        e.discard_partition(&old.token).unwrap();
        assert_eq!(e.resources.query().unwrap().reservations, 2);
        e.create_partition(&new.token).unwrap();
        assert_eq!(e.resources.query().unwrap().reserved.memory_bytes, 0);
        let mut bad = request("bad");
        bad.start_ip = "192.0.2.1".into();
        bad.prefix = "未配置IP".into();
        let preview = e.preview_partition(bad).unwrap();
        assert!(!preview.can_create);
        assert_eq!(preview.local_issues.len(), 1);
        assert_eq!(e.resources.query().unwrap().reserved.memory_bytes, 0);
    }
}
