use std::ops::Range;

use bytes::Bytes;
use smallvec::SmallVec;

use crate::{
    FrameType,
    error::{ConvertError, ParseError},
    parser::{
        apci::ApciHeader,
        asdu::{self, Asdu, AsduHeader, TypeId},
    },
};

/// 高层帧视图，保持零拷贝：`raw` 保存整体 bytes，字段仅为引用。
const APCI_LEN: usize = 6;
const ASDU_HEADER_LEN: usize = 6;

#[derive(Clone, Debug)]
pub struct FrameView {
    raw: Bytes,
    pub apci: ApciHeader,
    asdu: Option<AsduSnapshot>,
}

#[derive(Clone, Debug)]
struct AsduSnapshot {
    header: AsduHeader,
    payload: Range<usize>,
}

impl FrameView {
    pub fn new(raw: Bytes, apci: ApciHeader, asdu_header: AsduHeader) -> Self {
        let header_offset = (APCI_LEN + ASDU_HEADER_LEN).min(raw.len());
        let payload_start = header_offset;
        let payload_range = payload_start..raw.len();
        let snapshot = AsduSnapshot { header: asdu_header, payload: payload_range };
        Self { raw, apci, asdu: Some(snapshot) }
    }

    pub fn new_without_asdu(raw: Bytes, apci: ApciHeader) -> Self {
        Self { raw, apci, asdu: None }
    }

    /// 获取底层原始字节。
    pub fn raw(&self) -> &Bytes {
        &self.raw
    }

    /// 返回 ASDU 头部（仅 I 帧）。
    pub fn asdu_header(&self) -> Option<AsduHeader> {
        self.asdu.as_ref().map(|snapshot| snapshot.header)
    }

    /// 生成 ASDU 视图（头部 + payload 切片）。
    pub fn asdu(&self) -> Option<Asdu<'_>> {
        self.asdu.as_ref().map(|snapshot| {
            let payload = &self.raw[snapshot.payload.clone()];
            Asdu::new(snapshot.header, payload)
        })
    }
}

/// 复制型帧，用于跨线程或延迟处理。
#[derive(Clone, Debug)]
pub struct Frame {
    pub apci: ApciHeader,
    pub asdu_header: AsduHeader,
    pub payload: Vec<u8>,
}

impl Frame {
    /// 从 [`FrameView`] 构造拥有所有权的 [`Frame`]。
    ///
    /// 仅对 I 帧有效；若输入为 S/U 帧（没有 ASDU），返回 [`ConvertError::UnsupportedFrame`]。
    pub fn try_from_view(view: &FrameView) -> Result<Self, ConvertError> {
        let asdu = view.asdu().ok_or(ConvertError::UnsupportedFrame(view.apci.frame_type))?;
        Ok(Self { apci: view.apci, asdu_header: asdu.header, payload: asdu.payload.to_vec() })
    }
}

impl From<&FrameView> for Frame {
    fn from(view: &FrameView) -> Self {
        let asdu = view.asdu().expect("Frame::from requires I-frame");
        Self { apci: view.apci, asdu_header: asdu.header, payload: asdu.payload.to_vec() }
    }
}

/// 类型转换器：将 `FrameView` 转换为领域结构。
pub trait TypeConverter {
    type Output<'a>;
    /// 将 `FrameView` 转成领域结果，出错时返回 [`ConvertError`].
    fn convert<'a>(&self, frame: &'a FrameView) -> Result<Self::Output<'a>, ConvertError>;
}

/// 默认不做转换，仅返回 `Frame`。
pub struct IdentityConverter;

impl TypeConverter for IdentityConverter {
    type Output<'a> = Frame;

    fn convert<'a>(&self, frame: &'a FrameView) -> Result<Self::Output<'a>, ConvertError> {
        Frame::try_from_view(frame)
    }
}

/// 原始 ASDU 视图，包含头部与 payload 切片。
#[derive(Debug, Clone, Copy)]
pub struct RawAsdu<'a> {
    pub header: AsduHeader,
    pub payload: &'a [u8],
}

/// 信息体的通用元数据。
#[derive(Debug, Clone, Copy, Default)]
pub struct InfoObjectRef<'a> {
    pub ioa: Option<u32>,
    pub timestamp: Option<&'a [u8]>,
}

/// 解析完成的 ASDU 枚举。
#[derive(Debug, Clone)]
pub enum AsduFrame<'a> {
    Measurements(asdu::measurements::MeasurementSet<'a>),
    Commands(asdu::commands::CommandSet<'a>),
    TestCommand(asdu::test_command::TestCommandFrame<'a>),
    Interrogation(asdu::interrogation::InterrogationFrame),
    Initialization(asdu::initialization::InitializationFrame),
    Protection(asdu::events::ProtectionEvents<'a>),
    Parameters(asdu::parameters::ParameterSet<'a>),
    File(asdu::file_transfer::FileTransferSet<'a>),
    Security(asdu::security::SecurityFrame<'a>),
    Raw(RawAsdu<'a>),
}

/// 完整 APDU 结构（APCI + 已解析 ASDU）。
#[derive(Debug, Clone)]
pub struct Apdu<'a> {
    pub apci: ApciHeader,
    /// I 帧包含解析后的 ASDU，其余帧类型为 `None`。
    pub asdu: Option<AsduFrame<'a>>,
}

impl FrameView {
    /// 解析当前帧为 APDU：I 帧包含解析后的 ASDU，S/U 帧仅携带 APCI。
    pub fn to_apdu<'a, C>(&'a self, converter: &C) -> Result<Apdu<'a>, ConvertError>
    where C: TypeConverter<Output<'a> = AsduFrame<'a>> {
        let asdu = if self.apci.frame_type == FrameType::I { Some(converter.convert(self)?) } else { None };
        Ok(Apdu { apci: self.apci, asdu })
    }
}

impl<'a> AsduFrame<'a> {
    /// 返回该 ASDU 的头部（TI/VSQ/COT/CA）。
    pub fn header(&self) -> AsduHeader {
        match self {
            AsduFrame::Measurements(set) => set.header,
            AsduFrame::Commands(set) => set.header,
            AsduFrame::TestCommand(frame) => frame.header,
            AsduFrame::Interrogation(frame) => frame.header,
            AsduFrame::Initialization(frame) => frame.header,
            AsduFrame::Protection(events) => events.header,
            AsduFrame::Parameters(set) => set.header,
            AsduFrame::File(set) => set.header,
            AsduFrame::Security(sec) => sec.header(),
            AsduFrame::Raw(raw) => raw.header,
        }
    }

    /// 便捷获取 Type ID。
    pub fn type_id(&self) -> TypeId {
        self.header().type_id
    }

    /// 遍历信息体元数据，便于统一获取 IOA/时间戳。
    pub fn info_objects(&self) -> SmallVec<[InfoObjectRef<'a>; 4]> {
        let mut refs = SmallVec::new();
        match self {
            AsduFrame::Measurements(set) => {
                refs.extend(set.items.iter().map(|m| InfoObjectRef { ioa: Some(m.ioa), timestamp: m.timestamp }));
            }
            AsduFrame::Commands(set) => {
                refs.extend(set.items.iter().map(|c| InfoObjectRef { ioa: Some(c.ioa), timestamp: c.timestamp }));
            }
            AsduFrame::TestCommand(frame) => {
                refs.extend(
                    frame.items.iter().map(|cmd| InfoObjectRef { ioa: Some(cmd.ioa), timestamp: cmd.timestamp }),
                );
            }
            AsduFrame::Interrogation(frame) => {
                refs.extend(frame.items.iter().map(|req| InfoObjectRef { ioa: Some(req.ioa), timestamp: None }));
            }
            AsduFrame::Initialization(_) => {}
            AsduFrame::Protection(events) => {
                refs.extend(
                    events.items.iter().map(|event| InfoObjectRef { ioa: Some(event.ioa), timestamp: event.timestamp }),
                );
            }
            AsduFrame::Parameters(set) => {
                refs.extend(set.items.iter().map(|param| InfoObjectRef { ioa: Some(param.ioa), timestamp: None }));
            }
            _ => {}
        }
        refs
    }
}

/// 将 `FrameView` 转为 `AsduFrame` 的转换器。
pub struct AsduConverter;

impl TypeConverter for AsduConverter {
    type Output<'a> = AsduFrame<'a>;

    fn convert<'a>(&self, frame: &'a FrameView) -> Result<Self::Output<'a>, ConvertError> {
        let asdu = frame.asdu().ok_or(ConvertError::UnsupportedFrame(frame.apci.frame_type))?;
        let payload = asdu.payload;
        let ti = asdu.header.type_id.raw();
        let result = match ti {
            1..=16 | 20 | 21 | 30..=37 => AsduFrame::Measurements(map_parse(
                asdu::measurements::parse_measurements(asdu.header, payload),
                "measurements",
            )?),
            17..=19 | 38..=40 => AsduFrame::Protection(map_parse(
                asdu::events::parse_protection_events(asdu.header, payload),
                "protection",
            )?),
            45..=64 | 102 | 103 | 105 => {
                AsduFrame::Commands(map_parse(asdu::commands::parse_commands(asdu.header, payload), "commands")?)
            }
            104 | 107 => AsduFrame::TestCommand(map_parse(
                asdu::test_command::parse_test_command(asdu.header, payload),
                "test_command",
            )?),
            100 | 101 => AsduFrame::Interrogation(map_parse(
                asdu::interrogation::parse_interrogation(asdu.header, payload),
                "interrogation",
            )?),
            70 => AsduFrame::Initialization(map_parse(
                asdu::initialization::parse_initialization(asdu.header, payload),
                "initialization",
            )?),
            110..=113 => AsduFrame::Parameters(map_parse(
                asdu::parameters::parse_parameters(asdu.header, payload),
                "parameters",
            )?),
            120..=127 => {
                AsduFrame::File(map_parse(asdu::file_transfer::parse_file_transfer(asdu.header, payload), "file")?)
            }
            81..=95 => {
                AsduFrame::Security(map_parse(asdu::security::parse_security(asdu.header, payload), "security")?)
            }
            _ => AsduFrame::Raw(RawAsdu { header: asdu.header, payload }),
        };
        Ok(result)
    }
}

fn map_parse<'a, T>(res: Result<T, ParseError>, ctx: &'static str) -> Result<T, ConvertError> {
    res.map_err(|err| ConvertError::InvalidField { ctx, source: err })
}
