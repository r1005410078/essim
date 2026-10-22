//! 完整业务点分组，不访问配置仓储或系统接口。
use crate::domain::{error::Error, model::CollectionModel, slave::area_index};
/// 返回按数据区／地址／稳定身份排序的源点下标；不拆多寄存器点。
/// # Errors
/// 源模型非法、空表或组大小不合法时拒绝。
pub fn ordered_points(model: &CollectionModel, group_size: usize) -> Result<Vec<usize>, Error> {
    if group_size == 0 {
        return Err(Error::new("INVALID_PARTITION", "每组点数必须为正整数"));
    }
    model.validate()?;
    if model.points.is_empty() {
        return Err(Error::new("EMPTY_MODEL", "空模型不能分组"));
    }
    let mut indices: Vec<_> = (0..model.points.len()).collect();
    indices.sort_by_key(|i| {
        let p = &model.points[*i];
        (area_index(p.area), p.address, p.id.clone())
    });
    Ok(indices)
}
