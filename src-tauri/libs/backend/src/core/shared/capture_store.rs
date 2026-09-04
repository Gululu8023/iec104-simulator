//! 容量限制存储模块。
//!
//! 提供固定容量的队列存储，自动淘汰旧数据。

use std::collections::VecDeque;

/// 向队列添加元素并保持容量限制
///
/// 添加新元素到队列尾部，如果超过最大容量则移除最旧的元素。
pub(crate) fn push_capped<T>(records: &mut VecDeque<T>, item: T, max_len: usize) {
    records.push_back(item);
    while records.len() > max_len {
        records.pop_front();
    }
}
