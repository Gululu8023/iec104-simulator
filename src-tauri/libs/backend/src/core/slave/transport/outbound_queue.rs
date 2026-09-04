//! 从站出站队列管理模块。
//!
//! 本模块定义出站队列的数据结构和优先级管理,支持多优先级调度和去重,包括:
//!
//! - **优先级定义**:四级优先级(Control > Interrogation > Soe > Spontaneous)
//! - **工作项类型**:单帧、批量帧、召唤序列
//! - **去重支持**:基于 dedupe_key 避免重复发送相同内容
//! - **序列分组**:支持将多个工作项归为同一序列组
//! - **完成通知**:通过 oneshot channel 通知发送完成状态
//! - **容量限制**:队列容量范围 64-65535,防止内存溢出
//! - **等待时间跟踪**:记录入队时间,用于统计队列延迟
//!
//! # 优先级说明
//!
//! - **Control**: 控制命令响应(最高优先级)
//! - **Interrogation**: 总召唤/计数量召唤响应
//! - **Soe**: SOE 事件上报
//! - **Spontaneous**: 自发变化上报(最低优先级)

use std::{collections::VecDeque, time::Instant};

use tokio::sync::oneshot;

use super::outbound_frame::EncodedOutboundFrame;
use crate::errors::AppResult;

pub(crate) const MIN_OUTBOUND_QUEUE_CAPACITY: usize = 64;
pub(crate) const MAX_OUTBOUND_QUEUE_CAPACITY: usize = 65_535;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) enum OutboundPriority {
    Control,
    Interrogation,
    Soe,
    Spontaneous,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum OutboundWorkKind {
    SingleAsdu,
    BatchedAsdu,
    InterrogationSequence,
}

pub(crate) struct OutboundWorkItem {
    pub(crate) priority: OutboundPriority,
    pub(crate) kind: OutboundWorkKind,
    pub(crate) dedupe_key: Option<String>,
    pub(crate) sequence_group: Option<String>,
    pub(crate) frames: Vec<EncodedOutboundFrame>,
    pub(crate) enqueued_at: Instant,
    pub(crate) completion: Option<oneshot::Sender<AppResult<()>>>,
}

impl OutboundWorkItem {
    pub(crate) fn single(
        priority: OutboundPriority,
        kind: OutboundWorkKind,
        dedupe_key: Option<String>,
        sequence_group: Option<String>,
        frame: EncodedOutboundFrame,
    ) -> (Self, oneshot::Receiver<AppResult<()>>) {
        let (tx, rx) = oneshot::channel();
        (
            Self {
                priority,
                kind,
                dedupe_key,
                sequence_group,
                frames: vec![frame],
                enqueued_at: Instant::now(),
                completion: Some(tx),
            },
            rx,
        )
    }

    pub(crate) fn sequence(
        priority: OutboundPriority,
        sequence_group: Option<String>,
        frames: Vec<EncodedOutboundFrame>,
    ) -> (Self, oneshot::Receiver<AppResult<()>>) {
        let (tx, rx) = oneshot::channel();
        (
            Self {
                priority,
                kind: OutboundWorkKind::InterrogationSequence,
                dedupe_key: None,
                sequence_group,
                frames,
                enqueued_at: Instant::now(),
                completion: Some(tx),
            },
            rx,
        )
    }
}

#[derive(Default)]
pub(crate) struct PeerOutboundState {
    pub(crate) control_queue: VecDeque<OutboundWorkItem>,
    pub(crate) interrogation_queue: VecDeque<OutboundWorkItem>,
    pub(crate) soe_queue: VecDeque<OutboundWorkItem>,
    pub(crate) spontaneous_queue: VecDeque<OutboundWorkItem>,
    pub(crate) stopped: bool,
    pub(crate) interrogation_active: bool,
}

impl PeerOutboundState {
    pub(crate) fn total_len(&self) -> usize {
        self.control_queue.len() + self.interrogation_queue.len() + self.soe_queue.len() + self.spontaneous_queue.len()
    }

    pub(crate) fn push(&mut self, item: OutboundWorkItem) {
        if item.priority == OutboundPriority::Interrogation {
            self.interrogation_active = true;
        }
        match item.priority {
            OutboundPriority::Control => self.control_queue.push_back(item),
            OutboundPriority::Interrogation => self.interrogation_queue.push_back(item),
            OutboundPriority::Soe => self.soe_queue.push_back(item),
            OutboundPriority::Spontaneous => self.spontaneous_queue.push_back(item),
        }
    }

    pub(crate) fn pop_next(&mut self) -> Option<OutboundWorkItem> {
        self.control_queue
            .pop_front()
            .or_else(|| self.interrogation_queue.pop_front())
            .or_else(|| self.soe_queue.pop_front())
            .or_else(|| self.spontaneous_queue.pop_front())
    }

    pub(crate) fn drain_all(&mut self) -> Vec<OutboundWorkItem> {
        let mut drained = Vec::with_capacity(self.total_len());
        drained.extend(self.control_queue.drain(..));
        drained.extend(self.interrogation_queue.drain(..));
        drained.extend(self.soe_queue.drain(..));
        drained.extend(self.spontaneous_queue.drain(..));
        self.interrogation_active = false;
        drained
    }
}

#[inline]
pub(crate) fn sanitize_outbound_queue_capacity(value: usize) -> usize {
    value.clamp(MIN_OUTBOUND_QUEUE_CAPACITY, MAX_OUTBOUND_QUEUE_CAPACITY)
}
