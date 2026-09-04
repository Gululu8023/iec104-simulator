//! 从站版本化数据点存储及批量点表替换。

use std::collections::HashSet;

use crate::core::{
    iec104_registry::{iec104_protocol_default_value, iec104_type_name},
    shared::versioned_point_store::{PointStoreReadGuard, PointStoreSync, PointStoreTransaction, VersionedPointStore},
    slave::{
        protocol::quality::{apply_quality_metadata, quality_common_from_raw, quality_detail_from_point},
        simulation::engine::{PointKey, point_key},
    },
    types::{DataPoint, PointDef, PointSource, PointSyncCursor},
};

#[derive(Debug, Default)]
pub(crate) struct SlavePointStore {
    inner: VersionedPointStore<PointKey, DataPoint>,
}

impl SlavePointStore {
    pub(crate) fn new() -> Self {
        Self::default()
    }

    pub(crate) async fn read(&self) -> PointStoreReadGuard<'_, PointKey, DataPoint> {
        self.inner.read().await
    }

    pub(crate) async fn transaction(&self) -> PointStoreTransaction<'_, PointKey, DataPoint> {
        self.inner.transaction().await
    }

    pub(crate) async fn sync_since(&self, cursor: Option<PointSyncCursor>) -> PointStoreSync<PointKey, DataPoint> {
        self.inner.sync_since(cursor).await
    }

    pub(crate) async fn cursor(&self) -> PointSyncCursor {
        self.inner.cursor().await
    }

    pub(crate) async fn replace_common_address_definitions(
        &self,
        common_address: u16,
        defs: &[PointDef],
    ) -> HashSet<PointKey> {
        let now = chrono::Utc::now();
        let mut points = self.inner.transaction().await;
        let mut keep = HashSet::with_capacity(defs.len());

        for def in defs.iter().filter(|def| def.is_enabled) {
            let key = point_key(common_address, def.address);
            keep.insert(key);
            let entry = points.entry(key).or_insert_with(|| {
                let value = def
                    .default_value
                    .as_ref()
                    .and_then(default_value_to_f64)
                    .unwrap_or_else(|| iec104_protocol_default_value(def.type_id));
                DataPoint {
                    connection_id: "local".to_string(),
                    link_profile_id: None,
                    slave_id: None,
                    common_address: Some(common_address),
                    address: def.address,
                    name: def.name.clone(),
                    description: def.description.clone(),
                    control_ioa: def.control_ioa,
                    gi_group: def.gi_group,
                    counter_group: def.counter_group,
                    type_id: def.type_id,
                    data_type: iec104_type_name(def.type_id).to_string(),
                    value,
                    quality: 0,
                    quality_common: Some(quality_common_from_raw(def.type_id, 0)),
                    quality_detail: Some(quality_detail_from_point(def.type_id, 0, value)),
                    business_usable: true,
                    timestamp: now,
                    latest_event_timestamp: None,
                    timestamp_detail: None,
                    report_count: None,
                    latest_cause: None,
                    point_source: Some(PointSource::Manual),
                    control_status_snapshot: None,
                }
            });

            if entry.type_id != def.type_id
                && !crate::core::iec104_registry::same_type_family(entry.type_id, def.type_id)
            {
                let value = def
                    .default_value
                    .as_ref()
                    .and_then(default_value_to_f64)
                    .unwrap_or_else(|| iec104_protocol_default_value(def.type_id));
                entry.value = value;
                entry.quality = 0;
                entry.quality_common = Some(quality_common_from_raw(def.type_id, 0));
                entry.quality_detail = Some(quality_detail_from_point(def.type_id, 0, value));
                entry.timestamp = now;
                entry.latest_event_timestamp = None;
                entry.control_status_snapshot = None;
            }
            entry.name = def.name.clone();
            entry.description = def.description.clone();
            entry.control_ioa = def.control_ioa;
            entry.gi_group = def.gi_group;
            entry.counter_group = def.counter_group;
            entry.type_id = def.type_id;
            entry.data_type = iec104_type_name(def.type_id).to_string();
            entry.common_address = Some(common_address);
            apply_quality_metadata(entry, entry.quality);
        }

        points.remove_where(|(ca, ioa), _| *ca == common_address && !keep.contains(&(*ca, *ioa)));
        keep
    }

    pub(crate) async fn remove_common_address(&self, common_address: u16) {
        self.inner.transaction().await.remove_where(|(ca, _), _| *ca == common_address);
    }
}

fn default_value_to_f64(value: &serde_json::Value) -> Option<f64> {
    value
        .as_f64()
        .or_else(|| value.as_i64().map(|value| value as f64))
        .or_else(|| value.as_u64().map(|value| value as f64))
        .or_else(|| value.as_bool().map(|value| if value { 1.0 } else { 0.0 }))
        .or_else(|| value.as_str().and_then(|value| value.parse::<f64>().ok()))
}
