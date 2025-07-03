use pyo3::exceptions::{PyIOError, PyKeyError, PyTypeError, PyValueError};
use pyo3::prelude::*;
use pyo3::types::{PyBytes, PyString};
use std::collections::HashMap;
use std::sync::{Arc, RwLock};

use crate::amp::domain::{AmpIndexer, AmpResult, OriginalAmp, load_amp_data};
use crate::amp::index::BTreeAmpIndex;

mod domain;
mod index;

#[pyclass]
#[derive(Clone, PartialEq)]
pub struct PyAmpResult {
    #[pyo3(get)]
    pub title: String,
    #[pyo3(get)]
    pub url: String,
    #[pyo3(get)]
    pub click_url: String,
    #[pyo3(get)]
    pub impression_url: String,
    #[pyo3(get)]
    pub advertiser: String,
    #[pyo3(get)]
    pub block_id: i32,
    #[pyo3(get)]
    pub iab_category: String,
    #[pyo3(get)]
    pub icon: String,
    #[pyo3(get)]
    pub full_keyword: String,
}

impl From<AmpResult> for PyAmpResult {
    fn from(result: AmpResult) -> Self {
        PyAmpResult {
            title: result.title,
            url: result.url,
            click_url: result.click_url,
            impression_url: result.impression_url,
            advertiser: result.advertiser,
            block_id: result.block_id,
            iab_category: result.iab_category,
            icon: result.icon,
            full_keyword: result.full_keyword,
        }
    }
}

#[pyclass]
pub struct AmpIndexManager {
    indexes: Arc<RwLock<HashMap<String, BTreeAmpIndex>>>,
}

#[pymethods]
impl AmpIndexManager {
    #[new]
    fn new() -> PyResult<Self> {
        Ok(AmpIndexManager {
            indexes: Arc::new(RwLock::new(HashMap::new())),
        })
    }

    /// Build index from JSON file, made for development & debugging.
    #[pyo3(signature = (index_name, json_path, /))]
    fn build_from_file(&self, index_name: String, json_path: String) -> PyResult<()> {
        let amps = load_amp_data(json_path)
            .map_err(|e| PyIOError::new_err(format!("Failed to load JSON: {}", e)))?;

        let mut index = BTreeAmpIndex::new();
        index
            .build(&amps)
            .map_err(|e| PyValueError::new_err(format!("Failed to build index: {}", e)))?;

        let mut indexes = self.indexes.write().unwrap();
        indexes.insert(index_name, index);
        Ok(())
    }

    /// Build an index from an AMP suggestion payload in JSON and insert it into the index manager.
    /// If the index name is already stored in the index manager, it overwrites the old one.
    ///
    /// Args:
    ///   - `index_name`: the index name.
    ///   - `data`: either a JSON encoded Python `str` or `bytes` for an AMP suggestion payload.
    /// Returns:
    ///   - None
    /// Errors:
    ///   - `TypeError` if the input type is not Python `str` or `bytes`.
    ///   - `ValueError` if the JSON payload is malformed.
    #[pyo3(signature = (index_name, data, /))]
    fn build(&self, index_name: String, data: &Bound<PyAny>) -> PyResult<()> {
        // let amps: Vec<OriginalAmp> = if data.is_instance_of::<PyBytes>() {
        let amps: Vec<OriginalAmp> = if data.is_instance_of::<PyBytes>() {
            let input = data.downcast::<PyBytes>()?.extract::<&[u8]>()?;
            serde_json::from_slice(input)
                .map_err(|e| PyValueError::new_err(format!("Invalid JSON bytes: {}", e)))?
        } else if data.is_instance_of::<PyString>() {
            let input = data.downcast::<PyString>()?.extract::<&str>()?;
            serde_json::from_str(input)
                .map_err(|e| PyValueError::new_err(format!("Invalid JSON string: {}", e)))?
        } else {
            return Err(PyTypeError::new_err("Invalid type for the index input"));
        };

        let mut index = BTreeAmpIndex::new();
        index
            .build(&amps)
            .map_err(|e| PyValueError::new_err(format!("Failed to build index: {}", e)))?;

        let mut indexes = self.indexes.write().unwrap();
        indexes.insert(index_name, index);
        Ok(())
    }

    /// Query against the index for a given query.
    ///
    /// Args:
    ///   - `index_name`: the index name.
    ///   - `query`: the query to look up.
    /// Returns:
    ///   - A vector of `PyAmpResult`.
    /// Errors:
    ///   - `KeyError` if the given index is missing.
    ///   - `ValueError` if the query fails.
    #[pyo3(signature = (index_name, query, /))]
    fn query(&self, index_name: &str, query: &str) -> PyResult<Vec<PyAmpResult>> {
        let indexes = self.indexes.read().unwrap();
        let index = indexes
            .get(index_name)
            .ok_or_else(|| PyKeyError::new_err(format!("Index '{}' not found", index_name)))?;

        let results = index
            .query(query)
            .map_err(|e| PyValueError::new_err(format!("Query failed: {}", e)))?;

        Ok(results.into_iter().map(PyAmpResult::from).collect())
    }

    /// Delete a given index from the index manager.
    #[pyo3(signature = (index_name, /))]
    fn delete(&self, index_name: &str) -> PyResult<()> {
        let mut indexes = self.indexes.write().unwrap();
        indexes
            .remove(index_name)
            .ok_or_else(|| PyKeyError::new_err(format!("Index '{}' not found", index_name)))?;
        Ok(())
    }

    /// List all the indices from the index manager.
    #[pyo3(signature = (/))]
    fn list(&self) -> Vec<String> {
        let indexes = self.indexes.read().unwrap();
        indexes.keys().cloned().collect()
    }

    /// Check if an index exists in the index manager.
    #[pyo3(signature = (index_name, /))]
    fn has(&self, index_name: &str) -> bool {
        let indexes = self.indexes.read().unwrap();
        indexes.contains_key(index_name)
    }

    /// List all the icons of a given index.
    /// Args:
    ///   - `index_name`: the index name.
    /// Returns:
    ///   - A vector of icon IDs.
    /// Errors:
    ///   - `KeyError` if the given index is missing.
    #[pyo3(signature = (index_name, /))]
    fn list_icons(&self, index_name: &str) -> PyResult<Vec<String>> {
        let indexes = self.indexes.read().unwrap();
        let index = indexes
            .get(index_name)
            .ok_or_else(|| PyKeyError::new_err(format!("Index '{}' not found", index_name)))?;

        Ok(index.list_icons())
    }

    /// Fetch index stats for a given index.
    /// Args:
    ///   - `index_name`: the index name.
    /// Returns:
    ///   - A stats dictionary.
    /// Errors:
    ///   - `KeyError` if the given index is missing.
    #[pyo3(signature = (index_name, /))]
    fn stats(&self, index_name: &str) -> PyResult<HashMap<String, usize>> {
        let indexes = self.indexes.read().unwrap();
        let index = indexes
            .get(index_name)
            .ok_or_else(|| PyKeyError::new_err(format!("Index '{}' not found", index_name)))?;

        Ok(index.stats())
    }
}

/// Submodule for the "amp" extension.
#[pymodule(name = "amp")]
pub(crate) fn submodule(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_class::<PyAmpResult>()?;
    m.add_class::<AmpIndexManager>()?;
    Ok(())
}
