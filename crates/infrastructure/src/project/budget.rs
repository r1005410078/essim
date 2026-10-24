//! 将文件仓储接入原有资源准入，不把文件 IO 引入 core。
use super::ProjectRepository;
use crate::resources::{catalog_memory, Reservation, ResourceManager};
use essim_core::{
    application::{catalog::Catalog, ports::ConfigRepository},
    domain::{error::Error, resources::ResourceUsage},
};
pub(crate) struct BudgetRepository {
    inner: ProjectRepository,
    resources: ResourceManager,
    charge: Reservation,
}
impl BudgetRepository {
    pub(crate) fn new(inner: ProjectRepository, resources: ResourceManager) -> Result<Self, Error> {
        let charge = resources.reserve(ResourceUsage {
            memory_bytes: catalog_memory(inner.catalog_ref())?,
            ..Default::default()
        })?;
        charge.commit();
        Ok(Self {
            inner,
            resources,
            charge,
        })
    }
}
impl ConfigRepository for BudgetRepository {
    fn ensure_available(&self) -> Result<(), Error> {
        self.inner.ensure_available()
    }
    fn reserve_staging(&self) -> Result<Box<dyn Send>, Error> {
        Ok(Box::new(self.resources.reserve(ResourceUsage {
            memory_bytes: catalog_memory(self.inner.catalog_ref())?,
            ..Default::default()
        })?))
    }
    fn load(&self) -> Catalog {
        self.inner.load()
    }
    fn commit(&mut self, catalog: Catalog) -> Result<(), Error> {
        let charge = self.resources.reserve(ResourceUsage {
            memory_bytes: catalog_memory(&catalog)?,
            ..Default::default()
        })?;
        self.inner.commit(catalog)?;
        charge.commit();
        self.charge = charge;
        Ok(())
    }
}
