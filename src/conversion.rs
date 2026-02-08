//! Type conversion between Python objects and Arrow RecordBatches.
//!
//! Conversion priority (input):
//!   1. Arrow PyCapsule interface (__arrow_c_stream__ / __arrow_c_array__)
//!   2. PyArrow RecordBatch / Table
//!   3. Pandas DataFrame (via pyarrow bridge)
//!   4. Polars DataFrame (via .to_arrow())
//!   5. Dict of lists (columnar)
//!   6. List of dicts (row-oriented)
//!   7. Single dict (one row)

use std::sync::Arc;

use arrow_array::RecordBatch;
use arrow_schema::{Field, Schema, SchemaRef};
use pyo3::exceptions::PyTypeError;
use pyo3::prelude::*;
use pyo3::types::{PyDict, PyList};
use pyo3_arrow::{PyRecordBatch, PySchema, PyTable};

use crate::error::IngestionError;

// ---------------------------------------------------------------------------
// Python → Arrow (input conversion)
// ---------------------------------------------------------------------------

/// Extract Arrow `RecordBatch`es from a Python object using the best available path.
pub fn python_to_batches(
    py: Python<'_>,
    data: &Bound<'_, PyAny>,
    _schema: Option<&Schema>,
) -> PyResult<Vec<RecordBatch>> {
    // 1. Arrow PyCapsule interface (PyTable handles __arrow_c_stream__)
    if let Ok(table) = data.extract::<PyTable>() {
        let (batches, _schema) = table.into_inner();
        return Ok(batches);
    }

    // 2. Try as single RecordBatch (__arrow_c_array__)
    if let Ok(batch) = data.extract::<PyRecordBatch>() {
        return Ok(vec![batch.into_inner()]);
    }

    // 3. Pandas DataFrame
    if let Ok(batches) = try_pandas(py, data) {
        return Ok(batches);
    }

    // 4. Polars DataFrame
    if let Ok(batches) = try_polars(py, data) {
        return Ok(batches);
    }

    // 5–7. Python dicts / lists
    if let Ok(batches) = try_python_dicts(py, data) {
        return Ok(batches);
    }

    Err(PyTypeError::new_err(
        "Unsupported data type. Expected: Arrow-compatible object, PyArrow RecordBatch/Table, \
         Pandas DataFrame, Polars DataFrame, dict, list[dict], or dict of lists.",
    ))
}

/// Try converting a Pandas DataFrame via PyArrow.
fn try_pandas(py: Python<'_>, data: &Bound<'_, PyAny>) -> PyResult<Vec<RecordBatch>> {
    let pd = py
        .import("pandas")
        .map_err(|_| PyTypeError::new_err("not pandas"))?;
    let df_type = pd.getattr("DataFrame")?;

    if !data.is_instance(&df_type)? {
        return Err(PyTypeError::new_err("not a Pandas DataFrame"));
    }

    let pa = py.import("pyarrow")?;
    let table = pa.getattr("Table")?.call_method1("from_pandas", (data,))?;
    let py_table = table.extract::<PyTable>()?;
    let (batches, _schema) = py_table.into_inner();
    Ok(batches)
}

/// Try converting a Polars DataFrame via `.to_arrow()`.
fn try_polars(py: Python<'_>, data: &Bound<'_, PyAny>) -> PyResult<Vec<RecordBatch>> {
    let pl = py
        .import("polars")
        .map_err(|_| PyTypeError::new_err("not polars"))?;
    let df_type = pl.getattr("DataFrame")?;

    if !data.is_instance(&df_type)? {
        return Err(PyTypeError::new_err("not a Polars DataFrame"));
    }

    let arrow_table = data.call_method0("to_arrow")?;
    let py_table = arrow_table.extract::<PyTable>()?;
    let (batches, _schema) = py_table.into_inner();
    Ok(batches)
}

/// Try converting Python dicts/lists to Arrow RecordBatches.
fn try_python_dicts(py: Python<'_>, data: &Bound<'_, PyAny>) -> PyResult<Vec<RecordBatch>> {
    // Single dict — could be columnar or single row
    if let Ok(dict) = data.downcast::<PyDict>() {
        return dict_to_batches(py, dict);
    }

    // List of dicts
    if let Ok(list) = data.downcast::<PyList>() {
        if list.is_empty() {
            return Err(PyTypeError::new_err("empty list"));
        }
        return list_of_dicts_to_batches(py, list);
    }

    Err(PyTypeError::new_err("not a dict or list"))
}

/// Convert a Python dict to RecordBatch.
///
/// If values are lists → columnar format.
/// If values are scalars → single row.
fn dict_to_batches(py: Python<'_>, dict: &Bound<'_, PyDict>) -> PyResult<Vec<RecordBatch>> {
    let first_value = dict
        .values()
        .get_item(0)
        .map_err(|_| PyTypeError::new_err("empty dict"))?;
    let is_columnar = first_value.downcast::<PyList>().is_ok();

    if is_columnar {
        columnar_dict_to_batch(py, dict)
    } else {
        let list = PyList::new(py, [dict.as_any()])?;
        list_of_dicts_to_batches(py, &list)
    }
}

/// Convert a columnar dict (str → list) to RecordBatch via pyarrow.
fn columnar_dict_to_batch(py: Python<'_>, dict: &Bound<'_, PyDict>) -> PyResult<Vec<RecordBatch>> {
    let pa = py.import("pyarrow")?;
    let table = pa.call_method1("table", (dict,))?;
    let py_table = table.extract::<PyTable>()?;
    let (batches, _schema) = py_table.into_inner();
    Ok(batches)
}

/// Convert a list of dicts to RecordBatch via JSON parsing.
fn list_of_dicts_to_batches(
    py: Python<'_>,
    list: &Bound<'_, PyList>,
) -> PyResult<Vec<RecordBatch>> {
    let json_mod = py.import("json")?;
    let json_str: String = json_mod.call_method1("dumps", (list,))?.extract()?;
    json_str_to_batches(&json_str)
}

// ---------------------------------------------------------------------------
// JSON / CSV string → Arrow
// ---------------------------------------------------------------------------

/// Parse a JSON string into Arrow RecordBatches.
pub fn json_str_to_batches(json: &str) -> PyResult<Vec<RecordBatch>> {
    // Infer schema from the JSON data first
    let cursor = std::io::Cursor::new(json.as_bytes());
    let (inferred, _) = arrow_json::reader::infer_json_schema(cursor, None)
        .map_err(|e| IngestionError::new_err(format!("JSON schema inference error: {e}")))?;
    let schema = Arc::new(inferred);

    let cursor = std::io::Cursor::new(json.as_bytes());
    let reader = arrow_json::ReaderBuilder::new(schema)
        .build(cursor)
        .map_err(|e| IngestionError::new_err(format!("JSON parse error: {e}")))?;

    let mut batches = Vec::new();
    for batch in reader {
        batches.push(batch.map_err(|e| IngestionError::new_err(format!("JSON read error: {e}")))?);
    }
    Ok(batches)
}

/// Parse a CSV string into Arrow RecordBatches.
pub fn csv_str_to_batches(csv: &str) -> PyResult<Vec<RecordBatch>> {
    // Infer schema from CSV header + data
    let cursor = std::io::Cursor::new(csv.as_bytes());
    let (schema, _) = arrow_csv::reader::Format::default()
        .with_header(true)
        .infer_schema(cursor, None)
        .map_err(|e| IngestionError::new_err(format!("CSV schema inference error: {e}")))?;
    let schema = Arc::new(schema);

    let cursor = std::io::Cursor::new(csv.as_bytes());
    let reader = arrow_csv::ReaderBuilder::new(schema)
        .with_header(true)
        .build(cursor)
        .map_err(|e| IngestionError::new_err(format!("CSV parse error: {e}")))?;

    let mut batches = Vec::new();
    for batch in reader {
        batches.push(batch.map_err(|e| IngestionError::new_err(format!("CSV read error: {e}")))?);
    }
    Ok(batches)
}

// ---------------------------------------------------------------------------
// Arrow → Python (output conversion)
// ---------------------------------------------------------------------------

/// Convert RecordBatches to a PyArrow Table.
pub fn batches_to_pyarrow<'py>(
    py: Python<'py>,
    batches: &[RecordBatch],
    schema: &SchemaRef,
) -> PyResult<Bound<'py, PyAny>> {
    let py_table = PyTable::try_new(batches.to_vec(), schema.clone())?;
    let result: Bound<'py, PyAny> = py_table.into_pyarrow(py)?;
    Ok(result)
}

/// Convert RecordBatches to a Pandas DataFrame.
pub fn batches_to_pandas<'py>(
    py: Python<'py>,
    batches: &[RecordBatch],
    schema: &SchemaRef,
) -> PyResult<Bound<'py, PyAny>> {
    let table = batches_to_pyarrow(py, batches, schema)?;
    table.call_method0("to_pandas")
}

/// Convert RecordBatches to a Polars DataFrame.
pub fn batches_to_polars<'py>(
    py: Python<'py>,
    batches: &[RecordBatch],
    schema: &SchemaRef,
) -> PyResult<Bound<'py, PyAny>> {
    let pl = py.import("polars")?;
    let table = batches_to_pyarrow(py, batches, schema)?;
    pl.call_method1("from_arrow", (table,))
}

/// Convert RecordBatches to a Python dict (columnar via PyArrow).
pub fn batches_to_dicts<'py>(
    py: Python<'py>,
    batches: &[RecordBatch],
    schema: &SchemaRef,
) -> PyResult<Bound<'py, PyAny>> {
    let table = batches_to_pyarrow(py, batches, schema)?;
    table.call_method0("to_pydict")
}

/// Auto-detect the best available output library.
pub fn batches_to_best_df<'py>(
    py: Python<'py>,
    batches: &[RecordBatch],
    schema: &SchemaRef,
) -> PyResult<Bound<'py, PyAny>> {
    if py.import("polars").is_ok() {
        return batches_to_polars(py, batches, schema);
    }
    if py.import("pandas").is_ok() {
        return batches_to_pandas(py, batches, schema);
    }
    batches_to_pyarrow(py, batches, schema)
}

// ---------------------------------------------------------------------------
// Schema conversion
// ---------------------------------------------------------------------------

/// Convert a Python schema specification to an Arrow Schema.
///
/// Accepts: pyarrow.Schema or dict[str, str].
pub fn python_to_schema(_py: Python<'_>, obj: &Bound<'_, PyAny>) -> PyResult<Schema> {
    // Try PyArrow Schema via PyCapsule
    if let Ok(py_schema) = obj.extract::<PySchema>() {
        let schema_ref: SchemaRef = py_schema.into();
        return Ok(schema_ref.as_ref().clone());
    }

    // Dict mapping: {"col_name": "type_string"}
    if let Ok(dict) = obj.downcast::<PyDict>() {
        let mut fields = Vec::new();
        for (key, value) in dict.iter() {
            let name: String = key.extract()?;
            let type_str: String = value.extract()?;
            let data_type = parse_type_string(&type_str)?;
            fields.push(Field::new(&name, data_type, true));
        }
        return Ok(Schema::new(fields));
    }

    Err(PyTypeError::new_err(
        "Schema must be a pyarrow.Schema or a dict mapping column names to type strings",
    ))
}

/// Parse a simple type string to an Arrow DataType.
fn parse_type_string(s: &str) -> PyResult<arrow_schema::DataType> {
    use arrow_schema::DataType;
    match s.to_lowercase().as_str() {
        "int8" | "i8" => Ok(DataType::Int8),
        "int16" | "i16" => Ok(DataType::Int16),
        "int32" | "i32" | "int" => Ok(DataType::Int32),
        "int64" | "i64" => Ok(DataType::Int64),
        "uint8" | "u8" => Ok(DataType::UInt8),
        "uint16" | "u16" => Ok(DataType::UInt16),
        "uint32" | "u32" => Ok(DataType::UInt32),
        "uint64" | "u64" => Ok(DataType::UInt64),
        "float16" | "f16" => Ok(DataType::Float16),
        "float32" | "f32" | "float" => Ok(DataType::Float32),
        "float64" | "f64" | "double" => Ok(DataType::Float64),
        "bool" | "boolean" => Ok(DataType::Boolean),
        "string" | "str" | "utf8" => Ok(DataType::Utf8),
        "large_string" | "large_utf8" => Ok(DataType::LargeUtf8),
        "binary" | "bytes" => Ok(DataType::Binary),
        "date32" | "date" => Ok(DataType::Date32),
        "timestamp" | "datetime" => Ok(DataType::Timestamp(
            arrow_schema::TimeUnit::Microsecond,
            None,
        )),
        _ => Err(crate::error::SchemaError::new_err(format!(
            "Unknown type: {s}"
        ))),
    }
}
