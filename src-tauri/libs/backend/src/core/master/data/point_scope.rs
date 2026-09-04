//! 主站运行数据点的结构化作用域和复合键。

use std::collections::HashSet;

use crate::core::types::PointDef;

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub(crate) enum MasterPointScope {
    ConfiguredSlave(i64),
    LiveConnection(String),
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub(crate) struct MasterPointKey {
    pub(crate) scope: MasterPointScope,
    pub(crate) common_address: u16,
    pub(crate) ioa: u32,
}

/// 主站点表派生的运行时准入索引。
#[derive(Debug, Default)]
pub(crate) struct MasterPointAdmission {
    configured_keys: HashSet<MasterPointKey>,
    disabled_keys: HashSet<MasterPointKey>,
    disabled_scopes: HashSet<(MasterPointScope, u16)>,
}

impl MasterPointAdmission {
    pub(crate) fn replace_scope(
        &mut self,
        scope: &MasterPointScope,
        common_address: u16,
        defs: &[PointDef],
        scope_enabled: bool,
    ) -> Vec<MasterPointKey> {
        let previous_keys = self.configured_keys.iter().filter(|key| &key.scope == scope).cloned().collect::<Vec<_>>();
        self.configured_keys.retain(|key| &key.scope != scope);
        self.disabled_keys.retain(|key| &key.scope != scope);
        self.disabled_scopes.retain(|(item_scope, _)| item_scope != scope);

        if !scope_enabled {
            self.disabled_scopes.insert((scope.clone(), common_address));
        }
        for def in defs {
            let key = data_point_key(scope, common_address, def.address);
            self.configured_keys.insert(key.clone());
            if !scope_enabled || !def.is_enabled {
                self.disabled_keys.insert(key);
            }
        }
        previous_keys
    }

    pub(crate) fn clear_scope(&mut self, scope: &MasterPointScope) -> Vec<MasterPointKey> {
        self.replace_scope(scope, 0, &[], true)
    }

    pub(crate) fn is_disabled(&self, scope: &MasterPointScope, common_address: u16, address: u32) -> bool {
        self.disabled_scopes.contains(&(scope.clone(), common_address))
            || self.disabled_scopes.contains(&(scope.clone(), 0))
            || self.disabled_keys.contains(&data_point_key(scope, common_address, address))
            || self.disabled_keys.contains(&data_point_key(scope, 0, address))
    }
}

pub(crate) fn data_point_key(scope: &MasterPointScope, common_address: u16, address: u32) -> MasterPointKey {
    MasterPointKey { scope: scope.clone(), common_address, ioa: address }
}

pub(crate) fn resolve_point_scope(connection_id: &str, slave_id: Option<i64>) -> MasterPointScope {
    if let Some(value) = slave_id {
        return MasterPointScope::ConfiguredSlave(value);
    }
    MasterPointScope::LiveConnection(connection_id.to_string())
}
