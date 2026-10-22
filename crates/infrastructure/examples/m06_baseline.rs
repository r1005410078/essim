//! M06分开测量目录复制、序列化及同步落盘；不是持续规模验收。
fn main() -> Result<(), Box<dyn std::error::Error>> {
    use essim_core::application::catalog::Catalog;
    use essim_infrastructure::tcp_slave::Snapshot;
    use std::{io::Write, time::Instant};
    let mut args = std::env::args().skip(1);
    let snapshots: Vec<Snapshot> =
        serde_json::from_slice(&std::fs::read(args.next().ok_or("需要快照JSON")?)?)?;
    let path = args.next().ok_or("需要临时同步测量文件")?;
    let mut catalog = Catalog::default();
    for snapshot in &snapshots {
        catalog.endpoints.push(snapshot.endpoint.clone());
        catalog.applied.push(snapshot.endpoint.clone());
        for (id, value) in &snapshot.values {
            if let Some(model) = &value.model {
                if !catalog.models.iter().any(|m| m.id == model.id) {
                    catalog.models.push(model.as_ref().clone());
                }
            }
            catalog.values.insert(id.clone(), value.clone());
        }
    }
    let start = Instant::now();
    for _ in 0..10 {
        std::hint::black_box(catalog.clone());
    }
    let clone_ms = start.elapsed().as_secs_f64() * 100.;
    let start = Instant::now();
    let json = serde_json::to_vec(&catalog)?;
    let serialize_ms = start.elapsed().as_secs_f64() * 1000.;
    let start = Instant::now();
    let layout = essim_core::domain::slave::ModelLayout::new(catalog.models[0].clone())
        .map_err(|e| e.message)?;
    let layout_ms = start.elapsed().as_secs_f64() * 1000.;
    let start = Instant::now();
    for _ in 0..100 {
        std::hint::black_box(catalog.values.values().next().ok_or("需要Unit值")?.clone());
    }
    let values_ms = start.elapsed().as_secs_f64() * 10.;
    let start = Instant::now();
    for _ in 0..100 {
        std::hint::black_box(catalog.models[0].clone());
    }
    let model_clone_ms = start.elapsed().as_secs_f64() * 10.;
    let unit = catalog.values.values().next().ok_or("需要Unit值")?;
    let start = Instant::now();
    for _ in 0..100 {
        std::hint::black_box(unit.raw.clone());
    }
    let raw_clone_ms = start.elapsed().as_secs_f64() * 10.;
    let ranges = layout
        .addresses
        .iter()
        .map(|a| match (a.first_key_value(), a.last_key_value()) {
            (Some((&start, _)), Some((&end, _))) => {
                libmodbus_rs::AddressRange::new(start, u32::from(end) - u32::from(start) + 1)
                    .map(Some)
            }
            _ => Ok(None),
        })
        .collect::<Result<Vec<_>, _>>()?;
    let start = Instant::now();
    for _ in 0..100 {
        std::hint::black_box(libmodbus_rs::Mapping::new(
            ranges[0], ranges[1], ranges[2], ranges[3],
        )?);
    }
    let mapping_ms = start.elapsed().as_secs_f64() * 10.;
    let start = Instant::now();
    let mut file = std::fs::File::create(&path)?;
    for _ in 0..100 {
        file.write_all(&[b'x'; 1024])?;
        file.sync_data()?;
    }
    let sync_ms = start.elapsed().as_secs_f64() * 10.;
    drop(file);
    std::fs::remove_file(path)?;
    println!(
        "{}",
        serde_json::json!({"endpoints":catalog.endpoints.len(),"units":catalog.values.len(),"pointsPerModel":layout.model.points.len(),"catalogSerializedBytes":json.len(),"catalogCloneMeanMs":clone_ms,"catalogSerializeMs":serialize_ms,"singleModelLayoutMs":layout_ms,"singleUnitValuesCloneMeanMs":values_ms,"singleModelCloneMeanMs":model_clone_ms,"singleUnitRawValuesCloneMeanMs":raw_clone_ms,"singleCMappingAllocateDropMeanMs":mapping_ms,"mappingIterations":100,"mappingScope":"four contiguous area spans; allocation/drop only, excludes initial-value writes","sync1024BytesMeanMs":sync_ms,"cloneIterations":10,"syncIterations":100,"build":"debug","scope":"Catalog with current duplicated applied UnitValues; isolated costs, not process peak or sustained throughput"})
    );
    Ok(())
}
