//! 链路状态机：在 `link-manager` feature 下维护 STARTDT/STOPDT/TESTFR 语义。
//!
//! 解析器保持无状态，本模块提供 [`LinkValidator`] trait 和默认实现，
//! 让应用可以在帧级别验证序列窗口、U 帧动作和 T0/T1/T2 计时器。
//!
//! ## 使用示例
//!
//! ```ignore
//! use iec60870_parser::link::{DefaultLinkValidator, LinkValidator, AckAction};
//! use iec60870_parser::parser::apci::parse_apci;
//!
//! let mut validator = DefaultLinkValidator::default();
//! let apci = parse_apci(&frame_bytes)?;
//!
//! match validator.on_frame(&apci)? {
//!     AckAction::SendStartDtCon => {
//!         // 发送 STARTDT 确认帧
//!     }
//!     AckAction::SendAck { ns } => {
//!         // 发送 S 帧确认，包含接收序列号 ns
//!     }
//!     AckAction::None => {
//!         // 无需发送确认
//!     }
//!     _ => {}
//! }
//! ```

use std::time::Duration;

use crate::{
    error::LinkError,
    parser::apci::{ApciHeader, FrameType, UControl},
};

/// 链路状态枚举。
///
/// 表示 IEC 60870-5-104 链路层的当前状态。状态转换遵循协议规定：
///
/// ```text
/// Idle ──STARTDT_ACT──> Started ──STOPDT_ACT──> Stopped
///  └────────────────────────────────────────────┘
///                   STARTDT_ACT
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LinkState {
    /// 空闲状态 - 连接建立后的初始状态。
    ///
    /// 在此状态下：
    /// - 可以接收 STARTDT_ACT 转入 Started 状态
    /// - 不能传输 I 帧或 S 帧
    Idle,
    /// 已启动状态 - 可以正常传输数据。
    ///
    /// 在此状态下：
    /// - 可以传输和接收 I 帧（数据传输）
    /// - 可以传输和接收 S 帧（确认）
    /// - 可以接收 STOPDT_ACT 转入 Stopped 状态
    Started,
    /// 已停止状态 - 数据传输已暂停。
    ///
    /// 在此状态下：
    /// - 不能传输 I 帧或 S 帧
    /// - 可以接收 STARTDT_ACT 重新转入 Started 状态
    Stopped,
    /// 测试状态 - 正在执行链路测试。
    ///
    /// TESTFR 帧用于检测链路连通性，不影响正常数据传输。
    Test,
}

/// 链路确认动作枚举。
///
/// 表示接收到帧后需要执行的响应动作。调用方应根据返回的动作
/// 发送相应的确认帧。
#[derive(Debug, PartialEq, Eq)]
pub enum AckAction {
    /// 发送 STARTDT 确认帧 (STARTDT_CON)。
    ///
    /// 响应 STARTDT_ACT，表示同意启动数据传输。
    SendStartDtCon,
    /// 发送 STOPDT 确认帧 (STOPDT_CON)。
    ///
    /// 响应 STOPDT_ACT，表示同意停止数据传输。
    SendStopDtCon,
    /// 发送 TESTFR 确认帧 (TESTFR_CON)。
    ///
    /// 响应 TESTFR_ACT，表示链路正常。
    SendTestFrCon,
    /// 发送 S 帧确认。
    ///
    /// 确认已成功接收 I 帧，`ns` 是下一个期望接收的序列号。
    SendAck {
        /// 接收序列号 - 下一个期望接收的 I 帧序列号。
        ns: u16,
    },
    /// 请求重新同步。
    ///
    /// 当序列号错误或状态异常时，请求对端重新建立同步。
    RequestResync,
    /// 无需执行任何动作。
    ///
    /// 例如接收到确认帧 (CON) 时，只需更新内部状态。
    None,
}

/// 链路验证器配置。
///
/// - `k`: 发送窗口上限（未确认 I 帧数量上限）
/// - `w`: 接收窗口阈值（累计接收多少 I 帧后主动发送确认）
/// - `t1`: 等待对端确认超时时间
/// - `t2`: 延迟确认超时时间
/// - `t3`: 空闲链路超时时间
#[derive(Debug, Clone, Copy)]
pub struct LinkConfig {
    pub k: u16,
    pub w: u16,
    pub t1: Duration,
    pub t2: Duration,
    pub t3: Duration,
}

impl Default for LinkConfig {
    fn default() -> Self {
        Self { k: 12, w: 1, t1: Duration::from_secs(15), t2: Duration::from_secs(10), t3: Duration::from_secs(20) }
    }
}

/// 链路验证器 trait。
///
/// 定义链路层状态管理的核心接口。实现此 trait 可以自定义
/// 链路管理逻辑，例如添加超时处理、滑动窗口管理等功能。
///
/// ## 实现注意事项
///
/// - 必须正确维护序列号状态，防止序列号错误
/// - 必须严格执行状态转换规则，避免协议违规
/// - 应处理序列号溢出（16 位环绕）
pub trait LinkValidator {
    /// 处理接收到的帧。
    ///
    /// 每当接收到一个完整的 APCI 帧时调用此方法。实现应：
    /// 1. 验证帧的合法性（状态、序列号等）
    /// 2. 更新内部状态
    /// 3. 返回需要发送的确认动作
    ///
    /// # 参数
    ///
    /// - `apci`: 接收到的 APCI 头部信息
    ///
    /// # 返回值
    ///
    /// - `Ok(AckAction)`: 需要执行的确认动作
    /// - `Err(LinkError)`: 帧验证失败，如状态错误、序列号不匹配等
    ///
    /// # 示例
    ///
    /// ```ignore
    /// let action = validator.on_frame(&apci)?;
    /// match action {
    ///     AckAction::SendStartDtCon => send_startdt_con(),
    ///     AckAction::SendAck { ns } => send_s_frame(ns),
    ///     _ => {}
    /// }
    /// ```
    fn on_frame(&mut self, apci: &ApciHeader) -> Result<AckAction, LinkError>;

    /// 周期性心跳调用。
    ///
    /// 应用层应定期调用此方法，用于处理超时逻辑。例如：
    /// - T1 超时：等待确认超时
    /// - T2 超时：延迟确认超时（触发发送 S 帧确认）
    /// - T3 超时：链路空闲超时
    ///
    /// # 参数
    ///
    /// - `elapsed`: 自上次调用以来经过的时间
    ///
    /// # 返回值
    ///
    /// - `Ok(Some(AckAction))`: 超时触发的动作（如发送 TESTFR）
    /// - `Ok(None)`: 未触发任何动作
    /// - `Err(LinkError)`: 超时导致的错误（如长时间无响应）
    fn tick(&mut self, elapsed: Duration) -> Result<Option<AckAction>, LinkError>;
}

/// 默认链路验证器实现。
///
/// 按照 IEC 60870-5-104 标准实现基本的链路层状态管理：
/// - 维护链路状态（Idle/Started/Stopped/Test）
/// - 验证序列号连续性
/// - 处理 U 帧状态转换
///
/// ## 限制
///
/// 当前实现不包括：
/// - T0 连接建立计时器
/// - 自动重传机制
///
/// ## 示例
///
/// ```ignore
/// let mut validator = DefaultLinkValidator::default();
///
/// // 处理 STARTDT
/// let startdt = parse_apci(&startdt_frame)?;
/// assert_eq!(validator.on_frame(&startdt)?, AckAction::SendStartDtCon);
/// assert_eq!(validator.state(), LinkState::Started);
///
/// // 处理 I 帧
/// let i_frame = parse_apci(&i_frame_data)?;
/// if let AckAction::SendAck { ns } = validator.on_frame(&i_frame)? {
///     send_s_frame(ns);
/// }
/// ```
pub struct DefaultLinkValidator {
    config: LinkConfig,
    state: LinkState,
    expect_recv: u16,
    last_peer_ack: Option<u16>,
    pending_ack_count: u16,
    outgoing_tracking_enabled: bool,
    send_next: u16,
    send_oldest_unacked: u16,
    send_unacked: u16,
    t1_elapsed: Duration,
    t2_elapsed: Duration,
    t3_elapsed: Duration,
}

impl DefaultLinkValidator {
    /// 获取当前链路状态。
    ///
    /// # 示例
    ///
    /// ```ignore
    /// let validator = DefaultLinkValidator::default();
    /// assert_eq!(validator.state(), LinkState::Idle);
    /// ```
    pub fn state(&self) -> LinkState {
        self.state
    }

    /// 使用自定义参数创建链路验证器。
    pub fn with_config(config: LinkConfig) -> Self {
        Self { config: normalize_config(config), ..Self::default() }
    }

    /// 返回当前链路参数。
    pub fn config(&self) -> LinkConfig {
        self.config
    }

    /// 注册一条本端即将发送的 I 帧，返回本端应使用的发送序号 N(S)。
    ///
    /// 调用方在真正发出 I 帧之前调用此方法，用于启用：
    /// - 发送窗口 `k` 限流
    /// - 对端 N(R) 上界检查
    /// - T1 计时
    pub fn on_local_i_sent(&mut self) -> Result<u16, LinkError> {
        enforce_started(self.state)?;
        self.outgoing_tracking_enabled = true;
        if self.send_unacked >= self.config.k {
            return Err(LinkError::InvalidState("发送窗口已满(k)"));
        }
        let ns = self.send_next;
        self.send_next = seq_inc(self.send_next);
        self.send_unacked = self.send_unacked.saturating_add(1);
        self.t1_elapsed = Duration::ZERO;
        self.t3_elapsed = Duration::ZERO;
        Ok(ns)
    }

    fn reset_session(&mut self) {
        self.expect_recv = 0;
        self.last_peer_ack = None;
        self.pending_ack_count = 0;
        self.outgoing_tracking_enabled = false;
        self.send_next = 0;
        self.send_oldest_unacked = 0;
        self.send_unacked = 0;
        self.t1_elapsed = Duration::ZERO;
        self.t2_elapsed = Duration::ZERO;
        self.t3_elapsed = Duration::ZERO;
    }

    fn on_peer_ack(&mut self, nr: u16) -> Result<(), LinkError> {
        update_peer_ack(nr, &mut self.last_peer_ack)?;
        if !self.outgoing_tracking_enabled {
            return Ok(());
        }
        let acked = seq_distance(self.send_oldest_unacked, nr);
        if acked > self.send_unacked {
            return Err(LinkError::InvalidState("对端确认序号 N(R) 超出已发送范围"));
        }
        if acked > 0 {
            self.send_oldest_unacked = nr;
            self.send_unacked -= acked;
            self.t1_elapsed = Duration::ZERO;
        }
        Ok(())
    }

    fn on_incoming_i(&mut self, apci: &ApciHeader) -> Result<AckAction, LinkError> {
        self.on_peer_ack(apci.recv_seq)?;
        check_sequence(apci.send_seq, self.expect_recv)?;
        self.expect_recv = seq_inc(self.expect_recv);

        if self.pending_ack_count == 0 {
            self.t2_elapsed = Duration::ZERO;
        }
        self.pending_ack_count = self.pending_ack_count.saturating_add(1);
        if self.pending_ack_count >= self.config.w {
            self.pending_ack_count = 0;
            self.t2_elapsed = Duration::ZERO;
            return Ok(AckAction::SendAck { ns: self.expect_recv });
        }
        Ok(AckAction::None)
    }
}

impl Default for DefaultLinkValidator {
    fn default() -> Self {
        Self {
            config: LinkConfig::default(),
            state: LinkState::Idle,
            expect_recv: 0,
            last_peer_ack: None,
            pending_ack_count: 0,
            outgoing_tracking_enabled: false,
            send_next: 0,
            send_oldest_unacked: 0,
            send_unacked: 0,
            t1_elapsed: Duration::ZERO,
            t2_elapsed: Duration::ZERO,
            t3_elapsed: Duration::ZERO,
        }
    }
}

impl LinkValidator for DefaultLinkValidator {
    fn on_frame(&mut self, apci: &ApciHeader) -> Result<AckAction, LinkError> {
        self.t3_elapsed = Duration::ZERO;
        match apci.frame_type {
            FrameType::I => {
                enforce_started(self.state)?;
                self.on_incoming_i(apci)
            }
            FrameType::S => {
                enforce_started(self.state)?;
                self.on_peer_ack(apci.recv_seq)?;
                Ok(AckAction::None)
            }
            FrameType::U => {
                let control = apci.u_control().ok_or(LinkError::InvalidState("缺少 U 控制位"))?;
                let action = map_u_control(control, self.state)?;
                match action {
                    AckAction::SendStartDtCon => {
                        self.state = LinkState::Started;
                        self.reset_session();
                    }
                    AckAction::SendStopDtCon => {
                        self.state = LinkState::Stopped;
                        self.pending_ack_count = 0;
                        self.t1_elapsed = Duration::ZERO;
                        self.t2_elapsed = Duration::ZERO;
                        self.t3_elapsed = Duration::ZERO;
                    }
                    _ => {}
                }
                Ok(action)
            }
        }
    }

    fn tick(&mut self, elapsed: Duration) -> Result<Option<AckAction>, LinkError> {
        if self.state != LinkState::Started {
            return Ok(None);
        }

        self.t3_elapsed = self.t3_elapsed.saturating_add(elapsed);

        if self.pending_ack_count > 0 {
            self.t2_elapsed = self.t2_elapsed.saturating_add(elapsed);
            if self.t2_elapsed >= self.config.t2 {
                self.pending_ack_count = 0;
                self.t2_elapsed = Duration::ZERO;
                self.t3_elapsed = Duration::ZERO;
                return Ok(Some(AckAction::SendAck { ns: self.expect_recv }));
            }
        }

        if self.outgoing_tracking_enabled && self.send_unacked > 0 {
            self.t1_elapsed = self.t1_elapsed.saturating_add(elapsed);
            if self.t1_elapsed >= self.config.t1 {
                return Err(LinkError::InvalidState("等待对端确认超时(T1)"));
            }
        }

        if self.config.t3 > Duration::ZERO && self.t3_elapsed >= self.config.t3 {
            self.t3_elapsed = Duration::ZERO;
            return Ok(Some(AckAction::RequestResync));
        }

        Ok(None)
    }
}

fn map_u_control(control: UControl, state: LinkState) -> Result<AckAction, LinkError> {
    match control {
        UControl::STARTDT_ACT => {
            if !matches!(state, LinkState::Idle | LinkState::Stopped) {
                return Err(LinkError::InvalidState("STARTDT_ACT 只能在 Idle/Stopped 状态接收"));
            }
            Ok(AckAction::SendStartDtCon)
        }
        UControl::STOPDT_ACT => {
            if !matches!(state, LinkState::Started) {
                return Err(LinkError::InvalidState("STOPDT_ACT 只能在 Started 状态接收"));
            }
            Ok(AckAction::SendStopDtCon)
        }
        UControl::TESTFR_ACT => Ok(AckAction::SendTestFrCon),
        UControl::STARTDT_CON | UControl::STOPDT_CON | UControl::TESTFR_CON => Ok(AckAction::None),
    }
}

fn enforce_started(state: LinkState) -> Result<(), LinkError> {
    if state == LinkState::Started {
        Ok(())
    } else {
        Err(LinkError::InvalidState("未建立 STARTDT，不能处理 I/S 帧"))
    }
}

fn check_sequence(actual: u16, expected: u16) -> Result<(), LinkError> {
    if actual == expected { Ok(()) } else { Err(LinkError::InvalidState("接收序号不匹配")) }
}

fn normalize_config(mut config: LinkConfig) -> LinkConfig {
    if config.k == 0 {
        config.k = 1;
    }
    if config.w == 0 {
        config.w = 1;
    }
    if config.w > config.k {
        config.w = config.k;
    }
    config
}

fn seq_inc(value: u16) -> u16 {
    value.wrapping_add(1) & 0x7FFF
}

fn seq_distance(from: u16, to: u16) -> u16 {
    to.wrapping_sub(from) & 0x7FFF
}

fn update_peer_ack(current: u16, last_peer_ack: &mut Option<u16>) -> Result<(), LinkError> {
    if let Some(previous) = *last_peer_ack {
        let delta = seq_distance(previous, current);
        if delta > 0x4000 {
            return Err(LinkError::InvalidState("对端确认序号 N(R) 回退"));
        }
    }
    *last_peer_ack = Some(current);
    Ok(())
}
