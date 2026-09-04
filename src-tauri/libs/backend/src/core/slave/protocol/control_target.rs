//! 控制目标解析与验证
//!
//! 本模块负责处理 IEC104 从站的控制目标解析和验证逻辑，是控制命令处理的核心组件。
//!
//! # 核心功能
//!
//! - **控制点解析**：先校验命令 IOA 对应的已声明控制点，再独立解析可选监视目标
//! - **类型兼容性验证**：检查控制命令类型与目标数据点类型是否兼容
//! - **策略执行**：根据配置的类型不匹配策略决定是否拒绝命令
//! - **目标信息提取**：提取目标数据点的当前值和质量信息
//!
//! # 控制目标映射机制
//!
//! IEC104 协议中，控制命令的 IOA（信息对象地址）可能与实际数据点的 IOA 不同。
//! 从站通过 `control_ioa` 字段建立映射关系：
//!
//! ```text
//! 控制命令 IOA (command_ioa) → 数据点 control_ioa 字段 → 实际数据点 IOA (address)
//!
//! 示例：
//! - 控制命令：IOA=10000, Type=45 (C_SC_NA_1, 单点控制)
//! - 数据点：address=100, control_ioa=Some(10000), type_id=1 (M_SP_NA_1, 单点信息)
//! - 映射结果：控制命令 10000 → 数据点 100
//! ```
//!
//! # 类型不匹配策略
//!
//! 从站支持三种类型不匹配策略，用于处理控制命令类型与目标数据点类型不兼容的情况：
//!
//! ## Strict（严格模式）
//!
//! - **行为**：拒绝所有类型不匹配的控制命令
//! - **返回**：`Err(ControlTargetValidationError::TypeMismatch)`
//! - **适用场景**：生产环境，要求严格的类型安全
//! - **示例**：
//!   - 命令类型：46 (C_DC_NA_1, 双点控制)
//!   - 目标类型：1 (M_SP_NA_1, 单点信息)
//!   - 结果：拒绝命令，返回类型不匹配错误
//!
//! ## Compatible（兼容模式）
//!
//! - **行为**：允许类型不匹配，但标记为不匹配
//! - **返回**：`Ok((target, true))` - 第二个参数为 `true` 表示存在类型不匹配
//! - **适用场景**：测试环境，需要一定的灵活性
//! - **示例**：
//!   - 命令类型：46 (C_DC_NA_1, 双点控制)
//!   - 目标类型：1 (M_SP_NA_1, 单点信息)
//!   - 结果：接受命令，但标记为类型不匹配
//!
//! ## Debug（调试模式）
//!
//! - **行为**：允许所有类型不匹配，标记为不匹配
//! - **返回**：`Ok((target, true))` - 第二个参数为 `true` 表示存在类型不匹配
//! - **适用场景**：开发调试，需要最大的灵活性
//! - **示例**：与 Compatible 模式相同
//!
//! # 验证流程
//!
//! ```text
//! 1. 接收控制命令
//!    ↓
//! 2. 调用 resolve_control_target
//!    - 根据 command_ioa 查找已声明控制点
//!    - 检查 common_address 是否匹配
//!    - 查找可选的 control_ioa 监视点映射
//!    ↓
//! 3. 如果未找到已声明控制点
//!    → 返回 ControlTargetValidationError::NoMapping
//!    ↓
//! 4. 调用 validate_control_target
//!    - 检查命令类型与目标类型是否兼容
//!    - 根据策略决定是否拒绝
//!    ↓
//! 5. 返回验证结果
//!    - Ok((target, is_type_mismatch)) - 验证通过
//!    - Err(ControlTargetValidationError) - 验证失败
//! ```
//!
//! # 类型兼容性规则
//!
//! 控制命令类型与监视点类型的兼容性由 `is_command_type_compatible_with_monitor_type` 函数定义：
//!
//! - **单点控制 (45, 58)**：兼容单点信息 (1, 30)
//! - **双点控制 (46, 59)**：兼容双点信息 (3, 31)
//! - **设定值控制 (48, 49, 50, 61, 62, 63)**：兼容测量值 (9, 10, 11, 13, 34, 35, 36)
//! - **其他组合**：不兼容
//!
//! # 使用示例
//!
//! ```rust
//! use std::collections::HashMap;
//!
//! use crate::core::slave::{
//!     protocol::control_target::{
//!         ControlTargetValidationError, resolve_control_target, validate_control_target,
//!     },
//!     service::SlaveCommandMismatchPolicy,
//! };
//!
//! // 构建数据点映射
//! let mut points = HashMap::new();
//! points.insert((1, 100), DataPoint {
//!     address: 100,
//!     control_ioa: Some(10000), // 控制命令 IOA
//!     type_id: 1,               // M_SP_NA_1 (单点信息)
//!     value: 0.0,
//!     quality: 0,
//!     // ... 其他字段
//! });
//!
//! // 解析控制目标
//! let target = resolve_control_target(&points, 1, 10000);
//! assert!(target.is_some());
//! assert_eq!(target.unwrap().address, 100);
//!
//! // 验证控制目标（严格模式）
//! let result = validate_control_target(
//!     &points,
//!     1,     // common_address
//!     10000, // command_ioa
//!     45,    // C_SC_NA_1 (单点控制)
//!     SlaveCommandMismatchPolicy::Strict,
//! );
//! assert!(result.is_ok()); // 类型兼容，验证通过
//!
//! // 验证控制目标（类型不匹配）
//! let result = validate_control_target(
//!     &points,
//!     1,     // common_address
//!     10000, // command_ioa
//!     46,    // C_DC_NA_1 (双点控制)
//!     SlaveCommandMismatchPolicy::Strict,
//! );
//! assert!(result.is_err()); // 类型不匹配，严格模式拒绝
//! ```
//!
//! # 错误处理
//!
//! ## NoMapping 错误
//!
//! - **原因**：未找到与 CA、IOA 和命令类型匹配的已声明控制点
//! - **可能情况**：
//!   - command_ioa 对应的数据点不存在
//!   - command_ioa 对应的数据点不是控制点
//!   - common_address 不匹配，或存在多个监视点指向同一控制点
//! - **说明**：已声明控制点可以不配置监视点映射，此时仍正常确认命令，但不更新监视值
//! - **处理**：发送负确认响应（ACT_CON | P/N）
//!
//! ## TypeMismatch 错误
//!
//! - **原因**：控制命令类型与目标数据点类型不兼容
//! - **触发条件**：仅在 Strict 策略下触发
//! - **处理**：发送负确认响应（ACT_CON | P/N）
//!
//! # 线程安全
//!
//! 本模块中的所有函数都是纯函数，不涉及共享状态，可以安全地在多线程环境中调用。
//! 数据点映射（PointMap）通过不可变引用传递，确保线程安全。

use std::collections::HashMap;

use super::point_type::is_command_type_compatible_with_monitor_type;
use crate::core::{
    iec104_registry::{PointRole, lookup_capability},
    slave::service::SlaveCommandMismatchPolicy,
    types::DataPoint,
};

/// 数据点映射类型
///
/// 使用 `(common_address, address)` 作为键，`DataPoint` 作为值的哈希映射。
/// 这种结构允许快速查找指定公共地址和 IOA 的数据点。
///
/// # 键结构
///
/// - 第一个元素：`common_address` (u16) - 公共地址
/// - 第二个元素：`address` (u32) - 信息对象地址（IOA）
///
/// # 使用场景
///
/// 在控制目标解析时，需要遍历所有数据点查找 `control_ioa` 匹配的数据点。
/// 虽然键是 `(common_address, address)`，但查找时主要通过 `control_ioa` 字段进行匹配。
type PointMap = HashMap<(u16, u32), DataPoint>;

/// 已解析的控制目标
///
/// 封装了控制目标的关键信息，用于后续的控制命令执行。
///
/// # 字段说明
///
/// - `address`：目标数据点的实际 IOA（不是控制命令的 IOA）
/// - `type_id`：目标数据点的类型标识（用于类型兼容性检查）
/// - `current_value`：目标数据点的当前值（用于状态比较和日志记录）
/// - `quality`：目标数据点的质量描述符（用于检查 BL 位等）
///
/// # 使用场景
///
/// 在控制命令处理流程中，首先解析控制目标，然后根据目标信息执行控制操作：
///
/// 1. 检查目标数据点的 BL（Blocked）位，如果被阻塞则拒绝命令
/// 2. 比较命令值与当前值，判断是否需要执行控制
/// 3. 更新目标数据点的值和质量
/// 4. 生成响应报文，使用目标数据点的实际 IOA
///
/// # 示例
///
/// ```rust
/// let target = ResolvedControlTarget {
///     address: 100,       // 实际数据点 IOA
///     type_id: 1,         // M_SP_NA_1 (单点信息)
///     current_value: 0.0, // 当前值为 0（OFF）
///     quality: 0,         // 质量良好
/// };
///
/// // 检查是否被阻塞
/// let is_blocked = (target.quality & 0x10) != 0; // BL 位在 bit 4
/// if is_blocked {
///     // 拒绝命令，发送负确认
/// }
/// ```
#[derive(Debug, Clone, Copy)]
pub(crate) struct ResolvedCommandPoint {
    pub(crate) type_id: u8,
}

#[derive(Debug, Clone, Copy)]
pub(crate) struct ResolvedMonitorTarget {
    pub(crate) address: u32,
    pub(crate) type_id: u8,
    pub(crate) current_value: f64,
    pub(crate) quality: u8,
}

#[derive(Debug, Clone, Copy)]
pub(crate) struct ResolvedControlTarget {
    pub(crate) command_point: ResolvedCommandPoint,
    pub(crate) mapped_monitor_target: Option<ResolvedMonitorTarget>,
    /// 目标数据点的实际 IOA
    pub(crate) address: u32,
    /// 目标数据点的类型标识
    pub(crate) type_id: u8,
    /// 目标数据点的当前值
    pub(crate) current_value: f64,
}

/// 控制目标验证错误
///
/// 表示控制目标验证过程中可能发生的错误类型。
///
/// # 错误类型
///
/// ## NoMapping
///
/// 无法解析控制命令 IOA 对应的已声明控制点。
///
/// **可能原因**：
/// - 控制命令的 IOA 对应的数据点不存在
/// - 对应数据点不是控制点
/// - 公共地址不匹配，或存在多个监视点指向同一控制点
///
/// **处理方式**：
/// - 发送负确认响应（ACT_CON | P/N）
/// - 记录错误日志
///
/// **示例**：
/// ```rust
/// // 控制命令：IOA=10000, CA=1
/// // 已声明控制点中不存在 address=10000, CA=1
/// // 结果：NoMapping（控制点未声明）
/// ```
///
/// ## TypeMismatch
///
/// 控制命令类型与目标数据点类型不兼容。
///
/// **触发条件**：
/// - 仅在 `SlaveCommandMismatchPolicy::Strict` 策略下触发
/// - 命令类型与目标类型不满足兼容性规则
///
/// **字段**：
/// - `target_type_id`：目标数据点的类型标识（用于日志记录）
///
/// **处理方式**：
/// - 发送负确认响应（ACT_CON | P/N）
/// - 记录错误日志，包含命令类型和目标类型
///
/// **示例**：
/// ```rust
/// // 控制命令：Type=46 (C_DC_NA_1, 双点控制)
/// // 目标数据点：Type=1 (M_SP_NA_1, 单点信息)
/// // 结果：TypeMismatch { target_type_id: 1 }（类型不兼容）
/// ```
///
/// # 使用场景
///
/// 在控制命令处理流程中，根据验证错误类型决定响应策略：
///
/// ```rust
/// match validate_control_target(&points, ca, ioa, type_id, policy) {
///     Ok((target, is_mismatch)) => {
///         // 验证通过，执行控制命令
///         if is_mismatch {
///             // 记录类型不匹配警告
///         }
///     }
///     Err(ControlTargetValidationError::NoMapping) => {
///         // 发送负确认：未知 IOA
///     }
///     Err(ControlTargetValidationError::TypeMismatch { target_type_id }) => {
///         // 发送负确认：类型不匹配
///     }
/// }
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ControlTargetValidationError {
    /// 未找到控制命令 IOA 对应的数据点映射
    NoMapping,
    /// 控制命令类型与目标数据点类型不兼容
    TypeMismatch {
        /// 目标数据点的类型标识
        target_type_id: u8,
    },
}

/// 解析控制目标
///
/// 根据控制命令的 IOA 查找对应的数据点目标。
/// 通过遍历数据点映射，查找 `control_ioa` 字段与命令 IOA 匹配的数据点。
///
/// # 参数
///
/// * `map` - 数据点映射（不可变引用）
/// * `common_address` - 公共地址（必须与数据点的公共地址匹配）
/// * `command_ioa` - 控制命令的 IOA（用于匹配数据点的 `control_ioa` 字段）
///
/// # 返回
///
/// - `Some(ResolvedControlTarget)` - 找到匹配的数据点，返回控制目标信息
/// - `None` - 未找到匹配的数据点
///
/// # 查找逻辑
///
/// 1. 按 `(common_address, command_ioa)` 查找已声明控制点
/// 2. 控制点角色必须为 `control`
/// 3. 独立查找 `control_ioa` 指向该控制点的唯一监视目标
/// 4. 未映射时仍返回控制点，`mapped_monitor_target` 为 `None`
///
/// # 控制目标映射
///
/// 控制命令的 IOA 可能与实际数据点的 IOA 不同，通过 `control_ioa` 字段建立映射：
///
/// ```text
/// 控制命令 IOA → 数据点 control_ioa 字段 → 实际数据点 IOA
///
/// 示例：
/// - 控制命令：IOA=10000
/// - 数据点：address=100, control_ioa=Some(10000)
/// - 映射结果：10000 → 100
/// ```
///
/// # 使用场景
///
/// 在处理控制命令时，首先调用此函数解析控制目标：
///
/// ```rust
/// let target = resolve_control_target(&points, common_address, command_ioa);
/// match target {
///     Some(target) => {
///         // 找到控制目标，继续验证和执行
///         println!("控制目标：IOA={}, Type={}", target.address, target.type_id);
///     }
///     None => {
///         // 未找到控制目标，发送负确认
///         println!("未找到控制目标：command_ioa={}", command_ioa);
///     }
/// }
/// ```
///
/// # 性能考虑
///
/// 本函数使用线性搜索遍历所有数据点，时间复杂度为 O(n)。
/// 在数据点数量较多时，可能存在性能瓶颈。
/// 如果需要优化，可以考虑建立 `control_ioa` 到数据点的反向索引。
///
/// # 线程安全
///
/// 本函数是纯函数，通过不可变引用访问数据点映射，可以安全地在多线程环境中调用。
pub(crate) fn resolve_control_target(
    map: &PointMap,
    common_address: u16,
    command_ioa: u32,
) -> Option<ResolvedControlTarget> {
    let command_point = map.get(&(common_address, command_ioa))?;
    if lookup_capability(command_point.type_id)?.point_role != PointRole::Control {
        return None;
    }

    let mut matches = map
        .iter()
        .filter(|((ca, _), point)| {
            *ca == common_address
                && point.control_ioa == Some(command_ioa)
                && lookup_capability(point.type_id)
                    .is_some_and(|capability| capability.point_role == PointRole::Monitor)
        })
        .map(|(_, point)| point);
    let mapped_point = matches.next();
    if matches.next().is_some() {
        return None;
    }
    let mapped_monitor_target = mapped_point.map(|point| ResolvedMonitorTarget {
        address: point.address,
        type_id: point.type_id,
        current_value: point.value,
        quality: point.quality,
    });
    let effective = mapped_monitor_target.unwrap_or(ResolvedMonitorTarget {
        address: command_point.address,
        type_id: command_point.type_id,
        current_value: command_point.value,
        quality: command_point.quality,
    });
    Some(ResolvedControlTarget {
        command_point: ResolvedCommandPoint { type_id: command_point.type_id },
        mapped_monitor_target,
        address: effective.address,
        type_id: effective.type_id,
        current_value: effective.current_value,
    })
}

/// 验证控制目标
///
/// 解析并验证控制目标，检查类型兼容性，并根据配置的策略决定是否拒绝命令。
/// 这是控制命令处理流程中的核心验证函数，集成了目标解析和类型验证两个步骤。
///
/// # 参数
///
/// * `map` - 数据点映射（不可变引用）
/// * `common_address` - 公共地址（必须与数据点的公共地址匹配）
/// * `command_ioa` - 控制命令的 IOA（用于匹配数据点的 `control_ioa` 字段）
/// * `command_type_id` - 控制命令的类型标识（用于类型兼容性检查）
/// * `policy` - 类型不匹配策略（Strict/Compatible/Debug）
///
/// # 返回
///
/// 返回 `Result<(ResolvedControlTarget, bool), ControlTargetValidationError>`：
///
/// ## 成功情况 `Ok((target, is_type_mismatch))`
///
/// - `target`：已解析的控制目标信息
/// - `is_type_mismatch`：类型是否不匹配
///   - `true`：命令类型与目标类型不兼容（仅在 Compatible 或 Debug 策略下返回）
///   - `false`：命令类型与目标类型兼容
///
/// ## 失败情况 `Err(ControlTargetValidationError)`
///
/// - `NoMapping`：无法解析控制命令 IOA 对应的已声明控制点
/// - `TypeMismatch { target_type_id }`：类型不匹配且策略为 Strict
///
/// # 执行流程
///
/// 1. **解析控制目标**：
///    - 调用 `resolve_control_target` 查找控制目标
///    - 如果未找到，返回 `Err(ControlTargetValidationError::NoMapping)`
///
/// 2. **检查类型兼容性**：
///    - 调用 `is_command_type_compatible_with_monitor_type` 检查类型兼容性
///    - 计算 `is_type_mismatch` 标志
///
/// 3. **应用策略**：
///    - 如果类型不匹配且策略为 `Strict`，返回 `Err(ControlTargetValidationError::TypeMismatch)`
///    - 否则返回 `Ok((target, is_type_mismatch))`
///
/// # 策略行为
///
/// ## Strict（严格模式）
///
/// - **行为**：拒绝所有类型不匹配的控制命令
/// - **返回**：`Err(ControlTargetValidationError::TypeMismatch { target_type_id })`
/// - **适用场景**：生产环境，要求严格的类型安全
///
/// **示例**：
/// ```rust
/// // 命令类型：46 (C_DC_NA_1, 双点控制)
/// // 目标类型：1 (M_SP_NA_1, 单点信息)
/// let result = validate_control_target(&points, 1, 10000, 46, SlaveCommandMismatchPolicy::Strict);
/// assert!(result.is_err()); // 类型不匹配，拒绝命令
/// ```
///
/// ## Compatible（兼容模式）
///
/// - **行为**：允许类型不匹配，但标记为不匹配
/// - **返回**：`Ok((target, true))` - 第二个参数为 `true` 表示存在类型不匹配
/// - **适用场景**：测试环境，需要一定的灵活性
///
/// **示例**：
/// ```rust
/// // 命令类型：46 (C_DC_NA_1, 双点控制)
/// // 目标类型：1 (M_SP_NA_1, 单点信息)
/// let result =
///     validate_control_target(&points, 1, 10000, 46, SlaveCommandMismatchPolicy::Compatible);
/// assert!(result.is_ok()); // 接受命令
/// assert!(result.unwrap().1); // 标记为类型不匹配
/// ```
///
/// ## Debug（调试模式）
///
/// - **行为**：允许所有类型不匹配，标记为不匹配
/// - **返回**：`Ok((target, true))` - 第二个参数为 `true` 表示存在类型不匹配
/// - **适用场景**：开发调试，需要最大的灵活性
///
/// **示例**：与 Compatible 模式相同
///
/// # 类型兼容性规则
///
/// 控制命令类型与监视点类型的兼容性由 `is_command_type_compatible_with_monitor_type` 函数定义：
///
/// - **单点控制 (45, 58)**：兼容单点信息 (1, 30)
/// - **双点控制 (46, 59)**：兼容双点信息 (3, 31)
/// - **设定值控制 (48, 49, 50, 61, 62, 63)**：兼容测量值 (9, 10, 11, 13, 34, 35, 36)
/// - **其他组合**：不兼容
///
/// # 使用示例
///
/// ## 示例 1：严格模式下的类型兼容验证
///
/// ```rust
/// use crate::core::slave::{
///     protocol::control_target::validate_control_target, service::SlaveCommandMismatchPolicy,
/// };
///
/// // 构建数据点映射
/// let mut points = HashMap::new();
/// points.insert((1, 100), DataPoint {
///     address: 100,
///     control_ioa: Some(10000),
///     type_id: 1, // M_SP_NA_1 (单点信息)
///     value: 0.0,
///     quality: 0,
///     // ... 其他字段
/// });
///
/// // 验证单点控制命令（类型兼容）
/// let result = validate_control_target(
///     &points,
///     1,     // common_address
///     10000, // command_ioa
///     45,    // C_SC_NA_1 (单点控制)
///     SlaveCommandMismatchPolicy::Strict,
/// );
/// assert!(result.is_ok());
/// let (target, is_mismatch) = result.unwrap();
/// assert_eq!(target.address, 100);
/// assert!(!is_mismatch); // 类型兼容
/// ```
///
/// ## 示例 2：严格模式下的类型不匹配拒绝
///
/// ```rust
/// // 验证双点控制命令（类型不匹配）
/// let result = validate_control_target(
///     &points,
///     1,     // common_address
///     10000, // command_ioa
///     46,    // C_DC_NA_1 (双点控制)
///     SlaveCommandMismatchPolicy::Strict,
/// );
/// assert!(result.is_err());
/// match result.unwrap_err() {
///     ControlTargetValidationError::TypeMismatch { target_type_id } => {
///         assert_eq!(target_type_id, 1); // 目标类型为单点信息
///     }
///     _ => panic!("Expected TypeMismatch error"),
/// }
/// ```
///
/// ## 示例 3：兼容模式下的类型不匹配接受
///
/// ```rust
/// // 验证双点控制命令（类型不匹配，但兼容模式接受）
/// let result = validate_control_target(
///     &points,
///     1,     // common_address
///     10000, // command_ioa
///     46,    // C_DC_NA_1 (双点控制)
///     SlaveCommandMismatchPolicy::Compatible,
/// );
/// assert!(result.is_ok());
/// let (target, is_mismatch) = result.unwrap();
/// assert_eq!(target.address, 100);
/// assert!(is_mismatch); // 标记为类型不匹配
/// ```
///
/// ## 示例 4：未找到映射的错误处理
///
/// ```rust
/// // 验证不存在的控制命令 IOA
/// let result = validate_control_target(
///     &points,
///     1,     // common_address
///     99999, // command_ioa（不存在）
///     45,    // C_SC_NA_1 (单点控制)
///     SlaveCommandMismatchPolicy::Strict,
/// );
/// assert!(result.is_err());
/// match result.unwrap_err() {
///     ControlTargetValidationError::NoMapping => {
///         // 未找到映射，发送负确认
///     }
///     _ => panic!("Expected NoMapping error"),
/// }
/// ```
///
/// # 错误处理
///
/// ## NoMapping 错误
///
/// - **原因**：无法解析与 CA、IOA 和命令类型匹配的已声明控制点
/// - **处理**：发送负确认响应（ACT_CON | P/N），传输原因为 47（未知 IOA）
///
/// ## TypeMismatch 错误
///
/// - **原因**：控制命令类型与目标数据点类型不兼容
/// - **触发条件**：仅在 Strict 策略下触发
/// - **处理**：发送负确认响应（ACT_CON | P/N），记录错误日志
///
/// # 性能考虑
///
/// 本函数内部调用 `resolve_control_target`，使用线性搜索遍历所有数据点，时间复杂度为 O(n)。
/// 在数据点数量较多时，可能存在性能瓶颈。
///
/// # 线程安全
///
/// 本函数是纯函数，通过不可变引用访问数据点映射，可以安全地在多线程环境中调用。
pub(crate) fn validate_control_target(
    map: &PointMap,
    common_address: u16,
    command_ioa: u32,
    command_type_id: u8,
    policy: SlaveCommandMismatchPolicy,
) -> Result<(ResolvedControlTarget, bool), ControlTargetValidationError> {
    let target =
        resolve_control_target(map, common_address, command_ioa).ok_or(ControlTargetValidationError::NoMapping)?;
    if target.command_point.type_id != command_type_id {
        return Err(ControlTargetValidationError::TypeMismatch { target_type_id: target.command_point.type_id });
    }
    let is_type_mismatch = target
        .mapped_monitor_target
        .is_some_and(|monitor| !is_command_type_compatible_with_monitor_type(command_type_id, monitor.type_id));

    if is_type_mismatch && matches!(policy, SlaveCommandMismatchPolicy::Strict) {
        return Err(ControlTargetValidationError::TypeMismatch { target_type_id: target.type_id });
    }

    Ok((target, is_type_mismatch))
}
