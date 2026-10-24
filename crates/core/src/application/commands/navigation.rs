//! 导航写入用例；跨聚合检查端点存在性，目录不变量由领域维护。
use super::Handled;
use crate::application::{catalog::Catalog, events::ConfigurationFact};
use crate::domain::{
    error::Error,
    identity::{EndpointId, FolderId},
    navigation::Folder,
};
pub(crate) fn save_folder(catalog: &mut Catalog, folder: Folder) -> Result<Handled, Error> {
    let id = folder.id.clone();
    catalog.navigation.save_folder(folder)?;
    Ok(Handled {
        object_id: id.0.clone(),
        fact: ConfigurationFact::FolderSaved { id },
    })
}
pub(crate) fn delete_folder(catalog: &mut Catalog, id: FolderId) -> Result<Handled, Error> {
    catalog.navigation.delete_folder(&id)?;
    Ok(Handled {
        object_id: id.0.clone(),
        fact: ConfigurationFact::FolderDeleted { id },
    })
}
pub(crate) fn move_endpoint(
    catalog: &mut Catalog,
    id: EndpointId,
    parent: Option<FolderId>,
) -> Result<Handled, Error> {
    if !catalog.endpoints.iter().any(|e| e.id == id) {
        return Err(Error::new("NOT_FOUND", "模拟器不存在"));
    }
    catalog
        .navigation
        .move_endpoint(id.clone(), parent.clone())?;
    Ok(Handled {
        object_id: id.0.clone(),
        fact: ConfigurationFact::EndpointMoved { id, parent },
    })
}

/// 核心先检查所有对象，再由导航领域原子移动。
pub(crate) fn move_nodes(
    catalog: &mut Catalog,
    nodes: Vec<crate::domain::navigation::NavigationNode>,
    scope: crate::domain::navigation::FolderScope,
    parent: Option<FolderId>,
) -> Result<Handled, Error> {
    use crate::domain::navigation::NavigationNode;
    for node in &nodes {
        let exists = match node {
            NavigationNode::Folder { id } => catalog.navigation.folders.iter().any(|f| &f.id == id),
            NavigationNode::Endpoint { id } => catalog.endpoints.iter().any(|e| &e.id == id),
            NavigationNode::Model { id } => catalog.models.iter().any(|m| &m.id == id),
        };
        if !exists {
            return Err(Error::new("NOT_FOUND", "移动对象已不存在"));
        }
    }
    catalog.navigation.move_nodes(&nodes, scope, parent)?;
    Ok(Handled {
        object_id: "navigation".into(),
        fact: ConfigurationFact::NavigationMoved,
    })
}

/// 新资源及父目录在同一配置事务中提交，失败不产生半成品。
pub(crate) fn create_in_folder(
    catalog: &mut Catalog,
    parent: Option<FolderId>,
    resource: NewNavigationResource,
) -> Result<Handled, Error> {
    use crate::domain::navigation::{FolderScope, NavigationNode};
    match resource {
        NewNavigationResource::Model { model } => {
            catalog
                .navigation
                .check_parent(FolderScope::Models, parent.as_ref())?;
            if catalog.models.iter().any(|m| m.id == model.id) {
                return Err(Error::new("DUPLICATE_ID", "模型身份已存在"));
            }
            let id = model.id.clone();
            let handled = super::model::save_model(catalog, model)?;
            catalog.navigation.move_nodes(
                &[NavigationNode::Model { id }],
                FolderScope::Models,
                parent,
            )?;
            Ok(handled)
        }
        NewNavigationResource::Endpoint { endpoint, model } => {
            catalog
                .navigation
                .check_parent(FolderScope::Simulators, parent.as_ref())?;
            let id = endpoint.id.clone();
            let handled = if let Some(model) = model {
                super::overrides::create(catalog, endpoint, model)?
            } else {
                super::endpoint::create_endpoint(catalog, endpoint)?
            };
            catalog.navigation.move_endpoint(id, parent)?;
            Ok(handled)
        }
    }
}
/// 新建资源的限定载荷；不能嵌套任意写入命令。
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum NewNavigationResource {
    /// 新建空模型或已完成导入校验的模型。
    Model {
        /// 模型内容。
        model: crate::domain::model::CollectionModel,
    },
    /// 创建模拟器，可同时创建其首个Unit引用的模型。
    Endpoint {
        /// 模拟器配置。
        endpoint: crate::domain::endpoint::EndpointConfig,
        /// 可选的新模型，已有引用时为空。
        #[serde(default)]
        model: Option<crate::domain::model::CollectionModel>,
    },
}
