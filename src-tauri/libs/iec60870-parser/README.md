# IEC 60870-5-104 Parser

高性能、零拷贝的 IEC 60870-5-104 协议解析库，专注于工业 SCADA 系统的 TCP 报文解析、验证与转换。

## 特性

- **流式解码**：`StreamDecoder` 支持持续从 TCP 字节流中提取完整帧，自动处理分包与缓冲
- **零拷贝解析**：`FrameView` 直接引用原始字节，避免不必要的内存分配
- **类型支持**：内置 `AsduConverter` 覆盖常见 ASDU 类型；未覆盖的类型以 `AsduFrame::Raw` 形式暴露，便于自定义转换器扩展
- **类型安全**：`TypeId` 枚举化设计，编译期 `TypeDescriptor` 查表，消除魔数
- **分层错误处理**：`ParseError`、`ValidationError`、`StreamError`、`ConvertError` 精确区分错误场景
- **语义验证**：内置 `FrameValidator`（可组合 `FrameRule`），支持按需插拔扩展规则
- **可选功能**：
  - `simd`: 使用 SIMD 加速帧起始定位
  - `link-manager`: 链路层状态机，支持窗口管理和确认机制
  - `pcap`: 离线 PCAP 文件解析工具
  - `serde`: 序列化支持

## 快速开始

### 安装

```toml
[dependencies]
iec60870-parser = "0.1"

# 可选特性
iec60870-parser = { version = "0.1", features = ["simd"] }
```

### 基础示例

```rust
use iec60870_parser::{
    StreamDecoder,
    stream::BytesStream,
    types::{AsduConverter, AsduFrame},
    validator::FrameValidator,
};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // 创建流式解码器
    let mut decoder = StreamDecoder::new(BytesStream::default());
    let converter = AsduConverter;
    let validator = FrameValidator::with_builtin_rules();

    // 模拟接收 TCP 数据（实际使用中从网络读取）
    let tcp_data = vec![/* 原始字节 */];

    decoder.push(&tcp_data);

    // 循环处理所有完整的帧
    while let Some(frame) = decoder.poll_frame()? {
        // 语义验证
        validator.validate(&frame)?;

        // 转换为类型化的 ASDU
        let asdu = converter.convert(&frame)?;
        let header = asdu.header();
        println!(
            "TI={}, VSQ=0x{:02X}, COT={}, CA={}",
            header.type_id.raw(),
            header.vsq,
            header.cot,
            header.common_addr
        );
        match asdu {
            AsduFrame::Measurements(set) => {
                println!("Measurements: {} items", set.items.len());
            }
            AsduFrame::Commands(set) => {
                println!("Commands: {} items", set.items.len());
            }
            AsduFrame::Raw(raw) => {
                println!("Raw payload: {} bytes", raw.payload.len());
            }
            other => {
                println!("ASDU={other:#?}");
            }
        };
    }

    Ok(())
}
```

## 核心架构

```
┌─────────────┐
│ TCP Stream  │
└──────┬──────┘
       │ 字节流
       ▼
┌─────────────┐
│ ByteStream  │  持续缓冲输入
└──────┬──────┘
       │
       ▼
┌─────────────┐
│StreamDecoder│  提取完整帧
└──────┬──────┘
       │ FrameView
       ▼
┌─────────────┐
│   Parser    │  解析 APCI/ASDU
└──────┬──────┘
       │
       ▼
┌─────────────┐
│  Validator  │  语义检查（可选）
└──────┬──────┘
       │
       ▼
┌─────────────┐
│  Converter  │  转换为领域对象
└─────────────┘
```

## 功能模块

| 模块 | 说明 |
|------|------|
| `stream` | 字节流抽象（`ByteStream` trait 和 `BytesStream` 实现） |
| `decoder` | 流式解码器（`StreamDecoder`），支持滑窗和容量控制 |
| `parser::apci` | APCI 层解析（帧类型、序号、控制域） |
| `parser::asdu` | ASDU 层解析（类型标识、信息体、时间戳） |
| `types` | 类型系统（`TypeId`、`TypeDescriptor`、`TypeConverter`） |
| `validator` | 语义验证规则（`FrameRule`、COT 检查、VSQ 校验） |
| `error` | 分层错误类型（`ParseError`、`ValidationError` 等） |
| `link` | (feature) 链路层状态机和确认管理 |

## 错误处理

库采用分层错误设计，清晰区分不同故障场景：

- **`ParseError`**: 二进制格式错误（帧起始无效、长度不足、字段非法）
  - `Incomplete`: 缓冲区数据不足
  - `InvalidStart`: 起始字节错误
  - `InvalidApci`: APCI 控制域非法
  - `InvalidAsduHeader`: ASDU 头部错误
  - `InvalidInfoObject`: 信息体解析失败
  - `UnsupportedTypeId`: 不支持的类型标识

- **`ValidationError`**: 语义规则违反（VSQ、COT、质量字段）
  - `InvalidVsq`: 信息体数量非法
  - `InvalidQuality`: 质量标志位非法
  - `InvalidCause`: 传输原因不合规

- **`StreamError`**: 流处理错误（缓冲区溢出、IO 异常）

- **`ConvertError`**: 类型转换失败（不支持的 ASDU 类型）

每个错误都带有详细的上下文信息（偏移量、TypeId、IOA、entry_index 等），便于诊断和日志记录。

## 进阶主题

### 自定义验证规则

```rust
use iec60870_parser::{FrameRule, ValidationError, FrameView};

struct CustomRule;

impl FrameRule for CustomRule {
    fn validate(&self, frame: &FrameView) -> Result<(), ValidationError> {
        // 自定义验证逻辑
        Ok(())
    }
}
```

### 链路层管理 (feature = "link-manager")

```rust
use std::time::Duration;
use iec60870_parser::link::{AckAction, DefaultLinkValidator, LinkConfig, LinkValidator};

let mut validator = DefaultLinkValidator::with_config(LinkConfig {
    k: 12,
    w: 8,
    ..LinkConfig::default()
});

// 收到对端 APCI 后喂给状态机
match validator.on_frame(&apci)? {
    AckAction::SendStartDtCon => { /* 发送 STARTDT_CON */ }
    AckAction::SendStopDtCon => { /* 发送 STOPDT_CON */ }
    AckAction::SendTestFrCon => { /* 发送 TESTFR_CON */ }
    AckAction::SendAck { ns } => { /* 发送 S 帧确认 N(R)=ns */ }
    AckAction::RequestResync => { /* 请求重建链路 */ }
    AckAction::None => {}
}

// 本端发送 I 帧前先登记，获取应使用的 N(S)
let ns = validator.on_local_i_sent()?;
// send_i_frame(ns, nr, asdu);

// 周期性心跳（驱动 T1/T2/T3）
if let Some(action) = validator.tick(Duration::from_millis(100))? {
    // 根据 action 发送对应控制帧
}
```

### 性能优化

- 启用 `simd` feature 可加速帧起始字节扫描（约 2-3x 性能提升）
- 使用 `with_max_frame_length()` 限制帧大小，防止恶意大帧攻击
- 考虑批量处理 `poll_frame()` 返回的帧以减少函数调用开销

## 文档与示例

- [架构设计](docs/ARCHITECTURE.md)：整体架构和设计决策
- [错误处理分析](docs/error_handling_analysis.md)：错误分层设计详解
- [TypeId 使用指南](docs/type_id_usage.md)：类型标识系统说明
- [开发指南](docs/DEVELOPMENT.md)：贡献者指南
- `examples/`: 可运行示例代码
  - `cargo run --example basic`: 基础解析示例
  - `cargo run --example file_transfer`: 文件传输类型演示（展示 Display trait 格式化输出）

## 开发与测试

```bash
# 运行所有测试
cargo test

# 生成文档
cargo doc --open

# 运行示例
cargo run --example basic

# PCAP 解析工具
cargo run --bin pcap_dump --features pcap -- data/sample.pcap

# 代码检查
cargo clippy -- -D warnings
cargo fmt --check
```

## 许可证

Apache License 2.0。详见仓库根目录的 `LICENSE` 和 `NOTICE`。

## 贡献

欢迎提交 Issue 和 Pull Request！请先阅读 [docs/DEVELOPMENT.md](docs/DEVELOPMENT.md) 了解开发规范。
