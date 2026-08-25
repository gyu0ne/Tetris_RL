#![forbid(unsafe_code)]

#[cfg(feature = "python-extension")]
mod python {
    use arena::SoloBatch;
    use pyo3::exceptions::PyValueError;
    use pyo3::prelude::*;
    use pyo3::types::PyBytes;
    use std::mem::size_of;

    #[pyclass(name = "SoloBatch")]
    struct PySoloBatch {
        inner: SoloBatch,
    }

    #[pymethods]
    impl PySoloBatch {
        #[new]
        fn new(seeds: Vec<u64>) -> PyResult<Self> {
            Ok(Self {
                inner: SoloBatch::new(&seeds).map_err(value_error)?,
            })
        }

        fn candidates<'py>(
            &mut self,
            py: Python<'py>,
        ) -> PyResult<(Bound<'py, PyBytes>, Vec<usize>, Vec<bool>)> {
            let batch = self.inner.candidates().map_err(value_error)?;
            let mut bytes = Vec::with_capacity(batch.features.len() * 10 * size_of::<i32>());
            for features in batch.features {
                for value in features {
                    bytes.extend_from_slice(&value.to_le_bytes());
                }
            }
            Ok((PyBytes::new(py, &bytes), batch.offsets, batch.done))
        }

        fn step(&mut self, selections: Vec<i64>) -> PyResult<()> {
            let parsed = selections
                .into_iter()
                .map(|selection| {
                    if selection < 0 {
                        Ok(None)
                    } else {
                        usize::try_from(selection).map(Some).map_err(value_error)
                    }
                })
                .collect::<PyResult<Vec<_>>>()?;
            self.inner.step(&parsed).map_err(value_error)
        }

        fn pieces_placed(&self) -> Vec<u64> {
            self.inner.pieces_placed()
        }

        fn game_count(&self) -> usize {
            self.inner.game_count()
        }
    }

    #[pymodule]
    fn tetris_engine(module: &Bound<'_, PyModule>) -> PyResult<()> {
        module.add_class::<PySoloBatch>()?;
        Ok(())
    }

    fn value_error(error: impl std::fmt::Display) -> PyErr {
        PyValueError::new_err(error.to_string())
    }
}
