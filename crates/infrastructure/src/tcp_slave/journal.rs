//! 自包含运行文件：每条确认经过sync_data，失败不降级为无录制继续运行。
use super::mapping::UnitMapping;
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
    path::Path,
    sync::atomic::{AtomicU64, Ordering},
    time::{SystemTime, UNIX_EPOCH},
};
static NEXT: AtomicU64 = AtomicU64::new(0);
pub(super) struct Journal {
    stream: crate::recording::RecordStream,
    chunks: u64,

    pub path: String,
    pub sequence: u64,
    pub unix_ms: String,
    started: std::time::Instant,
    #[cfg(test)]
    pub fail_after: Option<u64>,
}
impl Journal {
    pub async fn create(
        root: &Path,
        endpoint: &EndpointConfig,
        mappings: &BTreeMap<u8, (UnitId, UnitMapping)>,
        pool: &crate::recording::RecordingPool,
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

        let stamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos();
        let path = root.join(format!(
            "run-{stamp}-{}.jsonl",
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        let run_id = path.file_stem().unwrap().to_string_lossy().into_owned();
        let stream = pool.open(path.clone())?;
        let path = stream.path().unwrap_or(&path).to_path_buf();
        let snapshot = Start {
            kind: "snapshot",
            schema: 1,
            run: run_id,
            endpoint,
            models,
            units,
        };
        let mut journal = Self {
            stream,

            chunks: 0,

            path: path.to_string_lossy().into_owned(),
            sequence: 0,
            unix_ms: String::new(),
            started: std::time::Instant::now(),
            #[cfg(test)]
            fail_after: None,
        };
        journal.write(&snapshot).await?;
        Ok(journal)
    }
    async fn write(&mut self, value: &impl Serialize) -> Result<(), Error> {
        crate::recording::write_json_async(&self.stream, &mut self.chunks, value).await
    }
    pub async fn record(&mut self, command: Option<&str>, fact: &SlaveFact) -> Result<(), Error> {
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
        })
        .await?;
        self.sequence = next;
        self.unix_ms = unix_ms;
        Ok(())
    }
}
