//! 点位分组配置用例；准备与提交分离，准备不改目录。
use crate::{
    application::{
        catalog::Catalog,
        queries::partition::{PartitionGroup, PartitionPreview},
    },
    domain::{
        endpoint::{EndpointConfig, Role, Transport, UnitConfig},
        error::Error,
        identity::{EndpointId, FolderId, ModelId, PointId, UnitId},
        model::{CollectionModel, ModelRef},
    },
};
use serde::{Deserialize, Serialize};
use std::{collections::BTreeSet, net::Ipv4Addr};
/// 分组输入；batchId仅为一次创建生成新身份，不是配置版本。
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PartitionRequest {
    /// 当前已保存源模型。
    pub source: ModelId,
    /// 本次创建的不透明身份。
    pub batch_id: String,
    /// 分组名称前缀。
    pub prefix: String,
    /// 每组完整业务点数。
    pub group_size: usize,
    /// 起始具体IPv4；属于本机由基础设施校验。
    pub start_ip: String,
    /// 所有分组共用TCP端口。
    pub port: u16,
    /// 所有分组共用线上站号。
    pub station: u8,
    /// 目标文件夹；空表示根目录。
    pub folder: Option<FolderId>,
}
/// 不可变准备结果；包含一份源内容用于提交前比较，不向界面发送整表。
pub struct PartitionPlan {
    request: PartitionRequest,
    source: CollectionModel,
    indices: Vec<usize>,
    preview: PartitionPreview,
}
impl PartitionPlan {
    /// 获取有界的分组摘要。
    pub fn preview(&self) -> &PartitionPreview {
        &self.preview
    }
    /// 本次操作输入。
    pub fn request(&self) -> &PartitionRequest {
        &self.request
    }
}
fn id(request: &PartitionRequest, index: usize, suffix: &str) -> String {
    format!("{}:group:{index}:{suffix}", request.batch_id)
}
fn check_catalog(
    catalog: &Catalog,
    request: &PartitionRequest,
    groups: &[PartitionGroup],
) -> Result<(), Error> {
    if request
        .folder
        .as_ref()
        .is_some_and(|id| !catalog.navigation.folders.iter().any(|f| &f.id == id))
    {
        return Err(Error::new("FOLDER_NOT_FOUND", "目标文件夹不存在"));
    }
    for (index, g) in groups.iter().enumerate() {
        let name = format!("{}-分组{:03}", request.prefix.trim(), index + 1);
        if catalog
            .models
            .iter()
            .any(|m| m.id.0 == id(request, index, "model") || m.name == name)
            || catalog
                .endpoints
                .iter()
                .any(|e| e.id.0 == id(request, index, "endpoint") || e.name == name)
            || catalog
                .endpoints
                .iter()
                .chain(&catalog.applied)
                .any(|e| e.units.iter().any(|u| u.id.0 == id(request, index, "unit")))
        {
            return Err(Error::new(
                "PARTITION_CONFLICT",
                format!("分组{}身份或名称已存在", index + 1),
            ));
        }
        if catalog.endpoints.iter().chain(&catalog.applied).filter(|e|e.role==Role::Slave).any(|e|matches!(&e.transport,Transport::Tcp{address,port}if *port==request.port && (address==&g.ip||address=="0.0.0.0"))){return Err(Error::new("BINDING_CONFLICT",format!("分组{}监听配置{}:{}已占用",index+1,g.ip,request.port)));}
    }
    Ok(())
}
/// 校验源表并规划分组，最多1024组；无网络或配置副作用。
/// # Errors
/// 源模型／参数／目录／地址递增或身份、名称、监听配置冲突时拒绝。
pub fn prepare_partition(
    catalog: &Catalog,
    request: PartitionRequest,
) -> Result<PartitionPlan, Error> {
    if request.group_size == 0
        || request.prefix.trim().is_empty()
        || request.batch_id.trim().is_empty()
        || request.batch_id.len() > 128
        || request.prefix.len() > 256
        || request.port == 0
        || !(1..=247).contains(&request.station)
    {
        return Err(Error::new(
            "INVALID_PARTITION",
            "组大小、名称、身份、端口或站号非法",
        ));
    }
    let source = catalog
        .models
        .iter()
        .find(|m| m.id == request.source)
        .ok_or_else(|| Error::new("MODEL_NOT_FOUND", "源模型不存在"))?;
    let indices = crate::domain::partition::ordered_points(source, request.group_size)?;
    let count = source.points.len().div_ceil(request.group_size);
    if count > 1024 {
        return Err(Error::new(
            "PARTITION_LIMIT",
            "单次最多1024组，请调整每组点数",
        ));
    }
    let start: Ipv4Addr = request
        .start_ip
        .parse()
        .map_err(|_| Error::new("INVALID_IPV4", "起始IPv4非法"))?;
    let mut groups = Vec::new();
    for (index, chunk) in indices.chunks(request.group_size).enumerate() {
        let ip = Ipv4Addr::from(
            u32::from(start)
                .checked_add(index as u32)
                .ok_or_else(|| Error::new("IPV4_OVERFLOW", "IPv4递增溢出"))?,
        );
        if ip.is_unspecified()
            || ip.is_multicast()
            || ip.is_broadcast()
            || ip.octets()[0] == 0
            || ip.octets()[0] >= 240
        {
            return Err(Error::new(
                "INVALID_IPV4",
                format!("目标IPv4不可用于具体监听：{ip}"),
            ));
        }
        let mut ranges: Vec<(String, u16, u16)> = Vec::new();
        for i in chunk {
            let p = &source.points[*i];
            let area = format!("{:?}", p.area).to_lowercase();
            let end = (u32::from(p.address) + p.value_type.width() - 1) as u16;
            if let Some(last) = ranges.last_mut() {
                if last.0 == area && u32::from(last.2) + 1 == u32::from(p.address) {
                    last.2 = end;
                    continue;
                }
            }
            ranges.push((area, p.address, end));
        }
        groups.push(PartitionGroup {
            ip: ip.to_string(),
            port: request.port,
            station: request.station,
            name: format!("{}-分组{:03}", request.prefix.trim(), index + 1),
            points: chunk.len(),
            ranges,
        });
    }
    check_catalog(catalog, &request, &groups)?;
    let preview = PartitionPreview {
        source: request.source.clone(),
        total_points: indices.len(),
        groups,
    };
    Ok(PartitionPlan {
        request,
        source: source.clone(),
        indices,
        preview,
    })
}
/// 在事务暂存目录中一次生成全部对象；提交前源内容及冲突重新校验。
/// # Errors
/// 源内容改变、目录失效或冲突时不改变目录。
pub fn commit_partition(
    catalog: &mut Catalog,
    plan: PartitionPlan,
) -> Result<Vec<EndpointId>, Error> {
    if catalog.models.iter().find(|m| m.id == plan.request.source) != Some(&plan.source) {
        return Err(Error::new("PARTITION_STALE", "源模型已改变，请重新预览"));
    }
    check_catalog(catalog, &plan.request, &plan.preview.groups)?;
    let used_points: BTreeSet<_> = catalog
        .models
        .iter()
        .flat_map(|m| m.points.iter().map(|p| p.id.clone()))
        .collect();
    let mut models = Vec::new();
    let mut endpoints = Vec::new();
    for (index, chunk) in plan.indices.chunks(plan.request.group_size).enumerate() {
        let group = &plan.preview.groups[index];
        let model_id = ModelId(id(&plan.request, index, "model"));
        let mut points = Vec::new();
        for (offset, i) in chunk.iter().enumerate() {
            let mut point = plan.source.points[*i].clone();
            point.id = PointId(format!("{}:point:{offset}", model_id.0));
            if used_points.contains(&point.id) {
                return Err(Error::new("DUPLICATE_POINT_ID", "生成点身份已存在"));
            }
            points.push(point);
        }
        let model = CollectionModel {
            id: model_id.clone(),
            name: group.name.clone(),
            points,
        };
        model.validate()?;
        let endpoint = EndpointConfig {
            id: EndpointId(id(&plan.request, index, "endpoint")),
            name: group.name.clone(),
            note: String::new(),
            role: Role::Slave,
            transport: Transport::Tcp {
                address: group.ip.clone(),
                port: group.port,
            },
            master: Default::default(),
            units: vec![UnitConfig {
                overrides: Default::default(),
id: UnitId(id(&plan.request, index, "unit")),
                name: format!("Unit {}", group.station),
                note: String::new(),
                station: group.station,
                enabled: true,
                model: ModelRef { id: model_id },
                device: None,
            }],
        };
        endpoint.validate()?;
        models.push(model);
        endpoints.push(endpoint);
    }
    let ids = endpoints.iter().map(|e| e.id.clone()).collect::<Vec<_>>();
    // 所有可能失败的校验已结束；一次发布生成对象，调用方再原子提交仓储。
    if let Some(folder) = &plan.request.folder {
        for id in &ids {
            catalog
                .navigation
                .members
                .insert(id.clone(), folder.clone());
        }
        for model in &models {
            catalog
                .navigation
                .model_members
                .insert(model.id.clone(), folder.clone());
        }
    }
    catalog.models.extend(models);
    catalog.endpoints.extend(endpoints);
    Ok(ids)
}
