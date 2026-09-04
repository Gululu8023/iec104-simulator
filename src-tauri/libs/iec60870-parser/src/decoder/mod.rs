use bytes::Bytes;

use crate::{
    error::{ParseError, StreamError},
    parser::{apci, asdu::parse_asdu_header},
    stream::ByteStream,
    types::FrameView,
};

/// 流式帧解码器。
pub struct StreamDecoder<S: ByteStream> {
    stream: S,
    max_frame_length: usize,
}

impl<S: ByteStream> StreamDecoder<S> {
    pub fn new(stream: S) -> Self {
        Self { stream, max_frame_length: 255 }
    }

    /// 设置帧长上限。
    pub fn with_max_frame_length(mut self, max_len: usize) -> Self {
        self.max_frame_length = max_len;
        self
    }

    /// 推入一段原始字节。
    pub fn push(&mut self, chunk: &[u8]) {
        self.stream.push(chunk);
    }

    /// 轮询下一帧；返回 `Ok(None)` 表示尚未累积完整帧。
    pub fn poll_frame(&mut self) -> Result<Option<FrameView>, StreamError> {
        loop {
            let data = self.stream.peek();
            if data.len() < 2 {
                return Ok(None);
            }

            let start = match find_start(data) {
                Some(pos) => pos,
                None => {
                    // 丢弃除最后 1 字节外的垃圾。
                    let drop_len = data.len().saturating_sub(1);
                    if drop_len > 0 {
                        self.stream.consume(drop_len);
                    }
                    return Ok(None);
                }
            };

            // 对齐到 0x68 起始字节。
            if start > 0 {
                self.stream.consume(start);
                continue;
            }

            // 现在 data[0]==0x68 且 data.len()>=2
            let length = data[1] as usize;
            // IEC104: length 表示后续字节数，最少包含 4 字节控制域。
            if length < 4 {
                // 丢弃当前起始字节，继续尝试重同步。
                self.stream.consume(1);
                continue;
            }

            let total_len = 2 + length;
            if total_len > self.max_frame_length {
                // 丢弃当前起始字节，避免后续 poll 卡死在同一位置。
                self.stream.consume(1);
                return Err(StreamError::BufferOverflow { expected: total_len, available: self.max_frame_length });
            }

            if data.len() < total_len {
                return Ok(None);
            }

            let raw = self.stream.split_to(total_len);
            match build_frame_view(raw) {
                Ok(frame) => {
                    return Ok(Some(frame));
                }
                Err(err) => {
                    return Err(err);
                }
            }
        }
    }
}

/// SIMD加速的起始字节搜索（使用memchr，在支持的CPU上自动启用SIMD）
#[cfg(feature = "simd")]
fn find_start(data: &[u8]) -> Option<usize> {
    memchr::memchr(0x68, data)
}

/// 标准的起始字节搜索（默认实现）
#[cfg(not(feature = "simd"))]
fn find_start(data: &[u8]) -> Option<usize> {
    data.iter().position(|&b| b == 0x68)
}

fn build_frame_view(raw: Bytes) -> Result<FrameView, StreamError> {
    if raw.len() < 6 {
        return Err(ParseError::Incomplete.into());
    }

    match apci::parse_apci(&raw[..6]) {
        Ok(apci_header) => match apci_header.frame_type {
            crate::parser::apci::FrameType::I => {
                let asdu_bytes = &raw[6..];
                let asdu_header = parse_asdu_header(asdu_bytes)?;
                Ok(FrameView::new(raw, apci_header, asdu_header))
            }
            _ => Ok(FrameView::new_without_asdu(raw, apci_header)),
        },
        Err(err) => Err(StreamError::from(err)),
    }
}
