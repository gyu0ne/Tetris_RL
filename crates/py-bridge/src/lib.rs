#![forbid(unsafe_code)]

#[cfg(feature = "python-extension")]
mod python {
    use arena::{SoloBatch, VersusBatch};
    use pyo3::exceptions::PyValueError;
    use pyo3::prelude::*;
    use pyo3::types::PyBytes;
    use std::mem::size_of;

    type LabeledCandidateBatch<'py> = (
        Bound<'py, PyBytes>,
        Bound<'py, PyBytes>,
        Vec<usize>,
        Vec<bool>,
    );

    #[pyclass(name = "SoloBatch")]
    struct PySoloBatch {
        inner: SoloBatch,
    }

    #[pyclass(name = "VersusBatch")]
    struct PyVersusBatch {
        inner: VersusBatch,
    }

    #[pymethods]
    impl PyVersusBatch {
        #[new]
        fn new(seeds: Vec<u64>, frames_per_placement: u32) -> PyResult<Self> {
            Ok(Self {
                inner: VersusBatch::new(&seeds, frames_per_placement).map_err(value_error)?,
            })
        }

        fn candidates<'py>(
            &mut self,
            py: Python<'py>,
        ) -> PyResult<(
            Bound<'py, PyBytes>,
            Bound<'py, PyBytes>,
            Vec<usize>,
            Vec<bool>,
            Vec<i8>,
        )> {
            let batch = self.inner.candidates().map_err(value_error)?;
            Ok((
                PyBytes::new(py, &feature_bytes(batch.features)),
                PyBytes::new(py, &feature_bytes(batch.state_features)),
                batch.offsets,
                batch.done,
                batch.results,
            ))
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

        fn reset_done(&mut self, seeds: Vec<u64>) -> PyResult<()> {
            self.inner.reset_done(&seeds).map_err(value_error)
        }

        fn match_count(&self) -> usize {
            self.inner.match_count()
        }
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
            let bytes = feature_bytes(batch.features);
            Ok((PyBytes::new(py, &bytes), batch.offsets, batch.done))
        }

        fn labeled_candidates<'py>(
            &mut self,
            py: Python<'py>,
        ) -> PyResult<LabeledCandidateBatch<'py>> {
            let batch = self.inner.labeled_candidates().map_err(value_error)?;
            let features = feature_bytes(batch.features);
            let mut scores = Vec::with_capacity(batch.teacher_scores.len() * size_of::<i64>());
            for score in batch.teacher_scores {
                scores.extend_from_slice(&score.to_le_bytes());
            }
            Ok((
                PyBytes::new(py, &features),
                PyBytes::new(py, &scores),
                batch.offsets,
                batch.done,
            ))
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

        fn snapshot(&self, index: usize) -> PyResult<SoloSnapshotTuple> {
            let snapshot = self.inner.snapshot(index).map_err(value_error)?;
            Ok((
                snapshot.board_rows,
                snapshot.garbage_rows,
                snapshot.active,
                snapshot.hold,
                snapshot.preview,
                snapshot.pieces_placed,
                snapshot.top_out,
            ))
        }
    }

    type SoloSnapshotTuple = (
        Vec<u16>,
        Vec<u16>,
        String,
        Option<String>,
        Vec<String>,
        u64,
        bool,
    );

    #[pymodule]
    fn tetris_engine(module: &Bound<'_, PyModule>) -> PyResult<()> {
        module.add_class::<PySoloBatch>()?;
        module.add_class::<PyVersusBatch>()?;
        Ok(())
    }

    fn value_error(error: impl std::fmt::Display) -> PyErr {
        PyValueError::new_err(error.to_string())
    }

    fn feature_bytes<const N: usize>(features: Vec<[i32; N]>) -> Vec<u8> {
        let mut bytes = Vec::with_capacity(features.len() * N * size_of::<i32>());
        for features in features {
            for value in features {
                bytes.extend_from_slice(&value.to_le_bytes());
            }
        }
        bytes
    }
}
