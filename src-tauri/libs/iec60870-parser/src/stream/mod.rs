use bytes::{Buf, Bytes, BytesMut};

/// 通用零拷贝字节流接口。
pub trait ByteStream {
    /// 追加新的数据片段。
    fn push(&mut self, chunk: &[u8]);
    /// 查看当前可用数据（不会移动读指针）。
    fn peek(&self) -> &[u8];
    /// 丢弃前 `len` 字节。
    fn consume(&mut self, len: usize);
    /// 拆分前 `len` 字节为独立的 `Bytes`，零拷贝共享底层存储。
    fn split_to(&mut self, len: usize) -> Bytes;
    /// 当前缓冲区容量（字节数）。
    fn capacity(&self) -> usize;
}

/// 默认的字节流实现，基于 `BytesMut`。
#[derive(Debug, Default)]
pub struct BytesStream {
    buffer: BytesMut,
}

impl BytesStream {
    pub fn with_capacity(cap: usize) -> Self {
        Self { buffer: BytesMut::with_capacity(cap) }
    }
}

impl ByteStream for BytesStream {
    fn push(&mut self, chunk: &[u8]) {
        self.buffer.extend_from_slice(chunk);
    }

    fn peek(&self) -> &[u8] {
        &self.buffer
    }

    fn consume(&mut self, len: usize) {
        let len = len.min(self.buffer.len());
        self.buffer.advance(len);
    }

    fn split_to(&mut self, len: usize) -> Bytes {
        self.buffer.split_to(len).freeze()
    }

    fn capacity(&self) -> usize {
        self.buffer.capacity()
    }
}
