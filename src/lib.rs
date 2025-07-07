use pyo3::prelude::*;
use pyo3::types::PyDict;
use pyo3::wrap_pymodule;

/// Submodule for Suggest/AMP.
mod amp;

/// Python extensions for Mozilla/Merino implemented in Rust using PyO3.
#[pymodule]
fn moz_merino_ext(py: Python<'_>, m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add("__version__", env!("CARGO_PKG_VERSION"))?;
    m.add_wrapped(wrap_pymodule!(amp::submodule))?;

    // Inserting to sys.modules allows importing submodules nicely from Python
    // e.g. from moz_merino_ext.submodule import SubmoduleClass

    let sys = PyModule::import(py, "sys")?;
    let sys_modules: Bound<'_, PyDict> = sys.getattr("modules")?.downcast_into()?;
    sys_modules.set_item("moz_merino_ext.amp", m.getattr("amp")?)?;

    Ok(())
}
