//! 主站自包含JSONL记录；失败不降级为无记录继续通信。
use super::*;
use std::{
    fs::{File, OpenOptions},
    io::Write,
    path::{Path, PathBuf},
    sync::atomic::AtomicU64,
};
static NEXT: AtomicU64 = AtomicU64::new(0);
pub(super) struct Journal {
    file: File,
    pub path: String,
}
impl Journal {
    pub fn create(
        root: &Path,
        snapshot: &MasterSnapshot,
        cache: &BTreeMap<UnitId, BTreeMap<PointId, MasterPoint>>,
    ) -> Result<Self, Error> {
        std::fs::create_dir_all(root).map_err(failure)?;
        let path: PathBuf = root.join(format!(
            "master-{}-{}.jsonl",
            now(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        let file = OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&path)
            .map_err(failure)?;
        let mut journal = Self {
            file,
            path: path.to_string_lossy().into_owned(),
        };
        journal
            .write(&serde_json::json!({"kind":"masterSnapshot","schema":1,"snapshot":snapshot,"cache":cache}))?;
        File::open(root)
            .and_then(|f| f.sync_all())
            .map_err(failure)?;
        Ok(journal)
    }
    pub fn write(&mut self, value: &impl Serialize) -> Result<(), Error> {
        serde_json::to_writer(&mut self.file, value).map_err(failure)?;
        self.file
            .write_all(b"\n")
            .and_then(|_| self.file.sync_data())
            .map_err(|e| Error::new("RECORDING_FAILED", format!("主站记录失败：{e}")))
    }
}
