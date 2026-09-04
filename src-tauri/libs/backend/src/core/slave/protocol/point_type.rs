//! 数据点类型分类与兼容性判断
//!
//! 本模块负责处理 IEC104 协议中的类型标识（Type ID）分类和兼容性判断逻辑。
//! 这是控制命令验证和数据点模拟的基础模块。
//!
//! # 核心功能
//!
//! - **类型分类**：将类型标识分类为单点、双点、测量值等不同类别
//! - **兼容性判断**：判断控制命令类型与监视点类型是否兼容
//! - **可模拟性判断**：判断监视点类型是否支持数据模拟
//!
//! # IEC104 类型标识体系
//!
//! IEC104 协议定义了多种类型标识，分为监视点类型和控制命令类型两大类：
//!
//! ## 监视点类型（Monitor Types）
//!
//! 监视点类型用于表示从站上报的数据点信息：
//!
//! ### 单点信息（Single Point）
//!
//! - **1 (M_SP_NA_1)**：不带时标的单点信息
//! - **2 (M_SP_TA_1)**：带时标的单点信息（CP24Time2a）
//! - **30 (M_SP_TB_1)**：带时标的单点信息（CP56Time2a）
//!
//! ### 双点信息（Double Point）
//!
//! - **3 (M_DP_NA_1)**：不带时标的双点信息
//! - **4 (M_DP_TA_1)**：带时标的双点信息（CP24Time2a）
//! - **31 (M_DP_TB_1)**：带时标的双点信息（CP56Time2a）
//!
//! ### 步位置信息（Step Position）
//!
//! - **5 (M_ST_NA_1)**：不带时标的步位置信息
//! - **6 (M_ST_TA_1)**：带时标的步位置信息（CP24Time2a）
//! - **32 (M_ST_TB_1)**：带时标的步位置信息（CP56Time2a）
//!
//! ### 位串信息（Bitstring）
//!
//! - **7 (M_BO_NA_1)**：不带时标的位串信息
//! - **8 (M_BO_TA_1)**：带时标的位串信息（CP24Time2a）
//! - **33 (M_BO_TB_1)**：带时标的位串信息（CP56Time2a）
//!
//! ### 归一化测量值（Normalized Measurement）
//!
//! - **9 (M_ME_NA_1)**：不带时标的归一化测量值
//! - **10 (M_ME_TA_1)**：带时标的归一化测量值（CP24Time2a）
//! - **34 (M_ME_TD_1)**：带时标的归一化测量值（CP56Time2a）
//!
//! ### 标度化测量值（Scaled Measurement）
//!
//! - **11 (M_ME_NB_1)**：不带时标的标度化测量值
//! - **12 (M_ME_TB_1)**：带时标的标度化测量值（CP24Time2a）
//! - **35 (M_ME_TE_1)**：带时标的标度化测量值（CP56Time2a）
//!
//! ### 短浮点测量值（Short Float Measurement）
//!
//! - **13 (M_ME_NC_1)**：不带时标的短浮点测量值
//! - **14 (M_ME_TC_1)**：带时标的短浮点测量值（CP24Time2a）
//! - **36 (M_ME_TF_1)**：带时标的短浮点测量值（CP56Time2a）
//!
//! ### 累计量（Integrated Total）
//!
//! - **15 (M_IT_NA_1)**：不带时标的累计量
//! - **16 (M_IT_TA_1)**：带时标的累计量（CP24Time2a）
//! - **37 (M_IT_TB_1)**：带时标的累计量（CP56Time2a）
//!
//! ## 控制命令类型（Command Types）
//!
//! 控制命令类型用于表示主站发送的控制指令：
//!
//! ### 单点控制（Single Command）
//!
//! - **45 (C_SC_NA_1)**：不带时标的单点控制
//! - **58 (C_SC_TA_1)**：带时标的单点控制（CP56Time2a）
//!
//! ### 双点控制（Double Command）
//!
//! - **46 (C_DC_NA_1)**：不带时标的双点控制
//! - **59 (C_DC_TA_1)**：带时标的双点控制（CP56Time2a）
//!
//! ### 步位置控制（Regulating Step Command）
//!
//! - **47 (C_RC_NA_1)**：不带时标的步位置控制
//! - **60 (C_RC_TA_1)**：带时标的步位置控制（CP56Time2a）
//!
//! ### 设定值控制（Setpoint Command）
//!
//! - **48 (C_SE_NA_1)**：不带时标的归一化设定值控制
//! - **49 (C_SE_NB_1)**：不带时标的标度化设定值控制
//! - **50 (C_SE_NC_1)**：不带时标的短浮点设定值控制
//! - **61 (C_SE_TA_1)**：带时标的归一化设定值控制（CP56Time2a）
//! - **62 (C_SE_TB_1)**：带时标的标度化设定值控制（CP56Time2a）
//! - **63 (C_SE_TC_1)**：带时标的短浮点设定值控制（CP56Time2a）
//!
//! ### 位串控制（Bitstring Command）
//!
//! - **51 (C_BO_NA_1)**：不带时标的位串控制
//! - **64 (C_BO_TA_1)**：带时标的位串控制（CP56Time2a）
//!
//! # 类型兼容性规则
//!
//! 控制命令类型与监视点类型的兼容性遵循以下规则：
//!
//! | 控制命令类型 | 兼容的监视点类型 | 说明 |
//! |------------|----------------|------|
//! | 45, 58 (单点控制) | 1, 2, 30 (单点信息) | 控制单点开关 |
//! | 46, 59 (双点控制) | 3, 4, 31 (双点信息) | 控制双点开关 |
//! | 47, 60 (步位置控制) | 5, 6, 32 (步位置信息) | 控制步进电机 |
//! | 48, 61 (归一化设定值) | 9, 10, 34 (归一化测量值) | 设置归一化值 |
//! | 49, 62 (标度化设定值) | 11, 12, 35 (标度化测量值) | 设置标度化值 |
//! | 50, 63 (短浮点设定值) | 13, 14, 36 (短浮点测量值) | 设置浮点值 |
//! | 51, 64 (位串控制) | 7, 8, 33 (位串信息) | 控制位串 |
//!
//! # 可模拟类型
//!
//! 从站模拟功能支持以下监视点类型的数据生成：
//!
//! - 单点信息（1, 2, 30）
//! - 双点信息（3, 4, 31）
//! - 步位置信息（5, 6, 32）
//! - 位串信息（7, 8, 33）
//! - 归一化测量值（9, 10, 34）
//! - 标度化测量值（11, 12, 35）
//! - 短浮点测量值（13, 14, 36）
//! - 累计量（15, 16, 37）
//!
//! **注意**：控制命令类型（45-51, 58-64）不支持模拟，因为它们是主站发送的命令，不是从站上报的数据。
//!
//! # 使用示例
//!
//! ```rust
//! use crate::core::slave::protocol::point_type::{
//!     is_command_type_compatible_with_monitor_type, is_simulatable_monitor_type,
//!     is_single_point_monitor_type,
//! };
//!
//! // 检查类型兼容性
//! let command_type = 45; // C_SC_NA_1 (单点控制)
//! let monitor_type = 1; // M_SP_NA_1 (单点信息)
//! assert!(is_command_type_compatible_with_monitor_type(command_type, monitor_type));
//!
//! // 检查类型不兼容
//! let command_type = 46; // C_DC_NA_1 (双点控制)
//! let monitor_type = 1; // M_SP_NA_1 (单点信息)
//! assert!(!is_command_type_compatible_with_monitor_type(command_type, monitor_type));
//!
//! // 检查是否可模拟
//! assert!(is_simulatable_monitor_type(1)); // 单点信息可模拟
//! assert!(!is_simulatable_monitor_type(45)); // 单点控制不可模拟
//!
//! // 检查类型分类
//! assert!(is_single_point_monitor_type(1)); // M_SP_NA_1 是单点信息
//! assert!(is_single_point_monitor_type(30)); // M_SP_TB_1 也是单点信息
//! ```
//!
//! # 线程安全
//!
//! 本模块中的所有函数都是纯函数，不涉及共享状态，可以安全地在多线程环境中调用。

/// 判断控制命令类型与监视点类型是否兼容
///
/// 检查控制命令类型标识与监视点类型标识是否属于同一类别，从而判断控制命令是否可以作用于该监视点。
/// 这是控制目标验证的核心函数，用于实现类型安全的控制命令处理。
///
/// # 参数
///
/// * `command_type_id` - 控制命令的类型标识（45-51, 58-64）
/// * `monitor_type_id` - 监视点的类型标识（1-16, 30-37）
///
/// # 返回
///
/// - `true` - 控制命令类型与监视点类型兼容，可以执行控制操作
/// - `false` - 控制命令类型与监视点类型不兼容，应拒绝控制操作
///
/// # 兼容性规则
///
/// | 控制命令类型 | 兼容的监视点类型 | 说明 |
/// |------------|----------------|------|
/// | 45, 58 (单点控制) | 1, 2, 30 (单点信息) | 控制单点开关（ON/OFF） |
/// | 46, 59 (双点控制) | 3, 4, 31 (双点信息) | 控制双点开关（ON/OFF/INTERMEDIATE/INDETERMINATE） |
/// | 47, 60 (步位置控制) | 5, 6, 32 (步位置信息) | 控制步进电机（LOWER/HIGHER） |
/// | 48, 61 (归一化设定值) | 9, 10, 34 (归一化测量值) | 设置归一化值（-1.0 ~ 1.0） |
/// | 49, 62 (标度化设定值) | 11, 12, 35 (标度化测量值) | 设置标度化值（整数） |
/// | 50, 63 (短浮点设定值) | 13, 14, 36 (短浮点测量值) | 设置浮点值（IEEE 754） |
/// | 51, 64 (位串控制) | 7, 8, 33 (位串信息) | 控制位串（32位） |
///
/// # 使用场景
///
/// 在控制命令处理流程中，使用此函数验证控制命令类型与目标数据点类型是否兼容：
///
/// ```rust
/// use crate::core::slave::protocol::point_type::is_command_type_compatible_with_monitor_type;
///
/// // 单点控制 → 单点信息（兼容）
/// let command_type = 45; // C_SC_NA_1 (单点控制)
/// let monitor_type = 1; // M_SP_NA_1 (单点信息)
/// assert!(is_command_type_compatible_with_monitor_type(command_type, monitor_type));
///
/// // 双点控制 → 单点信息（不兼容）
/// let command_type = 46; // C_DC_NA_1 (双点控制)
/// let monitor_type = 1; // M_SP_NA_1 (单点信息)
/// assert!(!is_command_type_compatible_with_monitor_type(command_type, monitor_type));
///
/// // 短浮点设定值 → 短浮点测量值（兼容）
/// let command_type = 50; // C_SE_NC_1 (短浮点设定值)
/// let monitor_type = 13; // M_ME_NC_1 (短浮点测量值)
/// assert!(is_command_type_compatible_with_monitor_type(command_type, monitor_type));
/// ```
///
/// # 实现细节
///
/// 命令和监视点的类型族由 IEC104 capability registry 统一判定。
///
/// 对于未知的控制命令类型，返回 `false`。
///
/// # 性能
///
/// 本函数使用内联的模式匹配和位运算，性能开销极小，可以在热路径中频繁调用。
pub(crate) fn is_command_type_compatible_with_monitor_type(command_type_id: u8, monitor_type_id: u8) -> bool {
    crate::core::iec104_registry::lookup_capability(command_type_id)
        .is_some_and(|capability| capability.point_role == crate::core::iec104_registry::PointRole::Control)
        && crate::core::iec104_registry::same_type_family(command_type_id, monitor_type_id)
}

/// 判断类型标识是否为单点类型（包括监视点和控制命令）
///
/// 检查类型标识是否属于单点类别，包括单点信息和单点控制。
///
/// # 参数
///
/// * `type_id` - 类型标识
///
/// # 返回
///
/// - `true` - 是单点类型（1, 2, 30, 45, 58）
/// - `false` - 不是单点类型
///
/// # 包含的类型
///
/// - **1 (M_SP_NA_1)**：不带时标的单点信息
/// - **2 (M_SP_TA_1)**：带时标的单点信息（CP24Time2a）
/// - **30 (M_SP_TB_1)**：带时标的单点信息（CP56Time2a）
/// - **45 (C_SC_NA_1)**：不带时标的单点控制
/// - **58 (C_SC_TA_1)**：带时标的单点控制（CP56Time2a）
#[inline]
pub(crate) fn is_single_point_type(type_id: u8) -> bool {
    crate::core::iec104_registry::lookup_capability(type_id)
        .is_some_and(|capability| capability.family == crate::core::iec104_registry::TypeFamily::SinglePoint)
}

/// 判断类型标识是否为双点类型（包括监视点和控制命令）
///
/// 检查类型标识是否属于双点类别，包括双点信息和双点控制。
///
/// # 参数
///
/// * `type_id` - 类型标识
///
/// # 返回
///
/// - `true` - 是双点类型（3, 4, 31, 46, 59）
/// - `false` - 不是双点类型
///
/// # 包含的类型
///
/// - **3 (M_DP_NA_1)**：不带时标的双点信息
/// - **4 (M_DP_TA_1)**：带时标的双点信息（CP24Time2a）
/// - **31 (M_DP_TB_1)**：带时标的双点信息（CP56Time2a）
/// - **46 (C_DC_NA_1)**：不带时标的双点控制
/// - **59 (C_DC_TA_1)**：带时标的双点控制（CP56Time2a）
#[inline]
pub(crate) fn is_double_point_type(type_id: u8) -> bool {
    crate::core::iec104_registry::lookup_capability(type_id)
        .is_some_and(|capability| capability.family == crate::core::iec104_registry::TypeFamily::DoublePoint)
}

/// 判断类型标识是否为累计量类型
///
/// 检查类型标识是否为累计量类型。
///
/// # 参数
///
/// * `type_id` - 类型标识
///
/// # 返回
///
/// - `true` - 是累计量类型（15, 16, 37）
/// - `false` - 不是累计量类型
///
/// # 包含的类型
///
/// - **15 (M_IT_NA_1)**：不带时标的累计量
/// - **16 (M_IT_TA_1)**：带时标的累计量（CP24Time2a）
/// - **37 (M_IT_TB_1)**：带时标的累计量（CP56Time2a）
#[inline]
pub(crate) fn is_integrated_total_type(type_id: u8) -> bool {
    crate::core::iec104_registry::lookup_capability(type_id)
        .is_some_and(|capability| capability.family == crate::core::iec104_registry::TypeFamily::IntegratedTotal)
}

/// 判断监视点类型是否支持数据模拟
///
/// 检查监视点类型是否可以通过从站模拟功能生成数据。
/// 控制命令类型不支持模拟，因为它们是主站发送的命令，不是从站上报的数据。
///
/// # 参数
///
/// * `type_id` - 类型标识
///
/// # 返回
///
/// - `true` - 支持数据模拟
/// - `false` - 不支持数据模拟（通常是控制命令类型）
///
/// # 支持模拟的类型
///
/// - 单点信息（1, 2, 30）
/// - 双点信息（3, 4, 31）
/// - 步位置信息（5, 6, 32）
/// - 位串信息（7, 8, 33）
/// - 归一化测量值（9, 10, 34）
/// - 标度化测量值（11, 12, 35）
/// - 短浮点测量值（13, 14, 36）
/// - 累计量（15, 16, 37）
///
/// # 不支持模拟的类型
///
/// - 所有控制命令类型（45-51, 58-64）
///
/// # 使用场景
///
/// 在从站模拟功能中，使用此函数过滤可模拟的数据点：
///
/// ```rust
/// use crate::core::slave::protocol::point_type::is_simulatable_monitor_type;
///
/// // 单点信息可模拟
/// assert!(is_simulatable_monitor_type(1)); // M_SP_NA_1
/// assert!(is_simulatable_monitor_type(30)); // M_SP_TB_1
///
/// // 控制命令不可模拟
/// assert!(!is_simulatable_monitor_type(45)); // C_SC_NA_1 (单点控制)
/// assert!(!is_simulatable_monitor_type(58)); // C_SC_TA_1 (单点控制)
/// ```
pub(crate) fn is_simulatable_monitor_type(type_id: u8) -> bool {
    crate::core::iec104_registry::lookup_capability(type_id).is_some_and(|capability| capability.simulation.is_some())
}
