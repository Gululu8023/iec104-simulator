//! 从站载荷批处理模块。
//!
//! 本模块提供 ASDU 载荷的批量打包和优化功能,用于提高传输效率,包括:
//!
//! - **容量计算**:根据 ASDU 最大字节数和对象长度计算每帧最多容纳对象数
//! - **载荷合并**:将多个对象载荷合并为单个 ASDU 载荷(SQ=0 模式)
//! - **SQ=1 优化**:尝试将连续地址的对象打包为 SQ=1 格式以节省空间
//! - **地址连续性检查**:验证对象地址是否连续递增
//! - **协议限制**:遵守 VSQ 最多 127 个对象的限制
//!
//! # SQ=1 优化原理
//!
//! SQ=1 模式下,仅首个对象包含 3 字节 IOA,后续对象省略 IOA,节省空间:
//! - SQ=0: 每个对象 = IOA(3字节) + 数据
//! - SQ=1: 首对象 = IOA(3字节) + 数据, 后续对象 = 数据(省略IOA)

pub(crate) const IEC104_MAX_VSQ_OBJECTS: usize = 127;

#[inline]
pub(crate) fn max_objects_per_asdu(object_payload_len: usize, max_asdu_bytes: u16) -> usize {
    if object_payload_len == 0 {
        return 1;
    }

    let max_asdu_bytes = usize::from(max_asdu_bytes.clamp(4, 249));
    let max_payload_len = max_asdu_bytes.saturating_sub(6);
    let by_size = max_payload_len / object_payload_len;
    by_size.clamp(1, IEC104_MAX_VSQ_OBJECTS)
}

#[inline]
pub(crate) fn combine_payloads(chunks: &[Vec<u8>]) -> Vec<u8> {
    let mut combined = Vec::with_capacity(chunks.iter().map(|payload| payload.len()).sum());
    for payload in chunks {
        combined.extend_from_slice(payload);
    }
    combined
}

pub(crate) fn try_compose_sq1_payload(chunks: &[Vec<u8>]) -> Option<Vec<u8>> {
    iec60870_parser::parser::asdu::encode::compose_sq1_payload(chunks)
}
