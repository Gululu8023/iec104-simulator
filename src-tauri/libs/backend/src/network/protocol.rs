//! IEC104 protocol runtime handling.

use std::collections::VecDeque;

use iec60870_parser::{
    StreamDecoder,
    parser::{
        apci::{FrameType, UControl},
        asdu::{
            commands::{CommandDetail, CommandQualifier},
            file_transfer,
            measurements::{MeasurementDescriptor, MeasurementValue},
            timestamp::Timestamp,
        },
    },
    stream::BytesStream,
    types::{AsduConverter, AsduFrame, TypeConverter},
    validator::{FrameRule, FrameValidator},
};
use serde::{Deserialize, Serialize};

use super::NetworkError;

/// 已解析的 ASDU（应用服务数据单元），运行时安全的自有数据结构。
#[derive(Debug, Clone)]
pub struct DecodedAsdu {
    /// 类型标识（Type ID），标识 ASDU 的数据类型（如单点信息、遥控命令等）
    pub type_id: u8,
    /// 可变结构限定词（VSQ），低 7 位表示信息对象数量，最高位表示是否为顺序地址
    pub vsq: u8,
    /// 传送原因（Cause of Transmission），包含原因码、P/N 位、T 位和源地址
    pub cause: u16,
    /// 公共地址（Common Address），标识 ASDU 的目标站点
    pub common_address: u16,
    /// ASDU 主体内容，根据 type_id 解析为不同的数据类型
    pub body: DecodedAsduBody,
    /// 原始 ASDU 载荷字节，用于调试和重传
    pub raw_payload: Vec<u8>,
}

/// ASDU 主体内容的解析结果，根据类型标识分类。
#[derive(Debug, Clone)]
pub enum DecodedAsduBody {
    /// 测量值（监视方向），包含单点、双点、遥测等信息对象
    Measurements(Vec<DecodedMeasurement>),
    /// 控制命令（控制方向），包含遥控、遥调等命令对象
    Commands(Vec<DecodedCommand>),
    /// 召唤命令（总召唤、电能召唤等）
    Interrogations(Vec<DecodedInterrogation>),
    /// 文件传输相关报文（文件就绪、段传输等）
    FileTransfers(Vec<DecodedFileTransferRecord>),
    /// 未解析的原始数据（不支持的类型或解析失败）
    Raw,
}

/// 解析后的测量值信息对象。
#[derive(Debug, Clone)]
pub struct DecodedMeasurement {
    /// 信息对象地址（Information Object Address），24 位地址
    pub ioa: u32,
    /// 测量值数据（单点、双点、遥测等）
    pub value: DecodedMeasurementValue,
    /// 品质描述词原始字节（包含 IV/NT/SB/BL 等标志位）
    pub quality: u8,
    /// 通用品质标志（所有测量类型共有的 4 个标志位）
    pub quality_common: DecodedQualityCommon,
    /// 类型特定的品质详情（SIQ/DIQ/QDS/BCR 等）
    pub quality_detail: DecodedMeasurementQualityDetail,
    /// 原始 CP24/CP56 字段及有效位语义
    pub timestamp: Option<DecodedMeasurementTimestamp>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DecodedMeasurementTimestamp {
    Cp24 {
        raw: Vec<u8>,
        milliseconds: u16,
        minutes: u8,
        invalid: bool,
    },
    Cp56 {
        raw: Vec<u8>,
        milliseconds: u16,
        minutes: u8,
        hours: u8,
        day: u8,
        month: u8,
        year: u16,
        weekday: u8,
        summer_time: bool,
        invalid: bool,
    },
}

/// 通用品质标志（IEC104 协议中所有测量类型共有的 4 个品质位）。
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct DecodedQualityCommon {
    /// IV (Invalid): 无效标志，true 表示测量值无效
    pub iv: bool,
    /// NT (Not Topical): 非当前值标志，true 表示值非最新
    pub nt: bool,
    /// SB (Substituted): 替代标志，true 表示值被人工替代
    pub sb: bool,
    /// BL (Blocked): 闭锁标志，true 表示值被闭锁
    pub bl: bool,
}

impl DecodedQualityCommon {
    /// 将品质标志转换为 u8 字节（高 4 位：IV/NT/SB/BL）。
    pub const fn as_u8(self) -> u8 {
        (if self.iv { 0x80 } else { 0 })
            | (if self.nt { 0x40 } else { 0 })
            | (if self.sb { 0x20 } else { 0 })
            | (if self.bl { 0x10 } else { 0 })
    }
}

/// 测量值类型特定的品质描述词详情。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DecodedMeasurementQualityDetail {
    /// SIQ (Single-point Information with Quality): 单点信息品质
    Siq { raw: u8, spi: bool, iv: bool, nt: bool, sb: bool, bl: bool },
    /// DIQ (Double-point Information with Quality): 双点信息品质
    Diq { raw: u8, dpi: u8, iv: bool, nt: bool, sb: bool, bl: bool },
    /// QDS (Quality Descriptor for measured values): 测量值品质描述词
    Qds {
        raw: u8,
        ov: bool,
        iv: bool,
        nt: bool,
        sb: bool,
        bl: bool,
        transient: Option<bool>,
        scd_status: Option<u16>,
        scd_change: Option<u16>,
    },
    /// BCR (Binary Counter Reading): 二进制计数器读数品质
    Bcr { raw: u8, sq: u8, cy: bool, ca: bool, iv: bool },
    /// 无品质描述词（某些类型不携带品质信息）
    None,
}

impl DecodedMeasurementQualityDetail {
    const fn raw(&self) -> Option<u8> {
        match self {
            Self::Siq { raw, .. } | Self::Diq { raw, .. } | Self::Qds { raw, .. } | Self::Bcr { raw, .. } => Some(*raw),
            Self::None => None,
        }
    }
}

/// 测量值数据的具体类型和值。
#[derive(Debug, Clone)]
pub enum DecodedMeasurementValue {
    /// 单点信息（M_SP_*）：开关量，true=合/ON，false=分/OFF
    Single(bool),
    /// 双点信息（M_DP_*）：双位开关量，0=中间态，1=分/OFF，2=合/ON，3=故障
    Double(u8),
    /// 步位信息（M_ST_*）：档位值和瞬变标志
    StepPosition { value: i8, transient: bool },
    /// 比特串（M_BO_*）：32 位状态字
    BitString(u32),
    /// 归一化测量值（M_ME_NA/TA/TD）：-1.0 到 +1.0 范围的定点数
    Normalized(i16),
    /// 标度化测量值（M_ME_NB/TB/TE）：整数测量值
    Scaled(i16),
    /// 短浮点测量值（M_ME_NC/TC/TF）：IEEE 754 单精度浮点数
    Float(f32),
    /// 累计量（M_IT_*）：电能脉冲计数等
    Integrated(i32),
    /// 成组单点（M_PS_NA）：32 位打包的单点状态
    PackedStatus { status: u16, change: u16 },
    /// 未解析的原始测量值
    Raw,
}

/// 解析后的控制命令信息对象。
#[derive(Debug, Clone)]
pub struct DecodedCommand {
    /// 信息对象地址（控制点地址）
    pub ioa: u32,
    /// 命令详情（单控、双控、遥调等）
    pub detail: DecodedCommandDetail,
    /// 命令限定词（QOC/QOS/QRP 等）
    pub qualifier: CommandQualifier,
    /// 时间戳（带时标的命令类型）
    pub timestamp: Option<Vec<u8>>,
}

/// 控制命令的具体类型和参数。
#[derive(Debug, Clone)]
pub enum DecodedCommandDetail {
    /// 单点控制（C_SC_*）：分/合命令
    Single { value: bool, select: bool },
    /// 双点控制（C_DC_*）：分/合命令（双位编码）
    Double { value: u8, select: bool },
    /// 升降命令（C_RC_*）：步进调节
    Step { position: u8, select: bool },
    /// 归一化设点命令（C_SE_NA/TA）
    SetPointNormalized(i16),
    /// 标度化设点命令（C_SE_NB/TB）
    SetPointScaled(i16),
    /// 短浮点设点命令（C_SE_NC/TC）
    SetPointFloat(f32),
    /// 比特串命令（C_BO_*）
    BitString(u32),
    /// 读命令（C_RD_NA）
    Read,
    /// 时钟同步命令（C_CS_NA）
    ClockSynchronization(iec60870_parser::parser::asdu::timestamp::CP56Time2a),
    /// 测试命令（C_TS_NA/TA）
    TestPattern(u16),
    /// 复位进程命令（C_RP_NA）
    ResetProcess,
    /// 未解析的原始命令数据
    Raw(Vec<u8>),
}

/// 召唤命令信息对象。
#[derive(Debug, Clone)]
pub struct DecodedInterrogation {
    /// 信息对象地址（通常为 0 表示全站召唤）
    pub ioa: u32,
    /// 召唤限定词（QOI/QCC 等）
    pub qualifier: u8,
}

/// 文件传输协议的信息对象记录。
#[derive(Debug, Clone)]
pub enum DecodedFileTransferRecord {
    /// F_FR_NA_1 (TI=120): 文件就绪
    FileReady { ioa: u32, file_name: u16, qualifier: u8, length: u32 },
    /// F_SR_NA_1 (TI=121): 段就绪
    SectionReady { ioa: u32, file_name: u16, qualifier: u8, section: u8, length: u32 },
    /// F_SC_NA_1 (TI=122): 召唤文件/段
    SelectCall { ioa: u32, file_name: u16, qualifier: u8, section: u8 },
    /// F_LS_NA_1 (TI=123): 最后的段/节
    LastSection { ioa: u32, file_name: u16, qualifier: u8, last_section_number: u8, last_segment_number: u8 },
    /// F_AF_NA_1 (TI=124): 确认文件/段
    AckSection { ioa: u32, file_name: u16, qualifier: u8, section: u8 },
    /// F_SG_NA_1 (TI=125): 段
    Segment { ioa: u32, file_name: u16, section: u8, los: u8, data: Vec<u8> },
    /// F_DR_TA_1 (TI=126): 目录
    Directory {
        ioa: u32,
        file_name: u16,
        qualifier: u8,
        section: u16,
        length: u32,
        last_section: u16,
        last_segment: u16,
        status_raw: u8,
        is_file: bool,
        is_last_file_of_directory: bool,
        timestamp: Option<Vec<u8>>,
    },
    /// F_SC_NB_1 (TI=127): 查询日志
    QueryLog { ioa: u32, file_name: u16, range_start_time: Vec<u8>, range_end_time: Vec<u8> },
    /// 未解析的原始文件传输数据
    Raw(Vec<u8>),
}

/// IEC104 协议状态机。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum ProtocolState {
    /// 未连接（TCP 断开或初始状态）
    Disconnected,
    /// 连接中（已发送 STARTDT，等待 STARTDT CON）
    Connecting,
    /// 已连接（TCP 已建立，但数据传输未启动）
    Connected,
    /// 数据传输中（STARTDT 握手完成，可以收发 I 帧）
    DataTransfer,
    /// 错误状态（协议异常，附带错误描述）
    Error(String),
}

/// IEC104 协议运行时处理器。
pub struct Iec104Protocol {
    /// 当前协议状态
    state: ProtocolState,
    /// 发送序号 V(S)，范围 0-32767
    send_sequence: u16,
    /// 接收序号 V(R)，范围 0-32767
    receive_sequence: u16,
    /// 对端确认序号 N(R)，用于计算未确认帧数
    ack_sequence: u16,
    /// 发送缓冲区（未确认的 I 帧）
    send_buffer: VecDeque<Vec<u8>>,
    /// 流式解码器（处理 TCP 字节流到帧的解析）
    decoder: StreamDecoder<BytesStream>,
    /// ASDU 转换器（将帧转换为 ASDU 结构）
    converter: AsduConverter,
    /// 帧验证器（语义校验）
    validator: FrameValidator,
    /// 未确认帧计数（发送窗口占用）
    unconfirmed_count: u16,
    /// 最大未确认帧数（k 参数，默认 12）
    max_unconfirmed: u16,
    /// 最大 ASDU 长度（默认 249 字节）
    max_asdu_len: usize,
}

impl Iec104Protocol {
    /// 创建新的 IEC104 协议处理器实例（初始状态为 Disconnected）。
    pub fn new() -> Self {
        Self {
            state: ProtocolState::Disconnected,
            send_sequence: 0,
            receive_sequence: 0,
            ack_sequence: 0,
            send_buffer: VecDeque::new(),
            decoder: StreamDecoder::new(BytesStream::default()).with_max_frame_length(255),
            converter: AsduConverter,
            validator: FrameValidator::with_builtin_rules(),
            unconfirmed_count: 0,
            max_unconfirmed: 12,
            max_asdu_len: 249,
        }
    }

    /// 获取对端确认序号 N(R)。
    pub fn ack_sequence(&self) -> u16 {
        self.ack_sequence
    }

    /// 获取当前未确认帧数（发送窗口占用）。
    pub fn unconfirmed_count(&self) -> u16 {
        self.unconfirmed_count
    }

    /// 获取所有未确认帧的副本（用于重传或调试）。
    pub fn unconfirmed_frames(&self) -> Vec<Vec<u8>> {
        self.send_buffer.iter().cloned().collect()
    }

    /// 获取当前协议状态。
    pub fn state(&self) -> &ProtocolState {
        &self.state
    }

    /// 设置协议状态（仅用于测试或特殊场景）。
    pub fn set_state(&mut self, state: ProtocolState) {
        self.state = state;
    }

    /// 获取最大未确认帧数（k 参数）。
    pub fn max_unconfirmed(&self) -> u16 {
        self.max_unconfirmed
    }

    /// 设置最大未确认帧数（k 参数），范围 1-32767。
    pub fn set_max_unconfirmed(&mut self, value: u16) {
        let value = value.clamp(1, 0x7FFF);
        self.max_unconfirmed = value;
    }

    /// 设置最大 ASDU 长度，范围 4-249 字节。
    pub fn set_max_asdu_len(&mut self, value: usize) {
        self.max_asdu_len = value.clamp(4, 249);
    }

    /// 开始数据传输（发送 STARTDT ACT），返回 U 帧字节。
    pub fn begin_data_transfer(&mut self) -> Result<Vec<u8>, NetworkError> {
        if !matches!(self.state, ProtocolState::Disconnected | ProtocolState::Connected) {
            return Err(NetworkError::Protocol("Invalid state for connect".to_string()));
        }

        self.state = ProtocolState::Connecting;
        self.reset_link_counters();

        self.generate_startdt_act()
    }

    /// 停止数据传输（发送 STOPDT ACT），返回 U 帧字节。
    pub fn begin_stop_data_transfer(&mut self) -> Result<Vec<u8>, NetworkError> {
        if !matches!(self.state, ProtocolState::Connecting | ProtocolState::Connected | ProtocolState::DataTransfer) {
            return Err(NetworkError::Protocol("Invalid state for stop data transfer".to_string()));
        }

        // STOPDT 仅结束业务链路，不代表 TCP 断开。
        self.state = ProtocolState::Connected;
        Ok(self.create_u_frame(0x13))
    }

    /// 处理连接事件（等同于 begin_data_transfer）。
    pub fn handle_connect(&mut self) -> Result<Vec<u8>, NetworkError> {
        self.begin_data_transfer()
    }

    /// 处理断开事件（发送 STOPDT ACT 并切换到 Disconnected 状态）。
    pub fn handle_disconnect(&mut self) -> Result<Vec<u8>, NetworkError> {
        if matches!(self.state, ProtocolState::Connected | ProtocolState::DataTransfer) {
            self.state = ProtocolState::Disconnected;
            Ok(self.create_u_frame(0x13))
        } else {
            Err(NetworkError::Protocol("Invalid state for disconnect".to_string()))
        }
    }

    /// 发送 ASDU（封装为 I 帧），返回完整帧字节。
    ///
    /// # 错误
    ///
    /// - 状态不是 DataTransfer：返回 "Not in data transfer state"
    /// - 发送窗口已满：返回 "Send window full"
    /// - ASDU 超长：返回 "ASDU too large for IEC104 frame"
    pub fn send_asdu(&mut self, asdu_data: &[u8]) -> Result<Vec<u8>, NetworkError> {
        if self.state != ProtocolState::DataTransfer {
            return Err(NetworkError::Protocol("Not in data transfer state".to_string()));
        }

        if self.unconfirmed_count >= self.max_unconfirmed {
            return Err(NetworkError::Protocol("Send window full".to_string()));
        }

        let frame = self.create_i_frame(asdu_data)?;
        self.send_sequence = (self.send_sequence + 1) & 0x7FFF;
        self.unconfirmed_count = seq_distance(self.send_sequence, self.ack_sequence);
        self.send_buffer.push_back(frame.clone());
        Ok(frame)
    }

    /// 处理接收到的 TCP 数据（流式解析为帧并处理）。
    ///
    /// # 功能
    ///
    /// - 将字节流推入解码器
    /// - 循环提取完整帧
    /// - 根据帧类型（I/S/U）执行相应处理
    /// - 更新序号和发送窗口
    /// - 返回解析后的协议消息列表
    ///
    /// # 返回
    ///
    /// - `Ok(Vec<ProtocolMessage>)`: 解析出的协议消息（可能为空）
    /// - `Err(NetworkError)`: 解码失败或协议错误
    pub fn handle_received_data(&mut self, data: &[u8]) -> Result<Vec<ProtocolMessage>, NetworkError> {
        self.decoder.push(data);
        let mut messages = Vec::new();

        loop {
            let frame = match self.decoder.poll_frame() {
                Ok(Some(frame)) => frame,
                Ok(None) => break,
                Err(err) => {
                    return Err(NetworkError::Protocol(format!("stream decode failed: {err}")));
                }
            };

            match frame.apci.frame_type {
                FrameType::I => {
                    if self.state != ProtocolState::DataTransfer {
                        messages.push(ProtocolMessage::IgnoredFrame {
                            frame_type: "I",
                            reason: format!("drop I-frame before STARTDT (state={:?})", self.state),
                        });
                        continue;
                    }

                    let send_sequence = frame.apci.send_seq;
                    let receive_sequence = frame.apci.recv_seq;

                    self.ack_sequence = receive_sequence;
                    self.unconfirmed_count = seq_distance(self.send_sequence, self.ack_sequence);
                    self.drain_acked_frames();

                    let expected_receive_sequence = self.receive_sequence & 0x7FFF;
                    if send_sequence == expected_receive_sequence {
                        self.receive_sequence = (self.receive_sequence + 1) & 0x7FFF;
                        let raw_frame = frame.raw().to_vec();
                        let decoded_asdu = self
                            .validator
                            .validate(&frame)
                            .map_err(|err| format!("frame semantic validation failed: {err}"))
                            .and_then(|()| decode_asdu(&self.converter, &frame).map_err(|err| err.to_string()));
                        match decoded_asdu {
                            Ok(decoded_asdu) => messages.push(ProtocolMessage::IFrame {
                                send_sequence,
                                receive_sequence,
                                decoded_asdu,
                                raw_frame,
                            }),
                            Err(error) => messages.push(ProtocolMessage::IFrameDecodeError {
                                send_sequence,
                                receive_sequence,
                                error,
                                raw_frame,
                            }),
                        }
                    } else if seq_forward_distance(expected_receive_sequence, send_sequence) >= 16384 {
                        // Duplicate/old I-frame: drop it but request immediate S(V(R)).
                        messages.push(ProtocolMessage::AckRequired {
                            expected_receive_sequence,
                            got_send_sequence: send_sequence,
                        });
                    } else {
                        // Gap/jump ahead: signal frame count error to caller (link recovery needed).
                        messages.push(ProtocolMessage::FrameCountError {
                            expected_receive_sequence,
                            got_send_sequence: send_sequence,
                        });
                    }
                }
                FrameType::S => {
                    if self.state != ProtocolState::DataTransfer {
                        messages.push(ProtocolMessage::IgnoredFrame {
                            frame_type: "S",
                            reason: format!("drop S-frame before STARTDT (state={:?})", self.state),
                        });
                        continue;
                    }

                    let receive_sequence = frame.apci.recv_seq;
                    self.ack_sequence = receive_sequence;
                    self.unconfirmed_count = seq_distance(self.send_sequence, self.ack_sequence);
                    self.drain_acked_frames();

                    messages.push(ProtocolMessage::SFrame { receive_sequence, raw_frame: frame.raw().to_vec() });
                }
                FrameType::U => {
                    let control = frame
                        .apci
                        .u_control()
                        .ok_or_else(|| NetworkError::Protocol("invalid U-frame control type".to_string()))?;
                    let mapped = map_u_frame(control, &mut self.state);
                    if matches!(mapped, UFrameType::StartDtAct | UFrameType::StartDtCon) {
                        self.reset_link_counters();
                    }
                    messages.push(ProtocolMessage::UFrame { u_type: mapped, raw_frame: frame.raw().to_vec() });
                }
            }
        }

        Ok(messages)
    }

    /// 创建 I 帧（信息帧），封装 ASDU 数据。
    ///
    /// # 帧格式
    ///
    /// - 起始字节：0x68
    /// - 长度字节：ASDU 长度 + 4
    /// - 控制域 4 字节：包含发送序号 V(S) 和接收序号 V(R)
    /// - ASDU 数据
    fn create_i_frame(&self, asdu_data: &[u8]) -> Result<Vec<u8>, NetworkError> {
        if asdu_data.len() > self.max_asdu_len {
            return Err(NetworkError::Protocol("ASDU too large for IEC104 frame".to_string()));
        }

        let mut frame = Vec::with_capacity(6 + asdu_data.len());
        frame.push(0x68);
        frame.push((asdu_data.len() + 4) as u8);

        let send_seq = self.send_sequence & 0x7FFF;
        let recv_seq = self.receive_sequence & 0x7FFF;
        let control1 = (send_seq & 0x7F) << 1;
        let control2 = (send_seq >> 7) & 0xFF;
        let control3 = (recv_seq & 0x7F) << 1;
        let control4 = (recv_seq >> 7) & 0xFF;

        frame.push(control1 as u8);
        frame.push(control2 as u8);
        frame.push(control3 as u8);
        frame.push(control4 as u8);
        frame.extend_from_slice(asdu_data);
        Ok(frame)
    }

    /// 创建 S 帧（监督帧），用于确认接收。
    ///
    /// # 帧格式
    ///
    /// - 起始字节：0x68
    /// - 长度字节：0x04
    /// - 控制域：0x01 0x00（标识 S 帧）+ 接收序号 V(R)
    fn create_s_frame(&self) -> Vec<u8> {
        let mut frame = Vec::with_capacity(6);
        frame.push(0x68);
        frame.push(0x04);
        frame.push(0x01);
        frame.push(0x00);

        let recv_seq = self.receive_sequence & 0x7FFF;
        let control3 = (recv_seq & 0x7F) << 1;
        let control4 = (recv_seq >> 7) & 0xFF;
        frame.push(control3 as u8);
        frame.push(control4 as u8);
        frame
    }

    /// 创建 U 帧（未编号控制帧），用于链路控制。
    ///
    /// # 参数
    ///
    /// - `control`: U 帧控制字节（如 0x07=STARTDT ACT, 0x0B=STARTDT CON）
    fn create_u_frame(&self, control: u8) -> Vec<u8> {
        vec![0x68, 0x04, control, 0x00, 0x00, 0x00]
    }

    /// 生成 S 帧确认（携带当前接收序号）。
    pub fn generate_ack(&self) -> Result<Vec<u8>, NetworkError> {
        Ok(self.create_s_frame())
    }

    /// 生成测试响应帧（TESTFR CON）。
    pub fn generate_test_response(&self) -> Result<Vec<u8>, NetworkError> {
        Ok(self.create_u_frame(0x83))
    }

    /// 生成测试激活帧（TESTFR ACT）。
    pub fn generate_test_act(&self) -> Result<Vec<u8>, NetworkError> {
        Ok(self.create_u_frame(0x43))
    }

    /// 生成启动数据传输激活帧（STARTDT ACT）。
    pub fn generate_startdt_act(&self) -> Result<Vec<u8>, NetworkError> {
        Ok(self.create_u_frame(0x07))
    }

    /// 生成启动数据传输确认帧（STARTDT CON）。
    pub fn generate_startdt_con(&self) -> Result<Vec<u8>, NetworkError> {
        Ok(self.create_u_frame(0x0B))
    }

    /// 生成停止数据传输确认帧（STOPDT CON）。
    pub fn generate_stopdt_con(&self) -> Result<Vec<u8>, NetworkError> {
        Ok(self.create_u_frame(0x23))
    }

    /// 重置链路层计数器（在 STARTDT 时调用）。
    fn reset_link_counters(&mut self) {
        self.send_sequence = 0;
        self.receive_sequence = 0;
        self.ack_sequence = 0;
        self.unconfirmed_count = 0;
        self.send_buffer.clear();
    }

    /// 清理已确认的帧（从发送缓冲区移除）。
    fn drain_acked_frames(&mut self) {
        while (self.send_buffer.len() as u16) > self.unconfirmed_count {
            self.send_buffer.pop_front();
        }
    }
}

/// 映射 U 帧控制类型并更新协议状态。
fn map_u_frame(control: UControl, state: &mut ProtocolState) -> UFrameType {
    match control {
        UControl::STARTDT_ACT => {
            *state = ProtocolState::DataTransfer;
            UFrameType::StartDtAct
        }
        UControl::STARTDT_CON => {
            *state = ProtocolState::DataTransfer;
            UFrameType::StartDtCon
        }
        UControl::STOPDT_ACT => {
            *state = ProtocolState::Connected;
            UFrameType::StopDtAct
        }
        UControl::STOPDT_CON => {
            *state = ProtocolState::Connected;
            UFrameType::StopDtCon
        }
        UControl::TESTFR_ACT => UFrameType::TestFrAct,
        UControl::TESTFR_CON => UFrameType::TestFrCon,
    }
}

/// 解码 ASDU（将解析器输出转换为运行时结构）。
fn decode_asdu(
    converter: &AsduConverter,
    frame: &iec60870_parser::types::FrameView,
) -> Result<DecodedAsdu, NetworkError> {
    let parsed =
        converter.convert(frame).map_err(|err| NetworkError::Protocol(format!("asdu convert failed: {err}")))?;

    let header = parsed.header();
    let raw_payload = frame.asdu().map(|asdu| asdu.payload.to_vec()).unwrap_or_default();

    let body = match parsed {
        AsduFrame::Measurements(set) => {
            let items = set
                .items
                .into_iter()
                .map(|m| -> Result<DecodedMeasurement, NetworkError> {
                    let quality_common = map_quality_common(m.quality);
                    let quality_detail = map_measurement_descriptor(m.descriptor);
                    let timestamp = m
                        .parsed_timestamp()
                        .map_err(|error| {
                            NetworkError::Protocol(format!("measurement timestamp decode failed: {error}"))
                        })?
                        .map(|timestamp| {
                            map_measurement_timestamp(timestamp, m.timestamp.expect("parsed timestamp has raw bytes"))
                        });
                    Ok(DecodedMeasurement {
                        ioa: m.ioa,
                        value: map_measurement_value(m.value),
                        quality: quality_detail.raw().unwrap_or_else(|| quality_common.as_u8()),
                        quality_common,
                        quality_detail,
                        timestamp,
                    })
                })
                .collect::<Result<Vec<_>, _>>()?;
            DecodedAsduBody::Measurements(items)
        }
        AsduFrame::Commands(set) => {
            let items = set
                .items
                .into_iter()
                .map(|cmd| DecodedCommand {
                    ioa: cmd.ioa,
                    detail: map_command_detail(cmd.detail, cmd.qualifier),
                    qualifier: cmd.qualifier,
                    timestamp: cmd.timestamp.map(|ts| ts.to_vec()),
                })
                .collect();
            DecodedAsduBody::Commands(items)
        }
        AsduFrame::TestCommand(frame) => {
            let items = frame
                .items
                .into_iter()
                .map(|cmd| DecodedCommand {
                    ioa: cmd.ioa,
                    detail: DecodedCommandDetail::TestPattern(cmd.fbp),
                    qualifier: CommandQualifier::None,
                    timestamp: cmd.timestamp.map(|timestamp| timestamp.to_vec()),
                })
                .collect();
            DecodedAsduBody::Commands(items)
        }
        AsduFrame::Interrogation(frame) => {
            let items = frame
                .items
                .into_iter()
                .map(|item| DecodedInterrogation { ioa: item.ioa, qualifier: item.qualifier })
                .collect();
            DecodedAsduBody::Interrogations(items)
        }
        AsduFrame::File(set) => {
            let items = set.items.into_iter().map(map_file_transfer_record).collect();
            DecodedAsduBody::FileTransfers(items)
        }
        AsduFrame::Raw(_) => DecodedAsduBody::Raw,
        _ => DecodedAsduBody::Raw,
    };

    Ok(DecodedAsdu {
        type_id: header.type_id.raw(),
        vsq: header.vsq,
        cause: header.cot.as_u16(),
        common_address: header.common_addr,
        body,
        raw_payload,
    })
}

/// 映射品质标志（从解析器类型到运行时类型）。
fn map_quality_common(quality: iec60870_parser::parser::asdu::qualifiers::QualityFlags) -> DecodedQualityCommon {
    DecodedQualityCommon { iv: quality.iv, nt: quality.nt, sb: quality.sb, bl: quality.bl }
}

fn map_measurement_timestamp(timestamp: Timestamp, raw: &[u8]) -> DecodedMeasurementTimestamp {
    match timestamp {
        Timestamp::CP24(value) => DecodedMeasurementTimestamp::Cp24 {
            raw: raw.to_vec(),
            milliseconds: value.milliseconds,
            minutes: value.minutes,
            invalid: value.invalid,
        },
        Timestamp::CP56(value) => DecodedMeasurementTimestamp::Cp56 {
            raw: raw.to_vec(),
            milliseconds: value.milliseconds,
            minutes: value.minutes,
            hours: value.hours,
            day: value.day,
            month: value.month,
            year: value.year,
            weekday: value.weekday,
            summer_time: value.summer_time,
            invalid: value.invalid,
        },
    }
}

/// 映射测量值描述符（从解析器类型到运行时类型）。
fn map_measurement_descriptor(descriptor: MeasurementDescriptor) -> DecodedMeasurementQualityDetail {
    match descriptor {
        MeasurementDescriptor::Siq { raw, spi, iv, nt, sb, bl } => {
            DecodedMeasurementQualityDetail::Siq { raw, spi, iv, nt, sb, bl }
        }
        MeasurementDescriptor::Diq { raw, dpi, iv, nt, sb, bl } => {
            DecodedMeasurementQualityDetail::Diq { raw, dpi, iv, nt, sb, bl }
        }
        MeasurementDescriptor::Qds { raw, ov, iv, nt, sb, bl, transient, scd_status, scd_change } => {
            DecodedMeasurementQualityDetail::Qds { raw, ov, iv, nt, sb, bl, transient, scd_status, scd_change }
        }
        MeasurementDescriptor::Bcr { raw, sq, cy, ca, iv } => {
            DecodedMeasurementQualityDetail::Bcr { raw, sq, cy, ca, iv }
        }
        MeasurementDescriptor::None => DecodedMeasurementQualityDetail::None,
    }
}

/// 映射测量值（从解析器类型到运行时类型）。
fn map_measurement_value(value: MeasurementValue<'_>) -> DecodedMeasurementValue {
    match value {
        MeasurementValue::SinglePoint { state } => DecodedMeasurementValue::Single(state.as_bool()),
        MeasurementValue::DoublePoint { state } => DecodedMeasurementValue::Double(state.as_u8()),
        MeasurementValue::StepPosition { value, transient } => {
            DecodedMeasurementValue::StepPosition { value, transient }
        }
        MeasurementValue::Normalized(v) | MeasurementValue::NormalizedNoQuality(v) => {
            DecodedMeasurementValue::Normalized(v)
        }
        MeasurementValue::Scaled(v) => DecodedMeasurementValue::Scaled(v),
        MeasurementValue::ShortFloat(v) => DecodedMeasurementValue::Float(v),
        MeasurementValue::IntegratedTotal { value, .. } => DecodedMeasurementValue::Integrated(value),
        MeasurementValue::BitString(bs) => {
            let val = if bs.len() >= 4 {
                u32::from_le_bytes([bs[0], bs[1], bs[2], bs[3]])
            } else {
                let mut arr = [0u8; 4];
                arr[..bs.len()].copy_from_slice(bs);
                u32::from_le_bytes(arr)
            };
            DecodedMeasurementValue::BitString(val)
        }
        MeasurementValue::PackedStatus { status, change } => DecodedMeasurementValue::PackedStatus { status, change },
        MeasurementValue::Raw(_) => DecodedMeasurementValue::Raw,
    }
}

/// 映射命令详情（从解析器类型到运行时类型）。
fn map_command_detail(detail: CommandDetail<'_>, qualifier: CommandQualifier) -> DecodedCommandDetail {
    match detail {
        CommandDetail::Single { state, select } => DecodedCommandDetail::Single { value: state.as_bool(), select },
        CommandDetail::Double { state, select } => DecodedCommandDetail::Double { value: state.as_u8(), select },
        CommandDetail::Step { position } => {
            DecodedCommandDetail::Step { position, select: qualifier.select().unwrap_or(false) }
        }
        CommandDetail::SetPointNormalized(v) => DecodedCommandDetail::SetPointNormalized(v),
        CommandDetail::SetPointScaled(v) => DecodedCommandDetail::SetPointScaled(v),
        CommandDetail::SetPointFloat(v) => DecodedCommandDetail::SetPointFloat(v),
        CommandDetail::BitString(bs) => {
            let val = if bs.len() >= 4 {
                u32::from_le_bytes([bs[0], bs[1], bs[2], bs[3]])
            } else {
                let mut arr = [0u8; 4];
                arr[..bs.len()].copy_from_slice(bs);
                u32::from_le_bytes(arr)
            };
            DecodedCommandDetail::BitString(val)
        }
        CommandDetail::Read => DecodedCommandDetail::Read,
        CommandDetail::ClockSync { raw, timestamp } => match timestamp {
            Timestamp::CP56(timestamp) => DecodedCommandDetail::ClockSynchronization(timestamp),
            Timestamp::CP24(_) => DecodedCommandDetail::Raw(raw.to_vec()),
        },
        CommandDetail::ResetProcess => DecodedCommandDetail::ResetProcess,
        CommandDetail::Raw(raw) => DecodedCommandDetail::Raw(raw.to_vec()),
    }
}

/// 映射文件传输记录（从解析器类型到运行时类型）。
fn map_file_transfer_record(record: file_transfer::FileTransferRecord<'_>) -> DecodedFileTransferRecord {
    match record {
        file_transfer::FileTransferRecord::FileReady(item) => DecodedFileTransferRecord::FileReady {
            ioa: item.ioa,
            file_name: item.file_name,
            qualifier: item.qualifier,
            length: item.length,
        },
        file_transfer::FileTransferRecord::SectionReady(item) => DecodedFileTransferRecord::SectionReady {
            ioa: item.ioa,
            file_name: item.file_name,
            qualifier: item.qualifier,
            section: item.section,
            length: item.length,
        },
        file_transfer::FileTransferRecord::SelectCall(item) => DecodedFileTransferRecord::SelectCall {
            ioa: item.ioa,
            file_name: item.file_name,
            qualifier: item.qualifier,
            section: item.section,
        },
        file_transfer::FileTransferRecord::LastSection(item) => DecodedFileTransferRecord::LastSection {
            ioa: item.ioa,
            file_name: item.file_name,
            qualifier: item.qualifier,
            last_section_number: item.last_section_number,
            last_segment_number: item.last_segment_number,
        },
        file_transfer::FileTransferRecord::AckSection(item) => DecodedFileTransferRecord::AckSection {
            ioa: item.ioa,
            file_name: item.file_name,
            qualifier: item.qualifier,
            section: item.section,
        },
        file_transfer::FileTransferRecord::Segment(item) => DecodedFileTransferRecord::Segment {
            ioa: item.ioa,
            file_name: item.file_name,
            section: item.section,
            los: item.los,
            data: item.data.to_vec(),
        },
        file_transfer::FileTransferRecord::Directory(item) => DecodedFileTransferRecord::Directory {
            ioa: item.ioa,
            file_name: item.file_name,
            qualifier: item.qualifier,
            section: item.section,
            length: item.attributes.length,
            last_section: item.attributes.last_section,
            last_segment: item.attributes.last_segment,
            status_raw: item.status.raw,
            is_file: item.status.is_file,
            is_last_file_of_directory: item.status.is_last_file_of_directory,
            timestamp: item.timestamp.map(|ts| ts.to_vec()),
        },
        file_transfer::FileTransferRecord::QueryLog(item) => DecodedFileTransferRecord::QueryLog {
            ioa: item.ioa,
            file_name: item.file_name,
            range_start_time: item.range_start_time.to_vec(),
            range_end_time: item.range_end_time.to_vec(),
        },
        file_transfer::FileTransferRecord::Raw(raw) => DecodedFileTransferRecord::Raw(raw.to_vec()),
    }
}

/// 计算序号距离（用于计算未确认帧数）。
///
/// 返回从 peer_ack 到 next_send 的序号距离（模 32768）。
#[inline]
fn seq_distance(next_send: u16, peer_ack: u16) -> u16 {
    let next_send = next_send as u32;
    let peer_ack = peer_ack as u32;
    ((next_send + 32768 - peer_ack) % 32768) as u16
}

/// 计算序号前向距离（用于检测重复帧或序号跳跃）。
///
/// 返回从 from 到 to 的前向序号距离（模 32768）。
#[inline]
fn seq_forward_distance(from: u16, to: u16) -> u16 {
    let from = from as u32;
    let to = to as u32;
    ((to + 32768 - from) % 32768) as u16
}

/// 协议消息（handle_received_data 的返回结果）。
#[derive(Debug, Clone)]
pub enum ProtocolMessage {
    /// I 帧（信息帧）：包含 ASDU 数据
    IFrame { send_sequence: u16, receive_sequence: u16, decoded_asdu: DecodedAsdu, raw_frame: Vec<u8> },
    /// 链路层已接收，但 ASDU 语义校验或应用解码失败的 I 帧。
    IFrameDecodeError { send_sequence: u16, receive_sequence: u16, error: String, raw_frame: Vec<u8> },
    /// S 帧（监督帧）：确认接收
    SFrame { receive_sequence: u16, raw_frame: Vec<u8> },
    /// U 帧（未编号控制帧）：链路控制
    UFrame { u_type: UFrameType, raw_frame: Vec<u8> },
    /// 需要立即发送 S 帧确认（收到重复/旧 I 帧）
    AckRequired { expected_receive_sequence: u16, got_send_sequence: u16 },
    /// 帧计数错误（序号跳跃，需要链路恢复）
    FrameCountError { expected_receive_sequence: u16, got_send_sequence: u16 },
    /// 帧被忽略（数据传输未建立）
    IgnoredFrame { frame_type: &'static str, reason: String },
}

/// U 帧类型。
#[derive(Debug, Clone)]
pub enum UFrameType {
    /// STARTDT ACT（启动数据传输激活）
    StartDtAct,
    /// STARTDT CON（启动数据传输确认）
    StartDtCon,
    /// STOPDT ACT（停止数据传输激活）
    StopDtAct,
    /// STOPDT CON（停止数据传输确认）
    StopDtCon,
    /// TESTFR ACT（测试帧激活）
    TestFrAct,
    /// TESTFR CON（测试帧确认）
    TestFrCon,
}

impl Default for Iec104Protocol {
    fn default() -> Self {
        Self::new()
    }
}
