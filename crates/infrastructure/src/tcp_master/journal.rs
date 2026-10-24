//! 主站自包含JSONL记录；失败不降级为无记录继续通信。
use super::*;
use std::{
    path::{Path, PathBuf},
    sync::atomic::AtomicU64,
};
static NEXT: AtomicU64 = AtomicU64::new(0);
pub(super) struct Journal {
    stream: crate::recording::RecordStream,
    chunks: u64,

    pub path: String,
}
impl Journal {
    pub fn create(
        root: &Path,
        snapshot: &MasterSnapshot,
        cache: &BTreeMap<UnitId, BTreeMap<PointId, MasterPoint>>,
        pool: &crate::recording::RecordingPool,
    ) -> Result<Self, Error> {
        let path: PathBuf = root.join(format!(
            "master-{}-{}.jsonl",
            now(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        let stream = pool.open(path.clone())?;
        let path = stream.path().unwrap_or(&path).to_path_buf();
        let mut journal = Self {
            stream,

            chunks: 0,
            path: path.to_string_lossy().into_owned(),
        };
        journal
            .write(&serde_json::json!({"kind":"masterSnapshot","schema":1,"snapshot":snapshot,"cache":cache}))?;
        Ok(journal)
    }
    pub fn write(&mut self, value: &impl Serialize) -> Result<(), Error> {
        crate::recording::write_json(&self.stream, &mut self.chunks, value)
    }
}
