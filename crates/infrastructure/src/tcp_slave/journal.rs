//! 自包含运行文件：每条确认经过sync_data，失败不降级为无录制继续运行。
use super::{failure, mapping::UnitMapping};
use essim_core::domain::{
    endpoint::EndpointConfig,
    identity::{PointId, UnitId},
    model::CollectionModel,
    point_table::RawValue,
};
use essim_core::{application::events::slave::SlaveFact, domain::error::Error};
use serde::Serialize;
use std::collections::BTreeMap;
use std::{
    fs::{self, File, OpenOptions},
    io::{BufWriter, Write},
    path::Path,
    sync::atomic::{AtomicU64, Ordering},
    time::{SystemTime, UNIX_EPOCH},
};
static NEXT: AtomicU64 = AtomicU64::new(0);
pub(super) struct Journal {
    file: File,
    pub path: String,
    pub sequence: u64,
    pub unix_ms: String,
    started: std::time::Instant,
    #[cfg(test)]
    pub fail_after: Option<u64>,
}
impl Journal {
    pub fn create(
        root: &Path,
        endpoint: &EndpointConfig,
        mappings: &BTreeMap<u8, (UnitId, UnitMapping)>,
    ) -> Result<Self, Error> {
        #[derive(Serialize)]
        #[serde(rename_all = "camelCase")]
        struct UnitStart<'a> {
            id: &'a UnitId,
            station: u8,
            model_index: usize,
            raw: BTreeMap<PointId, RawValue>,
        }
        #[derive(Serialize)]
        struct Start<'a> {
            kind: &'static str,
            schema: u32,
            run: String,
            endpoint: &'a EndpointConfig,
            models: Vec<&'a CollectionModel>,
            units: Vec<UnitStart<'a>>,
        }
        let mut models: Vec<&CollectionModel> = Vec::new();
        let mut units = Vec::new();
        for (station, (id, mapping)) in mappings {
            let model = mapping.layout.model.as_ref();
            let index = if let Some(index) = models.iter().position(|old| std::ptr::eq(*old, model))
            {
                index
            } else {
                models.push(model);
                models.len() - 1
            };
            units.push(UnitStart {
                id,
                station: *station,
                model_index: index,
                raw: mapping.raw_values()?,
            });
        }

        fs::create_dir_all(root).map_err(failure)?;
        let stamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos();
        let path = root.join(format!(
            "run-{stamp}-{}.jsonl",
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        let snapshot = Start {
            kind: "snapshot",
            schema: 1,
            run: path.file_stem().unwrap().to_string_lossy().into_owned(),
            endpoint,
            models,
            units,
        };
        let file = OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&path)
            .map_err(failure)?;
        let mut journal = Self {
            file,
            path: path.to_string_lossy().into_owned(),
            sequence: 0,
            unix_ms: String::new(),
            started: std::time::Instant::now(),
            #[cfg(test)]
            fail_after: None,
        };
        journal.write(&snapshot)?;
        // 同步目录项，避免仅同步文件内容却未可靠建立运行记录。
        File::open(root)
            .and_then(|dir| dir.sync_all())
            .map_err(failure)?;
        Ok(journal)
    }
    fn write(&mut self, value: &impl Serialize) -> Result<(), Error> {
        let result = (|| -> Result<(), Box<dyn std::error::Error>> {
            let mut buffer = BufWriter::with_capacity(64 * 1024, &mut self.file);
            serde_json::to_writer(&mut buffer, value)?;
            buffer.write_all(b"\n")?;
            buffer.flush()?;
            Ok(())
        })();
        result.map_err(|e| Error::new("RECORDING_FAILED", format!("运行记录写入失败：{e}")))?;
        self.file
            .sync_data()
            .map_err(|e| Error::new("RECORDING_FAILED", format!("运行记录同步失败：{e}")))
    }
    pub fn record(&mut self, command: Option<&str>, fact: &SlaveFact) -> Result<(), Error> {
        #[cfg(test)]
        if self.fail_after.is_some_and(|limit| self.sequence >= limit) {
            return Err(Error::new("RECORDING_FAILED", "注入可靠写入失败"));
        }
        let next = self
            .sequence
            .checked_add(1)
            .ok_or_else(|| Error::new("SEQUENCE_OVERFLOW", "运行记录序号耗尽"))?;
        #[derive(Serialize)]
        #[serde(rename_all = "camelCase")]
        struct Entry<'a> {
            sequence: String,
            relative_ms: String,
            unix_ms: String,
            command_id: Option<&'a str>,
            fact: &'a SlaveFact,
        }
        let unix_ms = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis()
            .to_string();
        self.write(&Entry {
            sequence: next.to_string(),
            relative_ms: self.started.elapsed().as_millis().to_string(),
            unix_ms: unix_ms.clone(),
            command_id: command,
            fact,
        })?;
        self.sequence = next;
        self.unix_ms = unix_ms;
        Ok(())
    }
}
