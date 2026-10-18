//! 未装配生产能力时的运行入口门禁。
use super::Handled;
use crate::application::{catalog::Catalog, queries::Capabilities};
use crate::domain::{error::Error, identity::EndpointId};
pub(crate) fn start(catalog: &Catalog, id: EndpointId) -> Result<Handled, Error> {
    let endpoint = catalog
        .endpoints
        .iter()
        .find(|e| e.id == id)
        .ok_or_else(|| Error::new("NOT_FOUND", "模拟器不存在"))?;
    if !endpoint.units.iter().any(|u| u.enabled) {
        return Err(Error::new("NO_ENABLED_UNIT", "至少启用一个Unit后才能启动"));
    }
    return Err(Error::new(
        "CAPABILITY_UNAVAILABLE",
        Capabilities::default().reason,
    ));
}
pub(crate) fn unavailable() -> Result<Handled, Error> {
    Err(Error::new(
        "CAPABILITY_UNAVAILABLE",
        Capabilities::default().reason,
    ))
}
