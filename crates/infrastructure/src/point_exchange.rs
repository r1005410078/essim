//! 点表编辑输入与文件格式适配；不修改权威目录，不执行公式。
use calamine::{Data, Reader, Xlsx};
use essim_core::domain::{
    error::Error,
    identity::{ModelId, PointId},
    model::{CollectionModel, PointDefinition},
    point_table::*,
};
use serde::{Deserialize, Serialize};
use std::{
    collections::BTreeMap,
    io::{Cursor, Read},
};

/// 完整写入相邻临时文件再替换目标；失败只清理本次实际创建的临时文件。
/// # Errors
/// 文件创建、写入、同步或替换失败时返回FILE_WRITE；不删除其他任务的文件。
pub fn write_export(path: &std::path::Path, bytes: &[u8]) -> Result<(), Error> {
    use std::io::Write;
    static NEXT: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
    let serial = NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let token = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_err(|e| Error::new("FILE_WRITE", e.to_string()))?
        .as_nanos();
    let temporary = path.with_extension(format!("pending-{}-{token}-{serial}", std::process::id()));
    let mut file = std::fs::OpenOptions::new()
        .create_new(true)
        .write(true)
        .open(&temporary)
        .map_err(|e| Error::new("FILE_WRITE", e.to_string()))?;
    let result = (|| -> std::io::Result<()> {
        file.write_all(bytes)?;
        file.sync_all()?;
        drop(file);
        std::fs::rename(&temporary, path)
    })();
    if let Err(error) = result {
        let _ = std::fs::remove_file(&temporary);
        return Err(Error::new("FILE_WRITE", error.to_string()));
    }
    Ok(())
}

/// 编辑与文件预览中的文本行，允许暂存尚未合法的单元格。
pub type InputRow = BTreeMap<String, String>;
/// 单次模型草稿校验输入。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DraftInput {
    /// 模型稳定身份。
    pub id: String,
    /// 草稿模型名称。
    pub name: String,
    /// 包含稳定点ID的全部分类行。
    pub points: Vec<InputRow>,
}
/// 实际编码结果，供前端显示而非自行用Number反算。
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PointPreview {
    /// 稳定点身份。
    pub point_id: String,
    /// 初始原始数值。
    pub raw: RawValue,
    /// 实际编码后的工程值。
    pub engineering: String,
    /// 对应寄存器或位映射值。
    pub registers: Vec<u16>,
}
/// 完整草稿的后端校验结果。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DraftPreview {
    /// 只有整表合法时才有规范模型。
    pub model: Option<CollectionModel>,
    /// 可定位的点字段错误。
    pub errors: Vec<Error>,
    /// 已成功转换的点预览。
    pub previews: Vec<PointPreview>,
}
/// 按文本列解析一行并校验编码。
/// # Errors
/// 字段解析或领域校验失败时返回稳定点身份和字段。
pub fn parse_row(row: &InputRow) -> Result<PointDefinition, Error> {
    let id = PointId(row.get("id").cloned().unwrap_or_default());
    let field = |key: &str, default: &str| {
        row.get(key)
            .filter(|v| !v.trim().is_empty())
            .cloned()
            .unwrap_or_else(|| default.into())
    };
    let invalid =
        |key: &str| Error::new("INVALID_FIELD", format!("{key}字段值非法")).at_point(&id, key);
    let enumeration = |key: &str, default: &str| serde_json::Value::String(field(key, default));
    let value_type = serde_json::from_value(enumeration("valueType", "u16"))
        .map_err(|_| invalid("valueType"))?;
    let scale = field("scale", "1");
    validate_scale(value_type, &scale).map_err(|e| e.at_point(&id, "scale"))?;
    let initial_engineering = field(
        "initialEngineering",
        if value_type == ValueType::Bool {
            "false"
        } else {
            "0"
        },
    );
    let initial = from_engineering(value_type, &initial_engineering, &scale)
        .map_err(|e| e.at_point(&id, "initialEngineering"))?;
    let point = PointDefinition {
        id: id.clone(),
        name: field("name", ""),
        category: serde_json::from_value(enumeration("category", "measurement"))
            .map_err(|_| invalid("category"))?,
        area: serde_json::from_value(enumeration("area", "holding"))
            .map_err(|_| invalid("area"))?,
        address: field("address", "")
            .parse()
            .map_err(|_| invalid("address"))?,
        value_type,
        scale,
        initial,
        initial_engineering,
        byte_order: serde_json::from_value(enumeration("byteOrder", "big"))
            .map_err(|_| invalid("byteOrder"))?,
        word_order: serde_json::from_value(enumeration("wordOrder", "big"))
            .map_err(|_| invalid("wordOrder"))?,
        unit: field("unit", ""),
        note: field("note", ""),
        slave_writable: match field("slaveWritable", "false").as_str() {
            "true" | "1" => true,
            "false" | "0" => false,
            _ => return Err(invalid("slaveWritable")),
        },
        master_period_ms: field("masterPeriodMs", "1000")
            .parse()
            .map_err(|_| invalid("masterPeriodMs"))?,
    };
    point.validate()?;
    Ok(point)
}
/// 校验全部分类；结果无副作用，可以反复预览非法草稿。
pub fn preview(input: DraftInput) -> DraftPreview {
    let mut result = DraftPreview {
        model: None,
        errors: Vec::new(),
        previews: Vec::new(),
    };
    if input.points.len() > MAX_ROWS {
        result
            .errors
            .push(Error::new("IMPORT_LIMIT", "点数超过100000"));
        return result;
    }
    let mut points = Vec::new();
    for row in input.points {
        match parse_row(&row) {
            Ok(point) => {
                match encode(
                    point.value_type,
                    &point.initial,
                    point.byte_order,
                    point.word_order,
                ) {
                    Ok(registers) => result.previews.push(PointPreview {
                        point_id: point.id.0.clone(),
                        raw: point.initial.clone(),
                        engineering: engineering_value(&point.initial, &point.scale)
                            .unwrap_or_default(),
                        registers,
                    }),
                    Err(error) => result
                        .errors
                        .push(error.at_point(&point.id, "initialEngineering")),
                }
                points.push(point);
            }
            Err(error) => result.errors.push(error),
        }
    }
    let model = CollectionModel {
        id: ModelId(input.id),
        name: input.name,
        points,
    };
    if result.errors.is_empty() {
        match model.validate() {
            Ok(()) => result.model = Some(model),
            Err(error) => result.errors.push(error),
        }
    }
    result
}
/// 文件单工作表预览；行列尚未映射。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InputSheet {
    /// 工作表名称。
    pub name: String,
    /// 原始列标题。
    pub headers: Vec<String>,
    /// 各行文本值，不经JS Number处理。
    pub rows: Vec<Vec<String>>,
}
/// 文件预览，可选择工作表和列映射。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InputFile {
    /// 文件名，不持有系统文件句柄。
    pub name: String,
    /// 用户可选的工作表。
    pub sheets: Vec<InputSheet>,
}
/// 输入文件字节上限，避免在IPC中无界加载。
pub const MAX_BYTES: usize = 16 * 1024 * 1024;
/// 点表行数保护上限，不代表大表性能承诺。
pub const MAX_ROWS: usize = 100_000;
/// 点表交换标准列；ID只是源元数据，导入后生成新身份。
pub const HEADERS: &[&str] = &[
    "id",
    "name",
    "category",
    "area",
    "address",
    "valueType",
    "byteOrder",
    "wordOrder",
    "scale",
    "unit",
    "initialEngineering",
    "slaveWritable",
    "masterPeriodMs",
    "note",
];
fn failure(message: impl Into<String>) -> Error {
    Error::new("FILE_FORMAT", message)
}
fn numeric_needs_text(value: f64) -> bool {
    if !value.is_finite() || value.abs() >= 1_000_000_000_000_000. {
        return true;
    }
    let text = value.abs().to_string();
    let significand = text.split(['e', 'E']).next().unwrap_or("");
    let digits: String = significand.chars().filter(char::is_ascii_digit).collect();
    digits.trim_start_matches('0').len() > 15
}
/// 将规范点转换为文本行，初始值以工程输入保存。
pub fn point_row(point: &PointDefinition) -> InputRow {
    let value = serde_json::to_value(point).unwrap_or_default();
    HEADERS
        .iter()
        .map(|key| {
            (
                key.to_string(),
                match &value[*key] {
                    serde_json::Value::String(v) => v.clone(),
                    v => v.to_string(),
                },
            )
        })
        .collect()
}
/// 解析JSON、CSV或XLSX，拒绝超限、未知版本、公式和不精确大整数。
/// # Errors
/// 文件格式不合法或无法保真时失败；不改变任何草稿或目录。
pub fn read_file(name: &str, bytes: &[u8]) -> Result<InputFile, Error> {
    read_file_cancellable(name, bytes, || false)
}
/// 可取消的后台解析；在行、工作表和ZIP条目边界检查取消，不交付部分结果。
/// # Errors
/// 除格式／限额错误外，取消返回IMPORT_CANCELLED。
pub fn read_file_cancellable(
    name: &str,
    bytes: &[u8],
    cancelled: impl Fn() -> bool,
) -> Result<InputFile, Error> {
    let check = || {
        if cancelled() {
            Err(Error::new("IMPORT_CANCELLED", "导入已取消"))
        } else {
            Ok(())
        }
    };
    check()?;
    if bytes.len() > MAX_BYTES {
        return Err(Error::new("IMPORT_LIMIT", "文件超过16MiB"));
    }
    let extension = name.rsplit('.').next().unwrap_or("").to_ascii_lowercase();
    let sheets = match extension.as_str() {
        "json" => {
            let value: serde_json::Value =
                serde_json::from_slice(bytes).map_err(|e| failure(e.to_string()))?;
            if value["formatVersion"] != 1 {
                return Err(failure("未知JSON格式版本"));
            }
            let model: CollectionModel = serde_json::from_value(value["model"].clone())
                .map_err(|e| failure(e.to_string()))?;
            if model.points.len() > MAX_ROWS {
                return Err(Error::new("IMPORT_LIMIT", "超过100000行"));
            }
            model.validate()?;
            let rows = model
                .points
                .iter()
                .map(|p| {
                    let row = point_row(p);
                    HEADERS.iter().map(|k| row[*k].clone()).collect()
                })
                .collect();
            vec![InputSheet {
                name: model.name,
                headers: HEADERS.iter().map(|v| v.to_string()).collect(),
                rows,
            }]
        }
        "csv" => {
            let bytes = bytes.strip_prefix(&[0xef, 0xbb, 0xbf]).unwrap_or(bytes);
            let mut reader = csv::ReaderBuilder::new().from_reader(bytes);
            let mut headers: Vec<String> = reader
                .headers()
                .map_err(|e| failure(e.to_string()))?
                .iter()
                .map(str::to_string)
                .collect();
            let escaped = headers.iter().position(|h| h == "__essimTextEncoding");
            if let Some(index) = escaped {
                headers.remove(index);
            }
            let mut rows = Vec::new();
            for row in reader.records() {
                check()?;
                if rows.len() >= MAX_ROWS {
                    return Err(Error::new("IMPORT_LIMIT", "超过100000行"));
                }
                let mut cells: Vec<String> = row
                    .map_err(|e| failure(e.to_string()))?
                    .iter()
                    .map(str::to_string)
                    .collect();
                if let Some(index) = escaped {
                    if cells.get(index).map(String::as_str) != Some("apostrophe-v1") {
                        return Err(failure("未知CSV文本编码"));
                    }
                    cells.remove(index);
                    for value in &mut cells {
                        if let Some(text) = value.strip_prefix('\'') {
                            *value = text.into();
                        }
                    }
                }
                rows.push(cells);
            }
            vec![InputSheet {
                name: "点表".into(),
                headers,
                rows,
            }]
        }
        "xlsx" => {
            // 解压前检查中央目录声明大小，防止压缩包远大于输入字节上限。
            let mut archive =
                zip::ZipArchive::new(Cursor::new(bytes)).map_err(|e| failure(e.to_string()))?;
            let mut expanded = 0_u64;
            let mut buffer = [0_u8; 16384];
            for i in 0..archive.len() {
                check()?;
                let mut entry = archive.by_index(i).map_err(|e| failure(e.to_string()))?;
                let remaining = 64 * 1024 * 1024 - expanded;
                if entry.size() > remaining {
                    return Err(Error::new("IMPORT_LIMIT", "XLSX解压大小超过64MiB"));
                }
                // 同时校验实际解压流，不只信任中央目录的声明大小。
                let mut bounded = (&mut entry).take(remaining + 1);
                loop {
                    check()?;
                    let count = bounded
                        .read(&mut buffer)
                        .map_err(|e| failure(e.to_string()))?;
                    if count == 0 {
                        break;
                    }
                    expanded += count as u64;
                    if expanded > 64 * 1024 * 1024 {
                        return Err(Error::new("IMPORT_LIMIT", "XLSX解压大小超过64MiB"));
                    }
                }
            }
            let mut workbook: Xlsx<_> =
                Xlsx::new(Cursor::new(bytes)).map_err(|e| failure(e.to_string()))?;
            let mut sheets = Vec::new();
            let mut total = 0;
            for name in workbook.sheet_names() {
                check()?;
                // 在分配稠密Range之前流式检查坐标，稀疏远端单元格不能触发巨大分配。
                let mut reader = workbook
                    .worksheet_cells_reader(&name)
                    .map_err(|e| failure(e.to_string()))?;
                let mut minimum = (u32::MAX, u32::MAX);
                let mut maximum = (0_u32, 0_u32);
                while let Some(cell) = reader
                    .next_cell_with_formula_metadata()
                    .map_err(|e| failure(e.to_string()))?
                {
                    check()?;
                    if cell.pos.0 > MAX_ROWS as u32 || cell.pos.1 >= 100 {
                        return Err(Error::new("IMPORT_LIMIT", "XLSX坐标超过100001行或100列"));
                    }
                    minimum.0 = minimum.0.min(cell.pos.0);
                    minimum.1 = minimum.1.min(cell.pos.1);
                    maximum.0 = maximum.0.max(cell.pos.0);
                    maximum.1 = maximum.1.max(cell.pos.1);
                    if (u64::from(maximum.0 - minimum.0) + 1)
                        * (u64::from(maximum.1 - minimum.1) + 1)
                        > 2_000_000
                    {
                        return Err(Error::new("IMPORT_LIMIT", "XLSX矩形范围超过2000000格"));
                    }
                    if cell.formula.is_some() {
                        return Err(failure(format!(
                            "{name}，行{}列{}为公式，请先转为明确值",
                            cell.pos.0 + 1,
                            cell.pos.1 + 1
                        )));
                    }
                }
                drop(reader);
                let range = workbook
                    .worksheet_range(&name)
                    .map_err(|e| failure(e.to_string()))?;
                if range.width() > 100 {
                    return Err(Error::new("IMPORT_LIMIT", "工作表超过100列"));
                }
                total += range.height().saturating_sub(1);
                if total > MAX_ROWS {
                    return Err(Error::new("IMPORT_LIMIT", "工作表合计超过100000行"));
                }
                let mut rows = Vec::new();
                let origin = range.start().unwrap_or((0, 0));
                for (r, row) in range.rows().enumerate() {
                    check()?;
                    let mut output = Vec::new();
                    for (c, cell) in row.iter().enumerate() {
                        match cell {
                            Data::Float(v) if numeric_needs_text(*v) => {
                                return Err(failure(format!(
                                    "{name}，行{}列{}数值可能已丢失精度，请使用文本单元格",
                                    r + origin.0 as usize + 1,
                                    c + origin.1 as usize + 1
                                )))
                            }
                            Data::Int(v) if v.unsigned_abs() >= 1_000_000_000_000_000 => {
                                return Err(failure(format!(
                                    "{name}，行{}列{}须以文本表示大整数",
                                    r + origin.0 as usize + 1,
                                    c + origin.1 as usize + 1
                                )))
                            }
                            Data::Error(v) => {
                                return Err(failure(format!(
                                    "{name}，行{}列{}错误：{v:?}",
                                    r + origin.0 as usize + 1,
                                    c + origin.1 as usize + 1
                                )))
                            }
                            _ => output.push(cell.to_string()),
                        }
                    }
                    rows.push(output);
                }
                let headers = if rows.is_empty() {
                    Vec::new()
                } else {
                    rows.remove(0)
                };
                sheets.push(InputSheet {
                    name,
                    headers,
                    rows,
                });
            }
            sheets
        }
        _ => return Err(failure("仅支持JSON、CSV、XLSX")),
    };
    check()?;
    Ok(InputFile {
        name: name.into(),
        sheets,
    })
}
/// 导出一个合法保存模型；文本单元格不执行公式。
/// # Errors
/// 模型非法、格式未知或格式写出失败时返回错误。
pub fn export(model: &CollectionModel, format: &str) -> Result<Vec<u8>, Error> {
    model.validate()?;
    let rows: Vec<_> = model.points.iter().map(point_row).collect();
    match format {
        "json" => serde_json::to_vec_pretty(&serde_json::json!({"formatVersion":1,"model":model}))
            .map_err(|e| failure(e.to_string())),
        "csv" => {
            let mut writer = csv::Writer::from_writer(Vec::new());
            writer
                .write_record(HEADERS.iter().copied().chain(["__essimTextEncoding"]))
                .map_err(|e| failure(e.to_string()))?;
            for row in rows {
                // CSV没有单元格类型。危险开头及原有单引号统一转义，并声明可逆编码；
                // Excel直接打开时不会把设备名称、备注或负数当成公式执行。
                let values = HEADERS.iter().map(|key| {
                    let value = &row[*key];
                    if value.starts_with(['\'', '=', '+', '-', '@', '\t', '\r', '\n']) {
                        format!("'{}", value)
                    } else {
                        value.clone()
                    }
                });
                writer
                    .write_record(values.chain(["apostrophe-v1".into()]))
                    .map_err(|e| failure(e.to_string()))?;
            }
            writer.into_inner().map_err(|e| failure(e.to_string()))
        }
        "xlsx" => {
            let mut workbook = rust_xlsxwriter::Workbook::new();
            let sheet = workbook.add_worksheet();
            for (column, header) in HEADERS.iter().enumerate() {
                sheet
                    .write_string(0, column as u16, *header)
                    .map_err(|e| failure(e.to_string()))?;
            }
            for (index, row) in rows.iter().enumerate() {
                for (column, key) in HEADERS.iter().enumerate() {
                    sheet
                        .write_string(index as u32 + 1, column as u16, &row[*key])
                        .map_err(|e| failure(e.to_string()))?;
                }
            }
            workbook
                .save_to_buffer()
                .map_err(|e| failure(e.to_string()))
        }
        _ => Err(failure("未知导出格式")),
    }
}
