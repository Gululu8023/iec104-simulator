use crate::core::types::DataPoint;

pub(crate) mod control;
pub(crate) mod counter;
pub(crate) mod file_transfer;
pub(crate) mod interrogation;
pub(crate) mod read;
pub(crate) mod system;

#[derive(Default)]
pub(crate) struct PointMutationResult {
    pub(crate) points_changed: bool,
    pub(crate) spontaneous_updates: Vec<DataPoint>,
    pub(crate) persisted_points: Vec<DataPoint>,
}
