//! 数据库模型定义模块。
//!
//! 本模块定义所有数据库表对应的 Rust 结构体,包括:
//!
//! - **枚举类型**:StationType(主站/从站)、ProtocolType(协议类型)、DataType(数据类型)
//! - **站点模型**:Station(站点配置)
//! - **连接模型**:MasterLink(主站链路)、ConnectionParams(连接参数)
//! - **数据点模型**:MasterPoint、SlavePoint
//! - **从站模型**:MasterSlave(链路从站)、SlaveDevice(从站子从站)
//! - **用户设置**:AppSetting(键值对配置)

use chrono::{DateTime, Utc};
use rusqlite::{Result, Row};
use serde::{Deserialize, Serialize};

/// 站点类型
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum StationType {
    /// 主站（客户端）
    Master,
    /// 从站（服务器）
    Slave,
}

impl std::fmt::Display for StationType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            StationType::Master => write!(f, "master"),
            StationType::Slave => write!(f, "slave"),
        }
    }
}

impl std::str::FromStr for StationType {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "master" => Ok(StationType::Master),
            "slave" => Ok(StationType::Slave),
            _ => Err(format!("Invalid station type: {}", s)),
        }
    }
}

/// 协议类型
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ProtocolType {
    /// IEC 60870-5-104 协议
    Iec104,
}

impl std::fmt::Display for ProtocolType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ProtocolType::Iec104 => write!(f, "iec104"),
        }
    }
}

impl std::str::FromStr for ProtocolType {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "iec104" => Ok(ProtocolType::Iec104),
            _ => Err(format!("Invalid protocol type: {}", s)),
        }
    }
}

/// 数据类型
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DataType {
    /// 单点信息
    SinglePoint,
    /// 双点信息
    DoublePoint,
    /// 步位信息
    StepPosition,
    /// 32位比特串
    Bitstring32,
    /// 归一化值
    Normalized,
    /// 标度化值
    Scaled,
    /// 短浮点数
    Float,
    /// 累计量
    IntegratedTotal,
}

impl std::fmt::Display for DataType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            DataType::SinglePoint => write!(f, "single_point"),
            DataType::DoublePoint => write!(f, "double_point"),
            DataType::StepPosition => write!(f, "step_position"),
            DataType::Bitstring32 => write!(f, "bitstring32"),
            DataType::Normalized => write!(f, "normalized"),
            DataType::Scaled => write!(f, "scaled"),
            DataType::Float => write!(f, "float"),
            DataType::IntegratedTotal => write!(f, "integrated_total"),
        }
    }
}

impl std::str::FromStr for DataType {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "single_point" => Ok(DataType::SinglePoint),
            "double_point" => Ok(DataType::DoublePoint),
            "step_position" => Ok(DataType::StepPosition),
            "bitstring32" => Ok(DataType::Bitstring32),
            "normalized" => Ok(DataType::Normalized),
            "scaled" => Ok(DataType::Scaled),
            "float" => Ok(DataType::Float),
            "integrated_total" => Ok(DataType::IntegratedTotal),
            _ => Err(format!("Invalid data type: {}", s)),
        }
    }
}

/// 站点配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Station {
    /// 主键 ID
    pub id: Option<i64>,
    /// 站点名称
    pub name: String,
    /// 站点类型（主站/从站）
    pub station_type: StationType,
    /// 描述信息
    pub description: Option<String>,
    /// 监听/连接地址
    pub host: String,
    /// 监听/连接端口
    pub port: u16,
    /// 是否启用
    pub enabled: bool,
    /// 创建时间
    pub created_at: Option<DateTime<Utc>>,
    /// 更新时间
    pub updated_at: Option<DateTime<Utc>>,
}

impl Station {
    pub fn from_row(row: &Row) -> rusqlite::Result<Self> {
        Ok(Self {
            id: Some(row.get(0)?),
            name: row.get(1)?,
            station_type: row.get::<_, String>(2)?.parse().map_err(|e| {
                rusqlite::Error::FromSqlConversionFailure(
                    2,
                    rusqlite::types::Type::Text,
                    Box::new(std::io::Error::new(std::io::ErrorKind::InvalidData, e)),
                )
            })?,
            description: row.get(3)?,
            host: row.get(4)?,
            port: row.get::<_, i64>(5)? as u16,
            enabled: row.get::<_, Option<i64>>(6)?.map(|value| value != 0).unwrap_or(true),
            created_at: row.get::<_, Option<String>>(7)?.map(|s| s.parse().ok()).flatten(),
            updated_at: row.get::<_, Option<String>>(8)?.map(|s| s.parse().ok()).flatten(),
        })
    }
}

/// 连接配置参数
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConnectionParams {
    /// 远端地址（用于主站侧连接 profile）
    #[serde(default)]
    pub host: String,
    #[serde(default)]
    pub port: u16,
    #[serde(default)]
    pub common_address: u16,

    /// 超时时间（秒）
    pub timeout: u32,
    /// 发送方最多发送未确认 I 格式帧数
    pub k_value: u16,
    /// 接收方最多接收未确认 I 格式帧数
    pub w_value: u16,
    /// 连接建立超时（秒）
    pub t0: u32,
    /// 发送或测试 APDU 超时（秒）
    pub t1: u32,
    /// 无数据报文时确认超时（秒）
    pub t2: u32,
    /// 长期空闲状态下发送测试帧超时（秒）
    pub t3: u32,
}

impl Default for ConnectionParams {
    fn default() -> Self {
        Self {
            host: String::new(),
            port: 0,
            common_address: 1,
            timeout: 30,
            k_value: 12,
            w_value: 8,
            t0: 30,
            t1: 15,
            t2: 10,
            t3: 20,
        }
    }
}

/// 连接配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MasterLink {
    /// 主键 ID
    pub id: Option<i64>,
    /// 所属站点 ID
    pub station_id: i64,
    /// 连接名称
    pub name: String,
    /// 协议类型
    pub protocol_type: ProtocolType,
    /// 连接参数
    pub connection_params: ConnectionParams,
    /// 是否激活
    pub is_active: bool,
    /// 是否自动连接
    pub auto_connect: bool,
    /// 是否自动启动数据传输
    pub auto_start_data_transfer: bool,
    /// 是否自动总召唤
    pub auto_gi: bool,
    /// 重连次数
    pub retry_count: u32,
    /// 重连间隔（毫秒）
    pub retry_interval: u32,
    /// 创建时间
    pub created_at: Option<DateTime<Utc>>,
    /// 更新时间
    pub updated_at: Option<DateTime<Utc>>,
}

impl MasterLink {
    pub fn from_row(row: &Row) -> rusqlite::Result<Self> {
        let params_json: String = row.get(4)?;
        let connection_params: ConnectionParams = serde_json::from_str(&params_json)
            .map_err(|e| rusqlite::Error::FromSqlConversionFailure(4, rusqlite::types::Type::Text, Box::new(e)))?;

        Ok(Self {
            id: Some(row.get(0)?),
            station_id: row.get(1)?,
            name: row.get(2)?,
            protocol_type: row.get::<_, String>(3)?.parse().map_err(|e| {
                rusqlite::Error::FromSqlConversionFailure(
                    3,
                    rusqlite::types::Type::Text,
                    Box::new(std::io::Error::new(std::io::ErrorKind::InvalidData, e)),
                )
            })?,
            connection_params,
            is_active: row.get(5)?,
            auto_connect: row.get(6)?,
            auto_start_data_transfer: row.get(7)?,
            auto_gi: row.get(8)?,
            retry_count: row.get::<_, i64>(9)? as u32,
            retry_interval: row.get::<_, i64>(10)? as u32,
            created_at: row.get::<_, Option<String>>(11)?.map(|s| s.parse().ok()).flatten(),
            updated_at: row.get::<_, Option<String>>(12)?.map(|s| s.parse().ok()).flatten(),
        })
    }
}

/// 用户配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppSetting {
    /// 主键 ID
    pub id: Option<i64>,
    /// 配置键
    pub key: String,
    /// 配置值（JSON 格式）
    pub value: String,
    /// 配置分类
    pub category: String,
    /// 描述信息
    pub description: Option<String>,
    /// 创建时间
    pub created_at: Option<DateTime<Utc>>,
    /// 更新时间
    pub updated_at: Option<DateTime<Utc>>,
}

impl AppSetting {
    pub fn from_row(row: &Row) -> rusqlite::Result<Self> {
        Ok(Self {
            id: Some(row.get(0)?),
            key: row.get(1)?,
            value: row.get(2)?,
            category: row.get(3)?,
            description: row.get(4)?,
            created_at: row.get::<_, Option<String>>(5)?.map(|s| s.parse().ok()).flatten(),
            updated_at: row.get::<_, Option<String>>(6)?.map(|s| s.parse().ok()).flatten(),
        })
    }
}

/// 主站从站（按链路配置下的 COA）
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MasterSlave {
    /// 主键 ID
    pub id: Option<i64>,
    /// 所属连接 ID
    pub connection_id: i64,
    /// 从站名称
    pub name: String,
    /// 公共地址（CA）
    pub common_address: u16,
    /// 是否启用
    pub enabled: bool,
    /// 创建时间
    pub created_at: Option<DateTime<Utc>>,
    /// 更新时间
    pub updated_at: Option<DateTime<Utc>>,
}

impl MasterSlave {
    pub fn from_row(row: &Row) -> rusqlite::Result<Self> {
        Ok(Self {
            id: Some(row.get(0)?),
            connection_id: row.get(1)?,
            name: row.get(2)?,
            common_address: row.get::<_, i64>(3)? as u16,
            enabled: row.get::<_, Option<i64>>(4)?.map(|value| value != 0).unwrap_or(true),
            created_at: row.get::<_, Option<String>>(5)?.and_then(|s| s.parse().ok()),
            updated_at: row.get::<_, Option<String>>(6)?.and_then(|s| s.parse().ok()),
        })
    }
}

/// 主站从站点表
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MasterPoint {
    /// 主键 ID
    pub id: Option<i64>,
    /// 所属从站 ID
    pub connection_slave_id: i64,
    /// 信息对象地址（IOA）
    pub address: u32,
    /// 数据点名称
    pub name: String,
    /// 类型标识（TypeID）
    pub type_id: u8,
    /// 数据类型
    pub data_type: DataType,
    /// 描述信息
    pub description: Option<String>,
    /// 默认值（JSON 格式）
    pub default_value: Option<String>,
    /// 是否启用
    pub is_enabled: bool,
    /// 创建时间
    pub created_at: Option<DateTime<Utc>>,
    /// 更新时间
    pub updated_at: Option<DateTime<Utc>>,
}

impl MasterPoint {
    pub fn from_row(row: &Row) -> rusqlite::Result<Self> {
        Ok(Self {
            id: Some(row.get(0)?),
            connection_slave_id: row.get(1)?,
            address: row.get::<_, i64>(2)? as u32,
            name: row.get(3)?,
            type_id: row.get::<_, i64>(4)? as u8,
            data_type: row.get::<_, String>(5)?.parse().map_err(|e| {
                rusqlite::Error::FromSqlConversionFailure(
                    5,
                    rusqlite::types::Type::Text,
                    Box::new(std::io::Error::new(std::io::ErrorKind::InvalidData, e)),
                )
            })?,
            description: row.get(6)?,
            default_value: row.get(7)?,
            is_enabled: row.get(8)?,
            created_at: row.get::<_, Option<String>>(9)?.and_then(|s| s.parse().ok()),
            updated_at: row.get::<_, Option<String>>(10)?.and_then(|s| s.parse().ok()),
        })
    }
}

/// 从站子站（按连接下的 COA）
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SlaveDevice {
    /// 主键 ID
    pub id: Option<i64>,
    /// 所属站点 ID
    pub station_id: i64,
    /// 子站名称
    pub name: String,
    /// 公共地址（CA）
    pub common_address: u16,
    /// 逻辑从站策略覆盖（JSON）
    pub station_policy_override: Option<String>,
    /// 是否启用
    pub enabled: bool,
    /// 创建时间
    pub created_at: Option<DateTime<Utc>>,
    /// 更新时间
    pub updated_at: Option<DateTime<Utc>>,
}

impl SlaveDevice {
    pub fn from_row(row: &Row) -> rusqlite::Result<Self> {
        Ok(Self {
            id: Some(row.get(0)?),
            station_id: row.get(1)?,
            name: row.get(2)?,
            common_address: row.get::<_, i64>(3)? as u16,
            station_policy_override: row.get(4)?,
            enabled: row.get::<_, Option<i64>>(5)?.map(|value| value != 0).unwrap_or(true),
            created_at: row.get::<_, Option<String>>(6)?.and_then(|s| s.parse().ok()),
            updated_at: row.get::<_, Option<String>>(7)?.and_then(|s| s.parse().ok()),
        })
    }
}

/// 从站子站点表
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SlavePoint {
    /// 主键 ID
    pub id: Option<i64>,
    /// 所属子站 ID
    pub slave_station_id: i64,
    /// 信息对象地址（IOA）
    pub address: u32,
    /// 数据点名称
    pub name: String,
    /// 类型标识（TypeID）
    pub type_id: u8,
    /// 数据类型
    pub data_type: DataType,
    /// 单位
    pub unit: Option<String>,
    /// 描述信息
    pub description: Option<String>,
    /// 最小值
    pub min_value: Option<f64>,
    /// 最大值
    pub max_value: Option<f64>,
    /// 默认值（JSON 格式）
    pub default_value: Option<String>,
    /// 运行态持久值（JSON 格式）
    pub value: Option<String>,
    /// 控制点地址（用于遥控）
    pub control_ioa: Option<u32>,
    /// 总召唤组（1-16）
    pub gi_group: Option<u8>,
    /// 计数量召唤组（1-4）
    pub counter_group: Option<u8>,
    /// 是否启用
    pub is_enabled: bool,
    /// 创建时间
    pub created_at: Option<DateTime<Utc>>,
    /// 更新时间
    pub updated_at: Option<DateTime<Utc>>,
}

impl SlavePoint {
    pub fn from_row(row: &Row) -> rusqlite::Result<Self> {
        Ok(Self {
            id: Some(row.get(0)?),
            slave_station_id: row.get(1)?,
            address: row.get::<_, i64>(2)? as u32,
            name: row.get(3)?,
            type_id: row.get::<_, i64>(4)? as u8,
            data_type: row.get::<_, String>(5)?.parse().map_err(|e| {
                rusqlite::Error::FromSqlConversionFailure(
                    5,
                    rusqlite::types::Type::Text,
                    Box::new(std::io::Error::new(std::io::ErrorKind::InvalidData, e)),
                )
            })?,
            unit: row.get(6)?,
            description: row.get(7)?,
            min_value: row.get(8)?,
            max_value: row.get(9)?,
            default_value: row.get(10)?,
            value: row.get(11)?,
            control_ioa: row.get::<_, Option<i64>>(12)?.map(|v| v as u32),
            gi_group: row.get::<_, Option<i64>>(13)?.map(|v| v as u8),
            counter_group: row.get::<_, Option<i64>>(14)?.map(|v| v as u8),
            is_enabled: row.get(15)?,
            created_at: row.get::<_, Option<String>>(16)?.and_then(|s| s.parse().ok()),
            updated_at: row.get::<_, Option<String>>(17)?.and_then(|s| s.parse().ok()),
        })
    }
}
