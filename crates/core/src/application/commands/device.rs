//! 可选设备创建用例。
use super::Handled;
use crate::application::{catalog::Catalog, events::ConfigurationFact};
use crate::domain::{device::Device, error::Error};
pub(crate) fn create_device(catalog: &mut Catalog, device: Device) -> Result<Handled, Error> {
    let fact = ConfigurationFact::DeviceCreated {
        id: device.id.clone(),
    };
    let object_id = {
        if device.id.0.is_empty()
            || device.name.trim().is_empty()
            || device.kind.trim().is_empty()
            || catalog.devices.iter().any(|old| old.id == device.id)
        {
            return Err(Error::new("INVALID_DEVICE", "设备身份重复或名称／类型为空"));
        }
        let result = device.id.0.clone();
        catalog.devices.push(device);
        result
    };
    Ok(Handled { object_id, fact })
}
