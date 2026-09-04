//! 带版本游标的数据点存储。

use std::{
    collections::{HashMap, HashSet, VecDeque, hash_map::Entry},
    hash::Hash,
    ops::Deref,
    sync::atomic::{AtomicU64, Ordering},
};

use tokio::sync::{RwLock, RwLockReadGuard, RwLockWriteGuard};

use crate::core::types::PointSyncCursor;

const MAX_JOURNAL_BATCHES: usize = 256;
const MAX_JOURNAL_KEYS: usize = 65_536;
static NEXT_EPOCH: AtomicU64 = AtomicU64::new(1);

#[derive(Debug)]
pub struct PointStoreSync<K, V> {
    pub cursor: PointSyncCursor,
    pub reset: bool,
    pub upserts: Vec<(K, V)>,
    pub removed: Vec<K>,
}

#[derive(Debug)]
struct JournalBatch<K> {
    revision: u64,
    keys: Vec<K>,
}

#[derive(Debug)]
struct StoreState<K, V> {
    points: HashMap<K, V>,
    epoch: u64,
    revision: u64,
    journal: VecDeque<JournalBatch<K>>,
    journal_key_count: usize,
}

/// 数据点 Map、版本游标和变化日志共享同一把锁，保证同步快照一致。
#[derive(Debug)]
pub struct VersionedPointStore<K, V> {
    state: RwLock<StoreState<K, V>>,
}

impl<K, V> Default for VersionedPointStore<K, V> {
    fn default() -> Self {
        Self::new()
    }
}

impl<K, V> VersionedPointStore<K, V> {
    pub fn new() -> Self {
        Self {
            state: RwLock::new(StoreState {
                points: HashMap::new(),
                epoch: NEXT_EPOCH.fetch_add(1, Ordering::Relaxed),
                revision: 0,
                journal: VecDeque::new(),
                journal_key_count: 0,
            }),
        }
    }
}

impl<K, V> VersionedPointStore<K, V>
where
    K: Clone + Eq + Hash,
    V: Clone + PartialEq,
{
    pub async fn read(&self) -> PointStoreReadGuard<'_, K, V> {
        PointStoreReadGuard { guard: self.state.read().await }
    }

    pub async fn transaction(&self) -> PointStoreTransaction<'_, K, V> {
        PointStoreTransaction { guard: self.state.write().await, originals: HashMap::new(), changed: HashSet::new() }
    }

    pub async fn cursor(&self) -> PointSyncCursor {
        let guard = self.state.read().await;
        PointSyncCursor { epoch: guard.epoch, revision: guard.revision }
    }
}

impl<K, V> VersionedPointStore<K, V>
where
    K: Clone + Eq + Hash,
    V: Clone,
{
    pub async fn sync_since(&self, cursor: Option<PointSyncCursor>) -> PointStoreSync<K, V> {
        let guard = self.state.read().await;
        let current = PointSyncCursor { epoch: guard.epoch, revision: guard.revision };
        let oldest_available_revision =
            guard.journal.front().map_or(guard.revision, |batch| batch.revision.saturating_sub(1));
        let reset = cursor.is_none_or(|value| {
            value.epoch != guard.epoch || value.revision > guard.revision || value.revision < oldest_available_revision
        });

        if reset {
            return PointStoreSync {
                cursor: current,
                reset: true,
                upserts: guard.points.iter().map(|(key, value)| (key.clone(), value.clone())).collect(),
                removed: Vec::new(),
            };
        }

        let revision = cursor.map_or(0, |value| value.revision);
        let mut changed = HashSet::new();
        for batch in guard.journal.iter().filter(|batch| batch.revision > revision) {
            changed.extend(batch.keys.iter().cloned());
        }

        let mut upserts = Vec::new();
        let mut removed = Vec::new();
        for key in changed {
            if let Some(value) = guard.points.get(&key) {
                upserts.push((key, value.clone()));
            } else {
                removed.push(key);
            }
        }
        PointStoreSync { cursor: current, reset: false, upserts, removed }
    }
}

pub struct PointStoreReadGuard<'a, K, V> {
    guard: RwLockReadGuard<'a, StoreState<K, V>>,
}

impl<K, V> Deref for PointStoreReadGuard<'_, K, V> {
    type Target = HashMap<K, V>;

    fn deref(&self) -> &Self::Target {
        &self.guard.points
    }
}

/// 写事务只在被触碰的键上生成 journal，避免高频更新退化为全表比较。
pub struct PointStoreTransaction<'a, K: Clone + Eq + Hash, V: Clone + PartialEq> {
    guard: RwLockWriteGuard<'a, StoreState<K, V>>,
    originals: HashMap<K, Option<V>>,
    changed: HashSet<K>,
}

impl<K, V> PointStoreTransaction<'_, K, V>
where
    K: Clone + Eq + Hash,
    V: Clone + PartialEq,
{
    fn record_original(&mut self, key: &K) {
        if self.originals.contains_key(key) {
            return;
        }
        self.originals.insert(key.clone(), self.guard.points.get(key).cloned());
    }

    pub fn get_mut(&mut self, key: &K) -> Option<&mut V> {
        self.record_original(key);
        self.guard.points.get_mut(key)
    }

    pub fn entry(&mut self, key: K) -> Entry<'_, K, V> {
        self.record_original(&key);
        self.guard.points.entry(key)
    }

    pub fn insert(&mut self, key: K, value: V) -> Option<V> {
        self.record_original(&key);
        self.guard.points.insert(key, value)
    }

    pub fn remove(&mut self, key: &K) -> Option<V> {
        self.record_original(key);
        self.guard.points.remove(key)
    }

    pub fn remove_where(&mut self, mut predicate: impl FnMut(&K, &V) -> bool) {
        let keys = self
            .guard
            .points
            .iter()
            .filter_map(|(key, value)| predicate(key, value).then(|| key.clone()))
            .collect::<Vec<_>>();
        for key in keys {
            self.remove(&key);
        }
    }

    pub fn for_each_mut(&mut self, mut update: impl FnMut(&K, &mut V) -> bool) {
        for (key, value) in &mut self.guard.points {
            if update(key, value) {
                self.changed.insert(key.clone());
            }
        }
    }

    #[allow(dead_code)]
    pub fn replace(&mut self, points: HashMap<K, V>) {
        for (key, value) in &self.guard.points {
            self.originals.entry(key.clone()).or_insert_with(|| Some(value.clone()));
        }
        for key in points.keys() {
            self.originals.entry(key.clone()).or_insert(None);
        }
        self.guard.points = points;
    }
}

impl<K: Clone + Eq + Hash, V: Clone + PartialEq> Deref for PointStoreTransaction<'_, K, V> {
    type Target = HashMap<K, V>;

    fn deref(&self) -> &Self::Target {
        &self.guard.points
    }
}

impl<K, V> Drop for PointStoreTransaction<'_, K, V>
where
    K: Clone + Eq + Hash,
    V: Clone + PartialEq,
{
    fn drop(&mut self) {
        let mut keys = std::mem::take(&mut self.changed);
        keys.extend(
            self.originals
                .drain()
                .filter_map(|(key, original)| (self.guard.points.get(&key) != original.as_ref()).then_some(key)),
        );
        if keys.is_empty() {
            return;
        }

        let keys = keys.into_iter().collect::<Vec<_>>();
        self.guard.revision = self.guard.revision.saturating_add(1);
        let revision = self.guard.revision;
        self.guard.journal_key_count = self.guard.journal_key_count.saturating_add(keys.len());
        self.guard.journal.push_back(JournalBatch { revision, keys });

        while self.guard.journal.len() > MAX_JOURNAL_BATCHES || self.guard.journal_key_count > MAX_JOURNAL_KEYS {
            let Some(batch) = self.guard.journal.pop_front() else {
                break;
            };
            self.guard.journal_key_count = self.guard.journal_key_count.saturating_sub(batch.keys.len());
        }
    }
}
