//! The Timeseries state a mock session holds: documents, lifecycle, jobs.

use crate::data::mock::explorer::observed_at;
use crate::data::mock::timeseries::backfill::RunStore;
use crate::data::mock::timeseries::dashboards::DashStore;
use crate::data::mock::timeseries::lifecycle::LifecycleStore;

#[derive(Debug, Clone)]
pub struct TimeseriesStore {
    pub dashboards: DashStore,
    pub lifecycle: LifecycleStore,
    pub runs: RunStore,
    ops: u32,
}

impl TimeseriesStore {
    pub fn seeded() -> Self {
        Self {
            dashboards: DashStore::seeded(observed_at()),
            lifecycle: LifecycleStore::seeded(),
            runs: RunStore::seeded(),
            ops: 104,
        }
    }

    /// The next audit identifier for an operation.
    pub fn next_operation(&mut self) -> String {
        self.ops += 1;
        format!("op-{:04}", self.ops)
    }
}
