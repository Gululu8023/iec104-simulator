//! 从站自发上报合并模块。
//!
//! 本模块提供自发上报(COT=3)的变化合并功能,用于优化短时间内多次变化的上报效率,包括:
//!
//! - **变化合并**:将短时间窗口内同一数据点的多次变化合并为一次上报
//! - **最新值保留**:对于同一点位(公共地址+IOA),仅保留最新的变化值
//! - **延迟刷新**:支持可配置的刷新延迟(1-250毫秒),避免频繁发送
//! - **去重优化**:减少网络流量和主站处理负担
//!
//! # 工作原理
//!
//! ```text
//! 数据点变化 → 加入待发送队列 → 延迟刷新定时器
//!                                    ↓
//!                          定时器到期 → 批量发送所有待发送点
//! ```
//!
//! # 参数限制
//!
//! - 最小刷新延迟: 1 毫秒
//! - 最大刷新延迟: 250 毫秒

use std::collections::HashMap;

use crate::core::types::DataPoint;

pub(crate) const MIN_SPONTANEOUS_FLUSH_MILLIS: u64 = 1;
pub(crate) const MAX_SPONTANEOUS_FLUSH_MILLIS: u64 = 250;

#[derive(Default)]
pub(crate) struct SpontaneousCoalescer {
    pub(crate) pending_points: HashMap<(u16, u32), DataPoint>,
    pub(crate) flush_active: bool,
}

#[inline]
pub(crate) fn sanitize_spontaneous_flush_ms(value: u64) -> u64 {
    value.clamp(MIN_SPONTANEOUS_FLUSH_MILLIS, MAX_SPONTANEOUS_FLUSH_MILLIS)
}
