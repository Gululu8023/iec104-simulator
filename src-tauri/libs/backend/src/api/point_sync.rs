//! 主从站数据点增量同步协议。

use serde::{Deserialize, Serialize};

pub use crate::core::types::PointSyncCursor;
use crate::core::{
    master::data::point_scope::{MasterPointKey, MasterPointScope},
    shared::versioned_point_store::PointStoreSync,
    slave::simulation::engine::PointKey,
    types::DataPoint,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PointSyncScopeKind {
    ConfiguredSlave,
    LiveConnection,
    SlaveCommonAddress,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct PointSyncKey {
    pub scope_kind: PointSyncScopeKind,
    pub scope_id: String,
    pub common_address: u16,
    pub address: u32,
}

#[derive(Debug, Clone, Serialize)]
pub struct PointSyncItem {
    pub key: PointSyncKey,
    pub point: DataPoint,
}

#[derive(Debug, Clone, Serialize)]
pub struct PointSyncBatch {
    pub cursor: PointSyncCursor,
    pub reset: bool,
    pub upserts: Vec<PointSyncItem>,
    pub removed: Vec<PointSyncKey>,
}

fn master_key(key: MasterPointKey) -> PointSyncKey {
    let (scope_kind, scope_id) = match key.scope {
        MasterPointScope::ConfiguredSlave(id) => (PointSyncScopeKind::ConfiguredSlave, id.to_string()),
        MasterPointScope::LiveConnection(id) => (PointSyncScopeKind::LiveConnection, id),
    };
    PointSyncKey { scope_kind, scope_id, common_address: key.common_address, address: key.ioa }
}

pub(crate) fn master_batch(sync: PointStoreSync<MasterPointKey, DataPoint>) -> PointSyncBatch {
    PointSyncBatch {
        cursor: sync.cursor,
        reset: sync.reset,
        upserts: sync.upserts.into_iter().map(|(key, point)| PointSyncItem { key: master_key(key), point }).collect(),
        removed: sync.removed.into_iter().map(master_key).collect(),
    }
}

fn slave_key((common_address, address): PointKey) -> PointSyncKey {
    PointSyncKey {
        scope_kind: PointSyncScopeKind::SlaveCommonAddress,
        scope_id: common_address.to_string(),
        common_address,
        address,
    }
}

pub(crate) fn slave_batch(sync: PointStoreSync<PointKey, DataPoint>) -> PointSyncBatch {
    PointSyncBatch {
        cursor: sync.cursor,
        reset: sync.reset,
        upserts: sync.upserts.into_iter().map(|(key, point)| PointSyncItem { key: slave_key(key), point }).collect(),
        removed: sync.removed.into_iter().map(slave_key).collect(),
    }
}
