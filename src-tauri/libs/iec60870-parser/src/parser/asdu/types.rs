//! Type Identification (TI) 类型定义及解析策略。
//!
//! 本模块包含：
//! - [`TypeId`] newtype: 协议层的原始 TI 表示（1..=127）
//! - [`TypeIdEnum`] 枚举: 类型安全的 TI 枚举，提供语义化分类方法
//! - [`TypeDescriptor`]: ASDU 解析策略描述（IOA/数据/时间戳长度）
//! - 编译期生成的 TI 查找表，支持零运行时开销的策略查询
//!
//! # TypeId vs TypeIdEnum
//!
//! - `TypeId`: 底层 newtype，代表协议原始值，用于数据存储和传输
//! - `TypeIdEnum`: 高层枚举，提供类型安全的业务操作，通过 `.kind()` 转换

use core::fmt;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AsduCategory {
    Measurement,
    Protection,
    Control,
    Initialization,
    Security,
    System,
    Parameter,
    FileTransfer,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InformationFamily {
    SinglePoint,
    DoublePoint,
    StepPosition,
    BitString,
    Normalized,
    Scaled,
    ShortFloat,
    IntegratedTotal,
    Protection,
    PackedStatus,
    Control,
    Initialization,
    Security,
    SystemCommand,
    Parameter,
    FileTransfer,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ValueKind {
    None,
    Boolean,
    DoublePoint,
    StepPosition,
    BitString32,
    Normalized,
    ScaledI16,
    Float32,
    Counter,
    Structured,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum QualityKind {
    None,
    Siq,
    Diq,
    Qds,
    Bcr,
    Qdp,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum QualifierKind {
    None,
    Command,
    Setpoint,
    Interrogation,
    CounterInterrogation,
    ResetProcess,
    Parameter,
    FileTransfer,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TimestampKind {
    None,
    Cp24,
    Cp56,
}

/// Type Identification (TI) 封装，值域 1..=127。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct TypeId(u8);

impl TypeId {
    /// 构造常量 TI，调用方需保证在合法范围。
    pub const fn new(val: u8) -> Self {
        Self(val)
    }

    /// 返回底层数值。
    pub const fn raw(self) -> u8 {
        self.0
    }

    /// 从原始值生成 `TypeId`，若超出 1..=127 返回 `None`。
    pub fn from_raw(val: u8) -> Option<Self> {
        if (1..=127).contains(&val) { Some(Self(val)) } else { None }
    }

    /// 将 TypeId 映射到类型安全的枚举 `TypeIdEnum`
    ///
    /// 由于 `TypeId::from_raw` 已确保值在 1..=127 范围内，
    /// 此方法总是成功。对于标准定义的 TypeId，返回对应的枚举变体；
    /// 对于非标准值（如 22-29 间隙），返回 `TypeIdEnum::Unknown(val)`。
    ///
    /// # 示例
    ///
    /// ```
    /// # use iec60870_parser::parser::asdu::{TypeId, TypeIdEnum};
    /// let ti = TypeId::from_raw(45).unwrap();
    /// assert_eq!(ti.kind(), TypeIdEnum::CScNa1);
    ///
    /// let ti_unknown = TypeId::from_raw(22).unwrap();
    /// assert_eq!(ti_unknown.kind(), TypeIdEnum::Unknown(22));
    /// ```
    pub fn kind(&self) -> TypeIdEnum {
        // SAFETY: TypeId 保证值在 1..=127 范围内，TypeIdEnum::try_from 总是返回 Ok
        TypeIdEnum::try_from(self.0).expect("TypeId 值域已由 from_raw 校验")
    }
}

impl fmt::Display for TypeId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "TI({})", self.0)
    }
}

// ============================================================================
// TypeIdEnum - 类型安全的枚举表示
// ============================================================================

/// Type Identification (TI) 枚举，覆盖 IEC 60870-5-104 标准定义的 1-127 值
///
/// IEC 60870-5-104 定义了多种 ASDU 类型，按功能分为：
/// - 遥信/遥测（M_*）：监测数据上送
/// - 遥控/设定值（C_*）：控制命令下发
/// - 参数（P_*）：参数传输
/// - 文件传输（F_*）：文件操作
/// - 安全/维护（S_*）：安全认证相关
/// - 系统（M_EI_NA_1）：系统事件
///
/// # 示例
///
/// ```
/// use iec60870_parser::parser::asdu::TypeIdEnum;
///
/// // 从原始值创建
/// let ti = TypeIdEnum::try_from(45).unwrap();
/// assert_eq!(ti, TypeIdEnum::CScNa1);
/// assert!(ti.is_command());
///
/// // 显示标准名称
/// assert_eq!(ti.to_string(), "C_SC_NA_1");
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum TypeIdEnum {
    // ==================== 遥信/遥测（不带 CP56 时间戳）====================
    /// M_SP_NA_1 (1): 单点信息
    MSpNa1,
    /// M_SP_TA_1 (2): 带 CP24 时间标签的单点信息
    MSpTa1,
    /// M_DP_NA_1 (3): 双点信息
    MDpNa1,
    /// M_DP_TA_1 (4): 带 CP24 时间标签的双点信息
    MDpTa1,
    /// M_ST_NA_1 (5): 步位置信息
    MStNa1,
    /// M_ST_TA_1 (6): 带 CP24 时间标签的步位置信息
    MStTa1,
    /// M_BO_NA_1 (7): 比特串
    MBoNa1,
    /// M_BO_TA_1 (8): 带 CP24 时间标签的比特串
    MBoTa1,
    /// M_ME_NA_1 (9): 归一化遥测值
    MMeNa1,
    /// M_ME_TA_1 (10): 带 CP24 时间标签的归一化遥测值
    MMeTa1,
    /// M_ME_NB_1 (11): 标度化遥测值
    MMeNb1,
    /// M_ME_TB_1 (12): 带 CP24 时间标签的标度化遥测值
    MMeTb1,
    /// M_ME_NC_1 (13): 短浮点遥测值
    MMeNc1,
    /// M_ME_TC_1 (14): 带 CP24 时间标签的短浮点遥测值
    MMeTc1,
    /// M_IT_NA_1 (15): 累计量
    MItNa1,
    /// M_IT_TA_1 (16): 带 CP24 时间标签的累计量
    MItTa1,
    /// M_EP_TA_1 (17): 带 CP24 时间标签的继电保护装置事件
    MEpTa1,
    /// M_EP_TB_1 (18): 带 CP24 时间标签的继电保护装置成组启动事件
    MEpTb1,
    /// M_EP_TC_1 (19): 带 CP24 时间标签的继电保护装置成组输出电路信息
    MEpTc1,
    /// M_PS_NA_1 (20): 具有状态变位检出的成组单点信息
    MPsNa1,
    /// M_ME_ND_1 (21): 不带品质描述的归一化遥测值
    MMeNd1,

    // ==================== 遥信/遥测（带 CP56 时间戳）====================
    /// M_SP_TB_1 (30): 带 CP56 时间标签的单点信息
    MSpTb1,
    /// M_DP_TB_1 (31): 带 CP56 时间标签的双点信息
    MDpTb1,
    /// M_ST_TB_1 (32): 带 CP56 时间标签的步位置信息
    MStTb1,
    /// M_BO_TB_1 (33): 带 CP56 时间标签的比特串
    MBoTb1,
    /// M_ME_TD_1 (34): 带 CP56 时间标签的归一化遥测值
    MMeTd1,
    /// M_ME_TE_1 (35): 带 CP56 时间标签的标度化遥测值
    MMeTe1,
    /// M_ME_TF_1 (36): 带 CP56 时间标签的短浮点遥测值
    MMeTf1,
    /// M_IT_TB_1 (37): 带 CP56 时间标签的累计量
    MItTb1,
    /// M_EP_TD_1 (38): 带 CP56 时间标签的继电保护装置事件
    MEpTd1,
    /// M_EP_TE_1 (39): 带 CP56 时间标签的继电保护装置成组启动事件
    MEpTe1,
    /// M_EP_TF_1 (40): 带 CP56 时间标签的继电保护装置成组输出电路信息
    MEpTf1,
    /// S_IT_TC_1 (41): 带 CP56 时间标签的累计量（安全版）
    SItTc1,

    // ==================== 遥控/设定值命令（不带时间戳）====================
    /// C_SC_NA_1 (45): 单点命令
    CScNa1,
    /// C_DC_NA_1 (46): 双点命令
    CDcNa1,
    /// C_RC_NA_1 (47): 步调节命令
    CRcNa1,
    /// C_SE_NA_1 (48): 归一化设定值命令
    CSeNa1,
    /// C_SE_NB_1 (49): 标度化设定值命令
    CSeNb1,
    /// C_SE_NC_1 (50): 短浮点设定值命令
    CSeNc1,
    /// C_BO_NA_1 (51): 比特串命令
    CBoNa1,

    // ==================== 遥控/设定值命令（带 CP56 时间戳）====================
    /// C_SC_TA_1 (58): 带 CP56 时间标签的单点命令
    CScTa1,
    /// C_DC_TA_1 (59): 带 CP56 时间标签的双点命令
    CDcTa1,
    /// C_RC_TA_1 (60): 带 CP56 时间标签的步调节命令
    CRcTa1,
    /// C_SE_TA_1 (61): 带 CP56 时间标签的归一化设定值命令
    CSeTa1,
    /// C_SE_TB_1 (62): 带 CP56 时间标签的标度化设定值命令
    CSeTb1,
    /// C_SE_TC_1 (63): 带 CP56 时间标签的短浮点设定值命令
    CSeTc1,
    /// C_BO_TA_1 (64): 带 CP56 时间标签的比特串命令
    CBoTa1,

    // ==================== 系统事件 ====================
    /// M_EI_NA_1 (70): 初始化结束
    MEiNa1,

    // ==================== 安全/维护相关 ====================
    /// S_CH_NA_1 (81): 认证质询
    SChNa1,
    /// S_RP_NA_1 (82): 认证应答
    SRpNa1,
    /// S_AR_NA_1 (83): 主动认证请求
    SArNa1,
    /// S_KR_NA_1 (84): 会话密钥状态请求
    SKrNa1,
    /// S_KS_NA_1 (85): 会话密钥状态
    SKsNa1,
    /// S_KC_NA_1 (86): 会话密钥改变
    SKcNa1,
    /// S_ER_NA_1 (87): 认证错误
    SErNa1,
    /// S_US_NA_1 (90): 用户状态改变
    SUsNa1,
    /// S_UQ_NA_1 (91): 更新密钥改变请求
    SUqNa1,
    /// S_UR_NA_1 (92): 更新密钥改变应答
    SUrNa1,
    /// S_UK_NA_1 (93): 更新密钥改变 - 对称
    SUkNa1,
    /// S_UA_NA_1 (94): 更新密钥改变 - 非对称
    SUaNa1,
    /// S_UC_NA_1 (95): 更新密钥改变确认
    SUcNa1,

    // ==================== 系统命令/召唤 ====================
    /// C_IC_NA_1 (100): 总召唤命令
    CIcNa1,
    /// C_CI_NA_1 (101): 电能脉冲召唤命令
    CCiNa1,
    /// C_RD_NA_1 (102): 读命令
    CRdNa1,
    /// C_CS_NA_1 (103): 时钟同步命令
    CCsNa1,
    /// C_TS_NA_1 (104): 测试命令（DL/T 634.5104 国内标准）
    CTsNa1,
    /// C_RP_NA_1 (105): 复位进程命令
    CRpNa1,
    /// C_TS_TA_1 (107): 带 CP56 时间标签的测试命令
    CTsTa1,

    // ==================== 参数/系数 ====================
    /// P_ME_NA_1 (110): 归一化参数
    PMeNa1,
    /// P_ME_NB_1 (111): 标度化参数
    PMeNb1,
    /// P_ME_NC_1 (112): 短浮点参数
    PMeNc1,
    /// P_AC_NA_1 (113): 参数激活
    PAcNa1,

    // ==================== 文件传输 ====================
    /// F_FR_NA_1 (120): 文件准备就绪
    FFrNa1,
    /// F_SR_NA_1 (121): 节准备就绪
    FSrNa1,
    /// F_SC_NA_1 (122): 召唤目录、选择文件、召唤文件
    FScNa1,
    /// F_LS_NA_1 (123): 最后的节、最后的段
    FLsNa1,
    /// F_AF_NA_1 (124): 确认文件、确认节
    FAfNa1,
    /// F_SG_NA_1 (125): 段
    FSgNa1,
    /// F_DR_TA_1 (126): 目录
    FDrTa1,
    /// F_SC_NB_1 (127): 查询日志、请求日志
    FScNb1,

    /// 未知或非标准 TypeId（用于处理非 1-127 的值或标准未定义的值）
    Unknown(u8),
}

impl TypeIdEnum {
    /// 返回 TypeId 的原始 u8 值
    ///
    /// # 示例
    ///
    /// ```
    /// # use iec60870_parser::parser::asdu::TypeIdEnum;
    /// assert_eq!(TypeIdEnum::CScNa1.to_u8(), 45);
    /// assert_eq!(TypeIdEnum::Unknown(200).to_u8(), 200);
    /// ```
    #[inline]
    pub const fn to_u8(self) -> u8 {
        match self {
            Self::MSpNa1 => 1,
            Self::MSpTa1 => 2,
            Self::MDpNa1 => 3,
            Self::MDpTa1 => 4,
            Self::MStNa1 => 5,
            Self::MStTa1 => 6,
            Self::MBoNa1 => 7,
            Self::MBoTa1 => 8,
            Self::MMeNa1 => 9,
            Self::MMeTa1 => 10,
            Self::MMeNb1 => 11,
            Self::MMeTb1 => 12,
            Self::MMeNc1 => 13,
            Self::MMeTc1 => 14,
            Self::MItNa1 => 15,
            Self::MItTa1 => 16,
            Self::MEpTa1 => 17,
            Self::MEpTb1 => 18,
            Self::MEpTc1 => 19,
            Self::MPsNa1 => 20,
            Self::MMeNd1 => 21,
            Self::MSpTb1 => 30,
            Self::MDpTb1 => 31,
            Self::MStTb1 => 32,
            Self::MBoTb1 => 33,
            Self::MMeTd1 => 34,
            Self::MMeTe1 => 35,
            Self::MMeTf1 => 36,
            Self::MItTb1 => 37,
            Self::MEpTd1 => 38,
            Self::MEpTe1 => 39,
            Self::MEpTf1 => 40,
            Self::SItTc1 => 41,
            Self::CScNa1 => 45,
            Self::CDcNa1 => 46,
            Self::CRcNa1 => 47,
            Self::CSeNa1 => 48,
            Self::CSeNb1 => 49,
            Self::CSeNc1 => 50,
            Self::CBoNa1 => 51,
            Self::CScTa1 => 58,
            Self::CDcTa1 => 59,
            Self::CRcTa1 => 60,
            Self::CSeTa1 => 61,
            Self::CSeTb1 => 62,
            Self::CSeTc1 => 63,
            Self::CBoTa1 => 64,
            Self::MEiNa1 => 70,
            Self::SChNa1 => 81,
            Self::SRpNa1 => 82,
            Self::SArNa1 => 83,
            Self::SKrNa1 => 84,
            Self::SKsNa1 => 85,
            Self::SKcNa1 => 86,
            Self::SErNa1 => 87,
            Self::SUsNa1 => 90,
            Self::SUqNa1 => 91,
            Self::SUrNa1 => 92,
            Self::SUkNa1 => 93,
            Self::SUaNa1 => 94,
            Self::SUcNa1 => 95,
            Self::CIcNa1 => 100,
            Self::CCiNa1 => 101,
            Self::CRdNa1 => 102,
            Self::CCsNa1 => 103,
            Self::CTsNa1 => 104,
            Self::CRpNa1 => 105,
            Self::CTsTa1 => 107,
            Self::PMeNa1 => 110,
            Self::PMeNb1 => 111,
            Self::PMeNc1 => 112,
            Self::PAcNa1 => 113,
            Self::FFrNa1 => 120,
            Self::FSrNa1 => 121,
            Self::FScNa1 => 122,
            Self::FLsNa1 => 123,
            Self::FAfNa1 => 124,
            Self::FSgNa1 => 125,
            Self::FDrTa1 => 126,
            Self::FScNb1 => 127,
            Self::Unknown(v) => v,
        }
    }

    /// 判断是否为遥信/遥测类型（M_* 系列）
    ///
    /// 包括所有监测数据上送类型，如单点信息、遥测值、累计量等。
    #[inline]
    pub const fn is_measurement(self) -> bool {
        matches!(
            self,
            Self::MSpNa1
                | Self::MSpTa1
                | Self::MDpNa1
                | Self::MDpTa1
                | Self::MStNa1
                | Self::MStTa1
                | Self::MBoNa1
                | Self::MBoTa1
                | Self::MMeNa1
                | Self::MMeTa1
                | Self::MMeNb1
                | Self::MMeTb1
                | Self::MMeNc1
                | Self::MMeTc1
                | Self::MItNa1
                | Self::MItTa1
                | Self::MEpTa1
                | Self::MEpTb1
                | Self::MEpTc1
                | Self::MPsNa1
                | Self::MMeNd1
                | Self::MSpTb1
                | Self::MDpTb1
                | Self::MStTb1
                | Self::MBoTb1
                | Self::MMeTd1
                | Self::MMeTe1
                | Self::MMeTf1
                | Self::MItTb1
                | Self::MEpTd1
                | Self::MEpTe1
                | Self::MEpTf1
                | Self::SItTc1
        )
    }

    /// 判断是否为遥控/设定值/召唤类命令（C_* 系列）
    ///
    /// 包括所有控制命令，如单点命令、设定值命令、总召唤等。
    #[inline]
    pub const fn is_command(self) -> bool {
        matches!(
            self,
            Self::CScNa1
                | Self::CDcNa1
                | Self::CRcNa1
                | Self::CSeNa1
                | Self::CSeNb1
                | Self::CSeNc1
                | Self::CBoNa1
                | Self::CScTa1
                | Self::CDcTa1
                | Self::CRcTa1
                | Self::CSeTa1
                | Self::CSeTb1
                | Self::CSeTc1
                | Self::CBoTa1
                | Self::CIcNa1
                | Self::CCiNa1
                | Self::CRdNa1
                | Self::CCsNa1
                | Self::CTsNa1
                | Self::CRpNa1
                | Self::CTsTa1
        )
    }

    /// 判断是否为参数类型（P_* 系列）
    ///
    /// 包括参数传输和参数激活相关类型。
    #[inline]
    pub const fn is_parameter(self) -> bool {
        matches!(self, Self::PMeNa1 | Self::PMeNb1 | Self::PMeNc1 | Self::PAcNa1)
    }

    /// 判断是否为文件传输类型（F_* 系列）
    ///
    /// 包括所有文件操作相关类型。
    #[inline]
    pub const fn is_file_transfer(self) -> bool {
        matches!(
            self,
            Self::FFrNa1
                | Self::FSrNa1
                | Self::FScNa1
                | Self::FLsNa1
                | Self::FAfNa1
                | Self::FSgNa1
                | Self::FDrTa1
                | Self::FScNb1
        )
    }

    /// 判断是否为安全/维护相关类型（S_* 系列）
    ///
    /// 包括认证、密钥管理等安全相关类型。
    #[inline]
    pub const fn is_security(self) -> bool {
        matches!(
            self,
            Self::SChNa1
                | Self::SRpNa1
                | Self::SArNa1
                | Self::SKrNa1
                | Self::SKsNa1
                | Self::SKcNa1
                | Self::SErNa1
                | Self::SUsNa1
                | Self::SUqNa1
                | Self::SUrNa1
                | Self::SUkNa1
                | Self::SUaNa1
                | Self::SUcNa1
        )
    }

    /// 判断是否为系统事件类型（M_EI_NA_1）
    ///
    /// 目前仅包括初始化结束事件。
    #[inline]
    pub const fn is_system(self) -> bool {
        matches!(self, Self::MEiNa1)
    }
}

impl TryFrom<u8> for TypeIdEnum {
    type Error = ();

    /// 从原始 u8 值转换为 TypeId 枚举
    ///
    /// 对于标准定义的 1-127 范围内的值，返回对应的枚举变体；
    /// 对于非标准值（如 0、128-255），返回 `Unknown(value)`。
    ///
    /// 注意：此方法总是返回 `Ok`，即使值不在标准范围内。
    fn try_from(value: u8) -> Result<Self, Self::Error> {
        let ti = match value {
            1 => Self::MSpNa1,
            2 => Self::MSpTa1,
            3 => Self::MDpNa1,
            4 => Self::MDpTa1,
            5 => Self::MStNa1,
            6 => Self::MStTa1,
            7 => Self::MBoNa1,
            8 => Self::MBoTa1,
            9 => Self::MMeNa1,
            10 => Self::MMeTa1,
            11 => Self::MMeNb1,
            12 => Self::MMeTb1,
            13 => Self::MMeNc1,
            14 => Self::MMeTc1,
            15 => Self::MItNa1,
            16 => Self::MItTa1,
            17 => Self::MEpTa1,
            18 => Self::MEpTb1,
            19 => Self::MEpTc1,
            20 => Self::MPsNa1,
            21 => Self::MMeNd1,
            30 => Self::MSpTb1,
            31 => Self::MDpTb1,
            32 => Self::MStTb1,
            33 => Self::MBoTb1,
            34 => Self::MMeTd1,
            35 => Self::MMeTe1,
            36 => Self::MMeTf1,
            37 => Self::MItTb1,
            38 => Self::MEpTd1,
            39 => Self::MEpTe1,
            40 => Self::MEpTf1,
            41 => Self::SItTc1,
            45 => Self::CScNa1,
            46 => Self::CDcNa1,
            47 => Self::CRcNa1,
            48 => Self::CSeNa1,
            49 => Self::CSeNb1,
            50 => Self::CSeNc1,
            51 => Self::CBoNa1,
            58 => Self::CScTa1,
            59 => Self::CDcTa1,
            60 => Self::CRcTa1,
            61 => Self::CSeTa1,
            62 => Self::CSeTb1,
            63 => Self::CSeTc1,
            64 => Self::CBoTa1,
            70 => Self::MEiNa1,
            81 => Self::SChNa1,
            82 => Self::SRpNa1,
            83 => Self::SArNa1,
            84 => Self::SKrNa1,
            85 => Self::SKsNa1,
            86 => Self::SKcNa1,
            87 => Self::SErNa1,
            90 => Self::SUsNa1,
            91 => Self::SUqNa1,
            92 => Self::SUrNa1,
            93 => Self::SUkNa1,
            94 => Self::SUaNa1,
            95 => Self::SUcNa1,
            100 => Self::CIcNa1,
            101 => Self::CCiNa1,
            102 => Self::CRdNa1,
            103 => Self::CCsNa1,
            104 => Self::CTsNa1,
            105 => Self::CRpNa1,
            107 => Self::CTsTa1,
            110 => Self::PMeNa1,
            111 => Self::PMeNb1,
            112 => Self::PMeNc1,
            113 => Self::PAcNa1,
            120 => Self::FFrNa1,
            121 => Self::FSrNa1,
            122 => Self::FScNa1,
            123 => Self::FLsNa1,
            124 => Self::FAfNa1,
            125 => Self::FSgNa1,
            126 => Self::FDrTa1,
            127 => Self::FScNb1,
            other => Self::Unknown(other),
        };
        Ok(ti)
    }
}

impl From<TypeIdEnum> for u8 {
    #[inline]
    fn from(value: TypeIdEnum) -> Self {
        value.to_u8()
    }
}

impl fmt::Display for TypeIdEnum {
    /// 格式化为 IEC 60870-5-104 标准名称
    ///
    /// 标准类型显示为协议定义的名称（如 "M_SP_NA_1"），
    /// 非标准类型显示为 "Unknown(value)"。
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let name = match self {
            Self::MSpNa1 => "M_SP_NA_1",
            Self::MSpTa1 => "M_SP_TA_1",
            Self::MDpNa1 => "M_DP_NA_1",
            Self::MDpTa1 => "M_DP_TA_1",
            Self::MStNa1 => "M_ST_NA_1",
            Self::MStTa1 => "M_ST_TA_1",
            Self::MBoNa1 => "M_BO_NA_1",
            Self::MBoTa1 => "M_BO_TA_1",
            Self::MMeNa1 => "M_ME_NA_1",
            Self::MMeTa1 => "M_ME_TA_1",
            Self::MMeNb1 => "M_ME_NB_1",
            Self::MMeTb1 => "M_ME_TB_1",
            Self::MMeNc1 => "M_ME_NC_1",
            Self::MMeTc1 => "M_ME_TC_1",
            Self::MItNa1 => "M_IT_NA_1",
            Self::MItTa1 => "M_IT_TA_1",
            Self::MEpTa1 => "M_EP_TA_1",
            Self::MEpTb1 => "M_EP_TB_1",
            Self::MEpTc1 => "M_EP_TC_1",
            Self::MPsNa1 => "M_PS_NA_1",
            Self::MMeNd1 => "M_ME_ND_1",
            Self::MSpTb1 => "M_SP_TB_1",
            Self::MDpTb1 => "M_DP_TB_1",
            Self::MStTb1 => "M_ST_TB_1",
            Self::MBoTb1 => "M_BO_TB_1",
            Self::MMeTd1 => "M_ME_TD_1",
            Self::MMeTe1 => "M_ME_TE_1",
            Self::MMeTf1 => "M_ME_TF_1",
            Self::MItTb1 => "M_IT_TB_1",
            Self::MEpTd1 => "M_EP_TD_1",
            Self::MEpTe1 => "M_EP_TE_1",
            Self::MEpTf1 => "M_EP_TF_1",
            Self::SItTc1 => "S_IT_TC_1",
            Self::CScNa1 => "C_SC_NA_1",
            Self::CDcNa1 => "C_DC_NA_1",
            Self::CRcNa1 => "C_RC_NA_1",
            Self::CSeNa1 => "C_SE_NA_1",
            Self::CSeNb1 => "C_SE_NB_1",
            Self::CSeNc1 => "C_SE_NC_1",
            Self::CBoNa1 => "C_BO_NA_1",
            Self::CScTa1 => "C_SC_TA_1",
            Self::CDcTa1 => "C_DC_TA_1",
            Self::CRcTa1 => "C_RC_TA_1",
            Self::CSeTa1 => "C_SE_TA_1",
            Self::CSeTb1 => "C_SE_TB_1",
            Self::CSeTc1 => "C_SE_TC_1",
            Self::CBoTa1 => "C_BO_TA_1",
            Self::MEiNa1 => "M_EI_NA_1",
            Self::SChNa1 => "S_CH_NA_1",
            Self::SRpNa1 => "S_RP_NA_1",
            Self::SArNa1 => "S_AR_NA_1",
            Self::SKrNa1 => "S_KR_NA_1",
            Self::SKsNa1 => "S_KS_NA_1",
            Self::SKcNa1 => "S_KC_NA_1",
            Self::SErNa1 => "S_ER_NA_1",
            Self::SUsNa1 => "S_US_NA_1",
            Self::SUqNa1 => "S_UQ_NA_1",
            Self::SUrNa1 => "S_UR_NA_1",
            Self::SUkNa1 => "S_UK_NA_1",
            Self::SUaNa1 => "S_UA_NA_1",
            Self::SUcNa1 => "S_UC_NA_1",
            Self::CIcNa1 => "C_IC_NA_1",
            Self::CCiNa1 => "C_CI_NA_1",
            Self::CRdNa1 => "C_RD_NA_1",
            Self::CCsNa1 => "C_CS_NA_1",
            Self::CTsNa1 => "C_TS_NA_1",
            Self::CRpNa1 => "C_RP_NA_1",
            Self::CTsTa1 => "C_TS_TA_1",
            Self::PMeNa1 => "P_ME_NA_1",
            Self::PMeNb1 => "P_ME_NB_1",
            Self::PMeNc1 => "P_ME_NC_1",
            Self::PAcNa1 => "P_AC_NA_1",
            Self::FFrNa1 => "F_FR_NA_1",
            Self::FSrNa1 => "F_SR_NA_1",
            Self::FScNa1 => "F_SC_NA_1",
            Self::FLsNa1 => "F_LS_NA_1",
            Self::FAfNa1 => "F_AF_NA_1",
            Self::FSgNa1 => "F_SG_NA_1",
            Self::FDrTa1 => "F_DR_TA_1",
            Self::FScNb1 => "F_SC_NB_1",
            Self::Unknown(v) => return write!(f, "Unknown({})", v),
        };
        f.write_str(name)
    }
}

// ============================================================================
// TypeDescriptor - ASDU 解析策略描述
// ============================================================================

/// 描述某个 TI 的解析策略。
#[derive(Clone, Copy)]
pub struct TypeDescriptor {
    /// 类型标识。
    pub type_id: TypeId,
    /// 类型名称（如 `M_SP_NA_1`），仅用于调试/文档。
    pub name: &'static str,
    /// IOA 长度（0~3 字节）。
    pub ioa_len: usize,
    /// 信息体主体长度；若为 `None` 代表长度可变，需要专用 parser。
    pub data_len: Option<usize>,
    /// 可选时间戳长度（CP24/CP56）。
    pub timestamp_len: Option<usize>,
    pub category: AsduCategory,
    pub family: InformationFamily,
    pub value_kind: ValueKind,
    pub quality_kind: QualityKind,
    pub qualifier_kind: QualifierKind,
    pub timestamp_kind: TimestampKind,
}

impl TypeDescriptor {
    /// 构造默认 descriptor，默认 IOA=3/数据长度未知/无时间戳。
    pub const fn new(type_id: TypeId, name: &'static str) -> Self {
        let raw = type_id.raw();
        Self {
            type_id,
            name,
            ioa_len: 3,
            data_len: None,
            timestamp_len: None,
            category: category_for_type(raw),
            family: family_for_type(raw),
            value_kind: value_kind_for_type(raw),
            quality_kind: quality_kind_for_type(raw),
            qualifier_kind: qualifier_kind_for_type(raw),
            timestamp_kind: TimestampKind::None,
        }
    }

    /// 指定 IOA 长度。
    pub const fn with_ioa_len(mut self, len: usize) -> Self {
        self.ioa_len = len;
        self
    }

    /// 指定数据长度（或 `None` 表示可变）。
    pub const fn with_data_len(mut self, len: Option<usize>) -> Self {
        self.data_len = len;
        self
    }

    /// 指定时间戳长度。
    pub const fn with_timestamp(mut self, len: Option<usize>) -> Self {
        self.timestamp_len = len;
        self.timestamp_kind = match len {
            Some(3) => TimestampKind::Cp24,
            Some(7) => TimestampKind::Cp56,
            _ => TimestampKind::None,
        };
        self
    }

    /// 计算“单条信息体”的总长度（IOA + data + timestamp）。
    /// 对于可变长度（部分安全/文件类型）返回 `None`。
    pub const fn entry_len(&self) -> Option<usize> {
        match self.data_len {
            Some(data) => {
                let ts = match self.timestamp_len {
                    Some(len) => len,
                    None => 0,
                };
                Some(self.ioa_len + data + ts)
            }
            None => None,
        }
    }
}

const fn category_for_type(type_id: u8) -> AsduCategory {
    match type_id {
        1..=16 | 20..=21 | 30..=37 | 41 => AsduCategory::Measurement,
        17..=19 | 38..=40 => AsduCategory::Protection,
        45..=64 => AsduCategory::Control,
        70 => AsduCategory::Initialization,
        81..=95 => AsduCategory::Security,
        100..=107 => AsduCategory::System,
        110..=113 => AsduCategory::Parameter,
        120..=127 => AsduCategory::FileTransfer,
        _ => AsduCategory::System,
    }
}

const fn family_for_type(type_id: u8) -> InformationFamily {
    match type_id {
        1 | 2 | 30 | 45 | 58 => InformationFamily::SinglePoint,
        3 | 4 | 31 | 46 | 59 => InformationFamily::DoublePoint,
        5 | 6 | 32 | 47 | 60 => InformationFamily::StepPosition,
        7 | 8 | 33 | 51 | 64 => InformationFamily::BitString,
        9 | 10 | 21 | 34 | 48 | 61 | 110 => InformationFamily::Normalized,
        11 | 12 | 35 | 49 | 62 | 111 => InformationFamily::Scaled,
        13 | 14 | 36 | 50 | 63 | 112 => InformationFamily::ShortFloat,
        15 | 16 | 37 | 41 => InformationFamily::IntegratedTotal,
        17..=19 | 38..=40 => InformationFamily::Protection,
        20 => InformationFamily::PackedStatus,
        45..=64 => InformationFamily::Control,
        70 => InformationFamily::Initialization,
        81..=95 => InformationFamily::Security,
        100..=107 => InformationFamily::SystemCommand,
        110..=113 => InformationFamily::Parameter,
        120..=127 => InformationFamily::FileTransfer,
        _ => InformationFamily::SystemCommand,
    }
}

const fn value_kind_for_type(type_id: u8) -> ValueKind {
    match family_for_type(type_id) {
        InformationFamily::SinglePoint => ValueKind::Boolean,
        InformationFamily::DoublePoint => ValueKind::DoublePoint,
        InformationFamily::StepPosition => ValueKind::StepPosition,
        InformationFamily::BitString => ValueKind::BitString32,
        InformationFamily::Normalized => ValueKind::Normalized,
        InformationFamily::Scaled => ValueKind::ScaledI16,
        InformationFamily::ShortFloat => ValueKind::Float32,
        InformationFamily::IntegratedTotal => ValueKind::Counter,
        InformationFamily::Protection | InformationFamily::PackedStatus => ValueKind::Structured,
        _ => ValueKind::None,
    }
}

const fn quality_kind_for_type(type_id: u8) -> QualityKind {
    match type_id {
        1 | 2 | 30 => QualityKind::Siq,
        3 | 4 | 31 => QualityKind::Diq,
        5..=14 | 20 | 32..=36 => QualityKind::Qds,
        15 | 16 | 37 => QualityKind::Bcr,
        17..=19 | 38..=40 => QualityKind::Qdp,
        _ => QualityKind::None,
    }
}

const fn qualifier_kind_for_type(type_id: u8) -> QualifierKind {
    match type_id {
        45..=47 | 51 | 58..=60 | 64 => QualifierKind::Command,
        48..=50 | 61..=63 => QualifierKind::Setpoint,
        100 => QualifierKind::Interrogation,
        101 => QualifierKind::CounterInterrogation,
        105 => QualifierKind::ResetProcess,
        110..=113 => QualifierKind::Parameter,
        120..=127 => QualifierKind::FileTransfer,
        _ => QualifierKind::None,
    }
}

/// 辅助函数：创建完整的 `TypeDescriptor`（使用类型安全的枚举）。
///
/// 此函数在编译期将枚举转换为 u8 值，提供类型安全的表构建。
const fn descriptor_enum(
    kind: TypeIdEnum,
    name: &'static str,
    ioa_len: usize,
    obj_len: Option<usize>,
    ts_len: Option<usize>,
) -> TypeDescriptor {
    TypeDescriptor::new(TypeId::new(kind.to_u8()), name)
        .with_ioa_len(ioa_len)
        .with_data_len(obj_len)
        .with_timestamp(ts_len)
}

const fn build_descriptor_table() -> [Option<TypeDescriptor>; 128] {
    let mut table: [Option<TypeDescriptor>; 128] = [None; 128];
    table[TypeIdEnum::MSpNa1.to_u8() as usize] =
        Some(descriptor_enum(TypeIdEnum::MSpNa1, "M_SP_NA_1", 3, Some(1), None));
    table[TypeIdEnum::MSpTa1.to_u8() as usize] =
        Some(descriptor_enum(TypeIdEnum::MSpTa1, "M_SP_TA_1", 3, Some(1), Some(3)));
    table[TypeIdEnum::MDpNa1.to_u8() as usize] =
        Some(descriptor_enum(TypeIdEnum::MDpNa1, "M_DP_NA_1", 3, Some(1), None));
    table[TypeIdEnum::MDpTa1.to_u8() as usize] =
        Some(descriptor_enum(TypeIdEnum::MDpTa1, "M_DP_TA_1", 3, Some(1), Some(3)));
    table[TypeIdEnum::MStNa1.to_u8() as usize] =
        Some(descriptor_enum(TypeIdEnum::MStNa1, "M_ST_NA_1", 3, Some(2), None));
    table[TypeIdEnum::MStTa1.to_u8() as usize] =
        Some(descriptor_enum(TypeIdEnum::MStTa1, "M_ST_TA_1", 3, Some(2), Some(3)));
    table[TypeIdEnum::MBoNa1.to_u8() as usize] =
        Some(descriptor_enum(TypeIdEnum::MBoNa1, "M_BO_NA_1", 3, Some(5), None));
    table[TypeIdEnum::MBoTa1.to_u8() as usize] =
        Some(descriptor_enum(TypeIdEnum::MBoTa1, "M_BO_TA_1", 3, Some(5), Some(3)));
    table[TypeIdEnum::MMeNa1.to_u8() as usize] =
        Some(descriptor_enum(TypeIdEnum::MMeNa1, "M_ME_NA_1", 3, Some(3), None));
    table[TypeIdEnum::MMeTa1.to_u8() as usize] =
        Some(descriptor_enum(TypeIdEnum::MMeTa1, "M_ME_TA_1", 3, Some(3), Some(3)));
    table[TypeIdEnum::MMeNb1.to_u8() as usize] =
        Some(descriptor_enum(TypeIdEnum::MMeNb1, "M_ME_NB_1", 3, Some(3), None));
    table[TypeIdEnum::MMeTb1.to_u8() as usize] =
        Some(descriptor_enum(TypeIdEnum::MMeTb1, "M_ME_TB_1", 3, Some(3), Some(3)));
    table[TypeIdEnum::MMeNc1.to_u8() as usize] =
        Some(descriptor_enum(TypeIdEnum::MMeNc1, "M_ME_NC_1", 3, Some(5), None));
    table[TypeIdEnum::MMeTc1.to_u8() as usize] =
        Some(descriptor_enum(TypeIdEnum::MMeTc1, "M_ME_TC_1", 3, Some(5), Some(3)));
    table[TypeIdEnum::MItNa1.to_u8() as usize] =
        Some(descriptor_enum(TypeIdEnum::MItNa1, "M_IT_NA_1", 3, Some(5), None));
    table[TypeIdEnum::MItTa1.to_u8() as usize] =
        Some(descriptor_enum(TypeIdEnum::MItTa1, "M_IT_TA_1", 3, Some(5), Some(3)));
    table[TypeIdEnum::MEpTa1.to_u8() as usize] =
        Some(descriptor_enum(TypeIdEnum::MEpTa1, "M_EP_TA_1", 3, Some(3), Some(3)));
    table[TypeIdEnum::MEpTb1.to_u8() as usize] =
        Some(descriptor_enum(TypeIdEnum::MEpTb1, "M_EP_TB_1", 3, Some(4), Some(3)));
    table[TypeIdEnum::MEpTc1.to_u8() as usize] =
        Some(descriptor_enum(TypeIdEnum::MEpTc1, "M_EP_TC_1", 3, Some(4), Some(3)));
    table[TypeIdEnum::MPsNa1.to_u8() as usize] =
        Some(descriptor_enum(TypeIdEnum::MPsNa1, "M_PS_NA_1", 3, Some(5), None));
    table[TypeIdEnum::MMeNd1.to_u8() as usize] =
        Some(descriptor_enum(TypeIdEnum::MMeNd1, "M_ME_ND_1", 3, Some(2), None));
    table[TypeIdEnum::MSpTb1.to_u8() as usize] =
        Some(descriptor_enum(TypeIdEnum::MSpTb1, "M_SP_TB_1", 3, Some(1), Some(7)));
    table[TypeIdEnum::MDpTb1.to_u8() as usize] =
        Some(descriptor_enum(TypeIdEnum::MDpTb1, "M_DP_TB_1", 3, Some(1), Some(7)));
    table[TypeIdEnum::MStTb1.to_u8() as usize] =
        Some(descriptor_enum(TypeIdEnum::MStTb1, "M_ST_TB_1", 3, Some(2), Some(7)));
    table[TypeIdEnum::MBoTb1.to_u8() as usize] =
        Some(descriptor_enum(TypeIdEnum::MBoTb1, "M_BO_TB_1", 3, Some(5), Some(7)));
    table[TypeIdEnum::MMeTd1.to_u8() as usize] =
        Some(descriptor_enum(TypeIdEnum::MMeTd1, "M_ME_TD_1", 3, Some(3), Some(7)));
    table[TypeIdEnum::MMeTe1.to_u8() as usize] =
        Some(descriptor_enum(TypeIdEnum::MMeTe1, "M_ME_TE_1", 3, Some(3), Some(7)));
    table[TypeIdEnum::MMeTf1.to_u8() as usize] =
        Some(descriptor_enum(TypeIdEnum::MMeTf1, "M_ME_TF_1", 3, Some(5), Some(7)));
    table[TypeIdEnum::MItTb1.to_u8() as usize] =
        Some(descriptor_enum(TypeIdEnum::MItTb1, "M_IT_TB_1", 3, Some(5), Some(7)));
    table[TypeIdEnum::MEpTd1.to_u8() as usize] =
        Some(descriptor_enum(TypeIdEnum::MEpTd1, "M_EP_TD_1", 3, Some(3), Some(7)));
    table[TypeIdEnum::MEpTe1.to_u8() as usize] =
        Some(descriptor_enum(TypeIdEnum::MEpTe1, "M_EP_TE_1", 3, Some(4), Some(7)));
    table[TypeIdEnum::MEpTf1.to_u8() as usize] =
        Some(descriptor_enum(TypeIdEnum::MEpTf1, "M_EP_TF_1", 3, Some(4), Some(7)));
    table[TypeIdEnum::SItTc1.to_u8() as usize] =
        Some(descriptor_enum(TypeIdEnum::SItTc1, "S_IT_TC_1", 0, Some(7), Some(7)));
    table[TypeIdEnum::CScNa1.to_u8() as usize] =
        Some(descriptor_enum(TypeIdEnum::CScNa1, "C_SC_NA_1", 3, Some(1), None));
    table[TypeIdEnum::CDcNa1.to_u8() as usize] =
        Some(descriptor_enum(TypeIdEnum::CDcNa1, "C_DC_NA_1", 3, Some(1), None));
    table[TypeIdEnum::CRcNa1.to_u8() as usize] =
        Some(descriptor_enum(TypeIdEnum::CRcNa1, "C_RC_NA_1", 3, Some(1), None));
    table[TypeIdEnum::CSeNa1.to_u8() as usize] =
        Some(descriptor_enum(TypeIdEnum::CSeNa1, "C_SE_NA_1", 3, Some(3), None));
    table[TypeIdEnum::CSeNb1.to_u8() as usize] =
        Some(descriptor_enum(TypeIdEnum::CSeNb1, "C_SE_NB_1", 3, Some(3), None));
    table[TypeIdEnum::CSeNc1.to_u8() as usize] =
        Some(descriptor_enum(TypeIdEnum::CSeNc1, "C_SE_NC_1", 3, Some(5), None));
    table[TypeIdEnum::CBoNa1.to_u8() as usize] =
        Some(descriptor_enum(TypeIdEnum::CBoNa1, "C_BO_NA_1", 3, Some(4), None));
    table[TypeIdEnum::CScTa1.to_u8() as usize] =
        Some(descriptor_enum(TypeIdEnum::CScTa1, "C_SC_TA_1", 3, Some(1), Some(7)));
    table[TypeIdEnum::CDcTa1.to_u8() as usize] =
        Some(descriptor_enum(TypeIdEnum::CDcTa1, "C_DC_TA_1", 3, Some(1), Some(7)));
    table[TypeIdEnum::CRcTa1.to_u8() as usize] =
        Some(descriptor_enum(TypeIdEnum::CRcTa1, "C_RC_TA_1", 3, Some(1), Some(7)));
    table[TypeIdEnum::CSeTa1.to_u8() as usize] =
        Some(descriptor_enum(TypeIdEnum::CSeTa1, "C_SE_TA_1", 3, Some(3), Some(7)));
    table[TypeIdEnum::CSeTb1.to_u8() as usize] =
        Some(descriptor_enum(TypeIdEnum::CSeTb1, "C_SE_TB_1", 3, Some(3), Some(7)));
    table[TypeIdEnum::CSeTc1.to_u8() as usize] =
        Some(descriptor_enum(TypeIdEnum::CSeTc1, "C_SE_TC_1", 3, Some(5), Some(7)));
    table[TypeIdEnum::CBoTa1.to_u8() as usize] =
        Some(descriptor_enum(TypeIdEnum::CBoTa1, "C_BO_TA_1", 3, Some(4), Some(7)));
    table[TypeIdEnum::MEiNa1.to_u8() as usize] =
        Some(descriptor_enum(TypeIdEnum::MEiNa1, "M_EI_NA_1", 0, Some(1), None));
    table[TypeIdEnum::SChNa1.to_u8() as usize] = Some(descriptor_enum(TypeIdEnum::SChNa1, "S_CH_NA_1", 0, None, None));
    table[TypeIdEnum::SRpNa1.to_u8() as usize] = Some(descriptor_enum(TypeIdEnum::SRpNa1, "S_RP_NA_1", 0, None, None));
    table[TypeIdEnum::SArNa1.to_u8() as usize] = Some(descriptor_enum(TypeIdEnum::SArNa1, "S_AR_NA_1", 0, None, None));
    table[TypeIdEnum::SKrNa1.to_u8() as usize] = Some(descriptor_enum(TypeIdEnum::SKrNa1, "S_KR_NA_1", 0, None, None));
    table[TypeIdEnum::SKsNa1.to_u8() as usize] = Some(descriptor_enum(TypeIdEnum::SKsNa1, "S_KS_NA_1", 0, None, None));
    table[TypeIdEnum::SKcNa1.to_u8() as usize] = Some(descriptor_enum(TypeIdEnum::SKcNa1, "S_KC_NA_1", 0, None, None));
    table[TypeIdEnum::SErNa1.to_u8() as usize] = Some(descriptor_enum(TypeIdEnum::SErNa1, "S_ER_NA_1", 0, None, None));
    table[TypeIdEnum::SUsNa1.to_u8() as usize] = Some(descriptor_enum(TypeIdEnum::SUsNa1, "S_US_NA_1", 0, None, None));
    table[TypeIdEnum::SUqNa1.to_u8() as usize] = Some(descriptor_enum(TypeIdEnum::SUqNa1, "S_UQ_NA_1", 0, None, None));
    table[TypeIdEnum::SUrNa1.to_u8() as usize] = Some(descriptor_enum(TypeIdEnum::SUrNa1, "S_UR_NA_1", 0, None, None));
    table[TypeIdEnum::SUkNa1.to_u8() as usize] = Some(descriptor_enum(TypeIdEnum::SUkNa1, "S_UK_NA_1", 0, None, None));
    table[TypeIdEnum::SUaNa1.to_u8() as usize] = Some(descriptor_enum(TypeIdEnum::SUaNa1, "S_UA_NA_1", 0, None, None));
    table[TypeIdEnum::SUcNa1.to_u8() as usize] = Some(descriptor_enum(TypeIdEnum::SUcNa1, "S_UC_NA_1", 0, None, None));
    table[TypeIdEnum::CIcNa1.to_u8() as usize] =
        Some(descriptor_enum(TypeIdEnum::CIcNa1, "C_IC_NA_1", 3, Some(1), None));
    table[TypeIdEnum::CCiNa1.to_u8() as usize] =
        Some(descriptor_enum(TypeIdEnum::CCiNa1, "C_CI_NA_1", 3, Some(1), None));
    table[TypeIdEnum::CRdNa1.to_u8() as usize] =
        Some(descriptor_enum(TypeIdEnum::CRdNa1, "C_RD_NA_1", 3, Some(0), None));
    table[TypeIdEnum::CCsNa1.to_u8() as usize] =
        Some(descriptor_enum(TypeIdEnum::CCsNa1, "C_CS_NA_1", 3, Some(0), Some(7)));
    // C_TS_NA_1 (104) 测试命令: IOA(3) + FBP(2)
    table[TypeIdEnum::CTsNa1.to_u8() as usize] =
        Some(descriptor_enum(TypeIdEnum::CTsNa1, "C_TS_NA_1", 3, Some(2), None));
    table[TypeIdEnum::CRpNa1.to_u8() as usize] =
        Some(descriptor_enum(TypeIdEnum::CRpNa1, "C_RP_NA_1", 3, Some(1), None));
    table[TypeIdEnum::CTsTa1.to_u8() as usize] =
        Some(descriptor_enum(TypeIdEnum::CTsTa1, "C_TS_TA_1", 3, Some(2), Some(7)));
    table[TypeIdEnum::PMeNa1.to_u8() as usize] =
        Some(descriptor_enum(TypeIdEnum::PMeNa1, "P_ME_NA_1", 3, Some(3), None));
    table[TypeIdEnum::PMeNb1.to_u8() as usize] =
        Some(descriptor_enum(TypeIdEnum::PMeNb1, "P_ME_NB_1", 3, Some(3), None));
    table[TypeIdEnum::PMeNc1.to_u8() as usize] =
        Some(descriptor_enum(TypeIdEnum::PMeNc1, "P_ME_NC_1", 3, Some(5), None));
    table[TypeIdEnum::PAcNa1.to_u8() as usize] =
        Some(descriptor_enum(TypeIdEnum::PAcNa1, "P_AC_NA_1", 3, Some(1), None));
    // 文件传输类型 (TI 120-127)
    // 格式: descriptor_enum(type, name, ioa_len, data_len, timestamp_len)
    // 注意: data_len 是信息体主体长度(不含IOA)，entry_len会自动计算为 ioa_len + data_len +
    // timestamp_len
    table[TypeIdEnum::FFrNa1.to_u8() as usize] =
        Some(descriptor_enum(TypeIdEnum::FFrNa1, "F_FR_NA_1", 3, Some(6), None)); // Body: FRQ(1) + NOF(2) + LOF(3)
    table[TypeIdEnum::FSrNa1.to_u8() as usize] =
        Some(descriptor_enum(TypeIdEnum::FSrNa1, "F_SR_NA_1", 3, Some(7), None)); // Body: SRQ(1) + NOF(2) + NOS(1) + LOF(3)
    table[TypeIdEnum::FScNa1.to_u8() as usize] =
        Some(descriptor_enum(TypeIdEnum::FScNa1, "F_SC_NA_1", 3, Some(4), None)); // Body: SCQ(1) + NOF(2) + NOS(1)
    table[TypeIdEnum::FLsNa1.to_u8() as usize] =
        Some(descriptor_enum(TypeIdEnum::FLsNa1, "F_LS_NA_1", 3, Some(5), None)); // Body: LSQ(1) + NOF(2) + last_section(2)
    table[TypeIdEnum::FAfNa1.to_u8() as usize] =
        Some(descriptor_enum(TypeIdEnum::FAfNa1, "F_AF_NA_1", 3, Some(4), None)); // Body: AFQ(1) + NOF(2) + NOS(1)
    table[TypeIdEnum::FSgNa1.to_u8() as usize] = Some(descriptor_enum(TypeIdEnum::FSgNa1, "F_SG_NA_1", 3, None, None)); // Body: 可变长度段数据
    table[TypeIdEnum::FDrTa1.to_u8() as usize] =
        Some(descriptor_enum(TypeIdEnum::FDrTa1, "F_DR_TA_1", 3, Some(6), Some(7))); // Body: NOF(2) + LOF(3) + SOF(1), Timestamp: CP56Time2a(7)
    table[TypeIdEnum::FScNb1.to_u8() as usize] =
        Some(descriptor_enum(TypeIdEnum::FScNb1, "F_SC_NB_1", 3, Some(16), None)); // Body: NOF(2) + CP56Time2a(7) + CP56Time2a(7)

    table
}

/// 编译期生成的 TypeDescriptor 查找表。
pub static TYPE_DESCRIPTOR_TABLE: [Option<TypeDescriptor>; 128] = build_descriptor_table();

/// 根据 TI 查找解析策略。
pub fn lookup_type_descriptor(ti: TypeId) -> Option<&'static TypeDescriptor> {
    TYPE_DESCRIPTOR_TABLE[ti.raw() as usize].as_ref()
}

/// 根据枚举类型查找解析策略。
///
/// 提供类型安全的查找接口，避免使用魔数。
///
/// # 参数
///
/// - `kind`: 枚举类型的 TypeId。标准枚举值（如 `TypeIdEnum::CScNa1`）总是有效； 对于
///   `TypeIdEnum::Unknown(v)` 变体，要求 `v` 在 1..=127 范围内， 超出范围将返回 `None`。
///
/// # 示例
///
/// ```
/// # use iec60870_parser::parser::asdu::{TypeIdEnum, lookup_type_descriptor_by_kind};
/// let desc = lookup_type_descriptor_by_kind(TypeIdEnum::CScNa1);
/// assert!(desc.is_some());
/// assert_eq!(desc.unwrap().name, "C_SC_NA_1");
///
/// // 非标准但在范围内的值
/// let desc = lookup_type_descriptor_by_kind(TypeIdEnum::Unknown(22));
/// assert!(desc.is_none()); // 未注册的间隙值
/// ```
pub fn lookup_type_descriptor_by_kind(kind: TypeIdEnum) -> Option<&'static TypeDescriptor> {
    let idx = kind.to_u8() as usize;
    if idx >= TYPE_DESCRIPTOR_TABLE.len() {
        return None;
    }
    TYPE_DESCRIPTOR_TABLE[idx].as_ref()
}
