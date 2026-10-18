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
