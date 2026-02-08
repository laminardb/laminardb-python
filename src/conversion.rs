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
//!
//! Conversion priority (output):
//!   Arrow PyCapsule → PyArrow → Pandas → Polars → dicts

use std::sync::Arc;

use arrow::ffi_stream::ArrowArrayStreamReader;
use arrow_array::RecordBatch;
use arrow_schema::{Field, Schema};
use pyo3::exceptions::PyTypeError;
use pyo3::prelude::*;
use pyo3::types::{PyDict, PyList, PyString};

use crate::error::IngestionError;

// ---------------------------------------------------------------------------
// Python → Arrow (input conversion)
// ---------------------------------------------------------------------------

/// Extract Arrow `RecordBatch`es from a Python object using the best available path.
pub fn python_to_batches(py: Python<'_>, data: &Bound<'_, PyAny>, schema: Option<&Schema>) -> PyResult<Vec<RecordBatch>> {
    // 1. Arrow PyCapsule interface
    if let Ok(batches) = try_pycapsule_stream(py, data) {
        return Ok(batches);
    }

    // 2. PyArrow RecordBatch / Table
    if let Ok(batches) = try_pyarrow(py, data) {
        return Ok(batches);
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
    if let Ok(batches) = try_python_dicts(py, data, schema) {
        return Ok(batches);
    }

    Err(PyTypeError::new_err(
        "Unsupported data type. Expected: Arrow-compatible object, PyArrow RecordBatch/Table, \
         Pandas DataFrame, Polars DataFrame, dict, list[dict], or dict of lists.",
    ))
}

/// Try the Arrow PyCapsule C Stream interface.
fn try_pycapsule_stream(_py: Python<'_>, data: &Bound<'_, PyAny>) -> PyResult<Vec<RecordBatch>> {
    if !data.hasattr("__arrow_c_stream__")? {
        return Err(PyTypeError::new_err("no __arrow_c_stream__"));
    }

    let capsule = data.call_method0("__arrow_c_stream__")?;
    let stream_ptr = capsule.extract::<pyo3_arrow::PyArrowConvert>()?;
    let reader: ArrowArrayStreamReader = stream_ptr.into();

    let mut batches = Vec::new();
    for batch in reader {
        batches.push(batch.map_err(|e| IngestionError::new_err(format!("Arrow stream error: {e}")))?);
    }
    Ok(batches)
}

/// Try converting a PyArrow RecordBatch or Table.
fn try_pyarrow(py: Python<'_>, data: &Bound<'_, PyAny>) -> PyResult<Vec<RecordBatch>> {
    let pa = py.import("pyarrow")?;
    let table_type = pa.getattr("Table")?;
    let batch_type = pa.getattr("RecordBatch")?;

    if data.is_instance(&table_type)? {
        let batches_py = data.call_method0("to_batches")?;
        let batches_list = batches_py.downcast::<PyList>()?;
        let mut batches = Vec::with_capacity(batches_list.len());
        for batch_obj in batches_list.iter() {
            let batch = pyo3_arrow::PyArrowConvert::from_pyarrow(&batch_obj)?;
            batches.push(batch);
        }
        return Ok(batches);
    }

    if data.is_instance(&batch_type)? {
        let batch = pyo3_arrow::PyArrowConvert::from_pyarrow(data)?;
        return Ok(vec![batch]);
    }

    Err(PyTypeError::new_err("not a PyArrow type"))
}

/// Try converting a Pandas DataFrame via PyArrow.
fn try_pandas(py: Python<'_>, data: &Bound<'_, PyAny>) -> PyResult<Vec<RecordBatch>> {
    let pd = py.import("pandas").map_err(|_| PyTypeError::new_err("not pandas"))?;
    let df_type = pd.getattr("DataFrame")?;

    if !data.is_instance(&df_type)? {
        return Err(PyTypeError::new_err("not a Pandas DataFrame"));
    }

    let pa = py.import("pyarrow")?;
    let table = pa.call_method1("Table.from_pandas", (data,))
        .or_else(|_| {
            let from_pandas = pa.getattr("Table")?.getattr("from_pandas")?;
            from_pandas.call1((data,))
        })?;

    try_pyarrow(py, &table)
}

/// Try converting a Polars DataFrame via `.to_arrow()`.
fn try_polars(py: Python<'_>, data: &Bound<'_, PyAny>) -> PyResult<Vec<RecordBatch>> {
    let pl = py.import("polars").map_err(|_| PyTypeError::new_err("not polars"))?;
    let df_type = pl.getattr("DataFrame")?;

    if !data.is_instance(&df_type)? {
        return Err(PyTypeError::new_err("not a Polars DataFrame"));
    }

    let arrow_table = data.call_method0("to_arrow")?;
    try_pyarrow(py, &arrow_table)
}

/// Try converting Python dicts/lists to Arrow RecordBatches.
fn try_python_dicts(py: Python<'_>, data: &Bound<'_, PyAny>, schema: Option<&Schema>) -> PyResult<Vec<RecordBatch>> {
    // Single dict — could be columnar or single row
    if let Ok(dict) = data.downcast::<PyDict>() {
        return dict_to_batches(py, dict, schema);
    }

    // List of dicts
    if let Ok(list) = data.downcast::<PyList>() {
        if list.is_empty() {
            return Err(PyTypeError::new_err("empty list"));
        }
        return list_of_dicts_to_batches(py, list, schema);
    }

    Err(PyTypeError::new_err("not a dict or list"))
}

/// Convert a Python dict to RecordBatch.
///
/// If values are lists → columnar format.
/// If values are scalars → single row.
fn dict_to_batches(py: Python<'_>, dict: &Bound<'_, PyDict>, schema: Option<&Schema>) -> PyResult<Vec<RecordBatch>> {
    // Check if it's columnar (values are lists) or a single row (values are scalars)
    let first_value = dict.values().get_item(0)
        .map_err(|_| PyTypeError::new_err("empty dict"))?;
    let is_columnar = first_value.downcast::<PyList>().is_ok();

    if is_columnar {
        columnar_dict_to_batch(py, dict, schema)
    } else {
        // Single row: wrap in a list-of-dicts conversion
        let list = PyList::new(py, &[dict.as_any()])?;
        list_of_dicts_to_batches(py, &list, schema)
    }
}

/// Convert a columnar dict (str → list) to RecordBatch.
fn columnar_dict_to_batch(py: Python<'_>, dict: &Bound<'_, PyDict>, _schema: Option<&Schema>) -> PyResult<Vec<RecordBatch>> {
    // Use pyarrow for robust conversion: pa.table(dict)
    let pa = py.import("pyarrow")?;
    let table_cls = pa.getattr("table")?;
    let table = table_cls.call1((dict,))?;
    try_pyarrow(py, &table)
}

/// Convert a list of dicts to RecordBatch.
fn list_of_dicts_to_batches(py: Python<'_>, list: &Bound<'_, PyList>, _schema: Option<&Schema>) -> PyResult<Vec<RecordBatch>> {
    // Convert to JSON and use arrow-json reader as a robust path
    let json_mod = py.import("json")?;
    let json_str: String = json_mod.call_method1("dumps", (list,))?.extract()?;
    json_str_to_batches(&json_str, _schema)
}

// ---------------------------------------------------------------------------
// JSON / CSV string → Arrow
// ---------------------------------------------------------------------------

/// Parse a JSON string into Arrow RecordBatches.
pub fn json_str_to_batches(json: &str, _schema: Option<&Schema>) -> PyResult<Vec<RecordBatch>> {
    let cursor = std::io::Cursor::new(json.as_bytes());

    let mut reader = arrow_json::ReaderBuilder::new_inferred()
        .build(cursor)
        .map_err(|e| IngestionError::new_err(format!("JSON parse error: {e}")))?;

    let mut batches = Vec::new();
    while let Some(batch) = reader.next() {
        batches.push(
            batch.map_err(|e| IngestionError::new_err(format!("JSON read error: {e}")))?,
        );
    }
    Ok(batches)
}

/// Parse a CSV string into Arrow RecordBatches.
pub fn csv_str_to_batches(csv: &str, _schema: Option<&Schema>) -> PyResult<Vec<RecordBatch>> {
    let cursor = std::io::Cursor::new(csv.as_bytes());

    let reader = arrow_csv::ReaderBuilder::new()
        .has_header(true)
        .build(cursor)
        .map_err(|e| IngestionError::new_err(format!("CSV parse error: {e}")))?;

    let mut batches = Vec::new();
    for batch in reader {
        batches.push(
            batch.map_err(|e| IngestionError::new_err(format!("CSV read error: {e}")))?,
        );
    }
    Ok(batches)
}

// ---------------------------------------------------------------------------
// Arrow → Python (output conversion)
// ---------------------------------------------------------------------------

/// Convert RecordBatches to a PyArrow Table.
pub fn batches_to_pyarrow<'py>(py: Python<'py>, batches: &[RecordBatch], schema: &Schema) -> PyResult<Bound<'py, PyAny>> {
    let pa = py.import("pyarrow")?;
    let pa_schema = pyo3_arrow::PyArrowConvert::to_pyarrow(&Arc::new(schema.clone()), py)?;

    let py_batches: Vec<Bound<'_, PyAny>> = batches
        .iter()
        .map(|b| pyo3_arrow::PyArrowConvert::to_pyarrow(b, py))
        .collect::<PyResult<_>>()?;

    let batch_list = PyList::new(py, &py_batches)?;
    let table_cls = pa.getattr("Table")?;
    table_cls.call_method1("from_batches", (batch_list, pa_schema))
}

/// Convert RecordBatches to a Pandas DataFrame.
pub fn batches_to_pandas<'py>(py: Python<'py>, batches: &[RecordBatch], schema: &Schema) -> PyResult<Bound<'py, PyAny>> {
    let table = batches_to_pyarrow(py, batches, schema)?;
    table.call_method0("to_pandas")
}

/// Convert RecordBatches to a Polars DataFrame.
pub fn batches_to_polars<'py>(py: Python<'py>, batches: &[RecordBatch], schema: &Schema) -> PyResult<Bound<'py, PyAny>> {
    let pl = py.import("polars")?;
    let table = batches_to_pyarrow(py, batches, schema)?;
    pl.call_method1("from_arrow", (table,))
}

/// Convert RecordBatches to a list of Python dicts.
pub fn batches_to_dicts<'py>(py: Python<'py>, batches: &[RecordBatch], schema: &Schema) -> PyResult<Bound<'py, PyAny>> {
    let table = batches_to_pyarrow(py, batches, schema)?;
    table.call_method0("to_pydict")
}

/// Auto-detect the best available output library.
pub fn batches_to_best_df<'py>(py: Python<'py>, batches: &[RecordBatch], schema: &Schema) -> PyResult<Bound<'py, PyAny>> {
    // Try polars first (better performance), then pandas, then pyarrow
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
pub fn python_to_schema(py: Python<'_>, obj: &Bound<'_, PyAny>) -> PyResult<Schema> {
    // Try PyArrow Schema first
    if let Ok(schema) = pyo3_arrow::PyArrowConvert::from_pyarrow(obj) {
        return Ok(schema);
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
        "timestamp" | "datetime" => Ok(DataType::Timestamp(arrow_schema::TimeUnit::Microsecond, None)),
        _ => Err(crate::error::SchemaError::new_err(format!("Unknown type: {s}"))),
    }
}
