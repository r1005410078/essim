//! 使用独立文件内容验证格式边界和精确值。
use essim_infrastructure::point_exchange::*;

fn row() -> InputRow {
    [
        ("id", "point-a"),
        ("name", "带逗号,换行\n点"),
        ("category", "measurement"),
        ("area", "holding"),
        ("address", "100"),
        ("valueType", "u64"),
        ("scale", "1"),
        ("initialEngineering", "18446744073709551615"),
        ("note", "测试备注"),
    ]
    .into_iter()
    .map(|(k, v)| (k.into(), v.into()))
    .collect()
}
#[test]
fn three_formats_keep_every_field_and_exact_u64_text() {
    let model = preview(DraftInput {
        id: "m".into(),
        name: "M".into(),

        points: vec![row()],
    })
    .model
    .unwrap();
    for format in ["json", "csv", "xlsx"] {
        let bytes = export(&model, format).unwrap();
        let file = read_file(&format!("table.{format}"), &bytes).unwrap();
        let sheet = &file.sheets[0];
        let columns: InputRow = sheet
            .headers
            .iter()
            .cloned()
            .zip(sheet.rows[0].clone())
            .collect();
        assert_eq!(parse_row(&columns).unwrap(), model.points[0]);
        assert_eq!(columns["initialEngineering"], "18446744073709551615");
    }
}
#[test]
fn independent_csv_fixture_supports_bom_and_quotes() {
    let csv = "\u{feff}name,address,area,valueType,category,initialEngineering\r\n\"测点,一\",10,input,u16,measurement,12\r\n";
    let file = read_file("test.csv", csv.as_bytes()).unwrap();
    assert_eq!(file.sheets[0].rows[0][0], "测点,一");
    assert!(read_file("bad.json", br#"{"formatVersion":99}"#).is_err());
    assert!(read_file("bad.xlsx", b"not a zip").is_err());
}
#[test]
fn worksheet_formulas_are_rejected_without_using_cached_values() {
    let mut book = rust_xlsxwriter::Workbook::new();
    book.add_worksheet().write_formula(1, 2, "=1+1").unwrap();
    let error = read_file("formula.xlsx", &book.save_to_buffer().unwrap()).unwrap_err();
    assert!(error.message.contains("行2列3"));
}
#[test]
fn invalid_draft_is_kept_outside_the_authoritative_model() {
    let mut bad = row();
    bad.insert("address".into(), "99999".into());
    let result = preview(DraftInput {
        id: "m".into(),
        name: "M".into(),

        points: vec![bad],
    });
    assert!(result.model.is_none());
    assert_eq!(result.errors[0].point_id.as_deref(), Some("point-a"));
    assert_eq!(result.errors[0].field.as_deref(), Some("address"));
}
#[test]
fn formula_like_csv_text_is_escaped_reversibly_and_xlsx_is_literal() {
    let mut row = row();
    row.insert("name".into(), "=HYPERLINK(\"http://test.invalid\")".into());
    row.insert("note".into(), "'already quoted".into());
    let model = preview(DraftInput {
        id: "m".into(),
        name: "M".into(),

        points: vec![row],
    })
    .model
    .unwrap();
    let bytes = export(&model, "csv").unwrap();
    let mut csv = csv::Reader::from_reader(bytes.as_slice());
    let headers = csv.headers().unwrap().clone();
    let record = csv.records().next().unwrap().unwrap();
    let name_index = headers.iter().position(|v| v == "name").unwrap();
    assert!(record[name_index].starts_with("'="));
    let imported = read_file("m.csv", &bytes).unwrap();
    let row: InputRow = imported.sheets[0]
        .headers
        .iter()
        .cloned()
        .zip(imported.sheets[0].rows[0].clone())
        .collect();
    assert_eq!(parse_row(&row).unwrap(), model.points[0]);
    assert!(read_file("m.xlsx", &export(&model, "xlsx").unwrap()).is_ok());
    assert!(read_file("too-big.csv", &vec![0; MAX_BYTES + 1]).is_err());
}
#[test]
fn cancellation_discards_the_complete_result() {
    use std::cell::Cell;
    let count = Cell::new(0);
    let csv = "name,address\na,1\nb,2\nc,3\nd,4\n";
    let error = read_file_cancellable("m.csv", csv.as_bytes(), || {
        count.set(count.get() + 1);
        count.get() >= 3
    })
    .unwrap_err();
    assert_eq!(error.code, "IMPORT_CANCELLED");
}
#[test]
fn independently_authored_openxml_fixture_preserves_text_integer() {
    let file = read_file(
        "independent.xlsx",
        include_bytes!("fixtures/point-table/independent-text.xlsx"),
    )
    .unwrap();
    assert_eq!(file.sheets[0].name, "独立点表");
    let mut row: InputRow = file.sheets[0]
        .headers
        .iter()
        .cloned()
        .zip(file.sheets[0].rows[0].clone())
        .collect();
    row.insert("id".into(), "new-id".into());
    let point = parse_row(&row).unwrap();
    assert_eq!(
        point.initial,
        essim_core::domain::foundation::RawValue::Unsigned(u64::MAX)
    );
}
#[test]
fn export_failure_does_not_remove_target_or_leave_partial_files() {
    let token = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let directory =
        std::env::temp_dir().join(format!("essim-m02-write-{}-{token}", std::process::id()));
    std::fs::create_dir(&directory).unwrap();
    let existing = directory.join("existing");
    std::fs::create_dir(&existing).unwrap();
    std::fs::write(existing.join("keep"), b"original").unwrap();
    assert!(write_export(&existing, b"new").is_err());
    assert_eq!(std::fs::read(existing.join("keep")).unwrap(), b"original");
    assert_eq!(std::fs::read_dir(&directory).unwrap().count(), 1);
    let target = directory.join("points.csv");
    std::fs::write(&target, b"old").unwrap();
    write_export(&target, b"complete").unwrap();
    assert_eq!(std::fs::read(&target).unwrap(), b"complete");
    std::fs::remove_dir_all(&directory).unwrap();
}
#[test]
fn sparse_far_cells_and_row_overflow_are_rejected_before_dense_allocation() {
    let mut book = rust_xlsxwriter::Workbook::new();
    let sheet = book.add_worksheet();
    sheet.write_string(0, 0, "name").unwrap();
    sheet.write_string(1_000_000, 99, "far").unwrap();
    assert_eq!(
        read_file("sparse.xlsx", &book.save_to_buffer().unwrap())
            .unwrap_err()
            .code,
        "IMPORT_LIMIT"
    );
    let mut csv = String::from("name,address\n");
    for _ in 0..=MAX_ROWS {
        csv.push_str("a,1\n");
    }
    assert_eq!(
        read_file("rows.csv", csv.as_bytes()).unwrap_err().code,
        "IMPORT_LIMIT"
    );
}
#[test]
fn numeric_excel_precision_errors_keep_absolute_source_coordinates() {
    for value in [
        9_007_199_254_740_992.,
        1_234_567_890_123_456.,
        0.12345678901234567,
    ] {
        let mut book = rust_xlsxwriter::Workbook::new();
        book.add_worksheet().write_number(1, 2, value).unwrap();
        assert!(read_file("precision.xlsx", &book.save_to_buffer().unwrap())
            .unwrap_err()
            .message
            .contains("行2列3"));
    }
}
