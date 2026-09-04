//! 配电私有类型 TypeDescriptor 定义。

use crate::parser::asdu::{TypeDescriptor, TypeId};

/// 配电私有类型描述符表。
///
/// 根据《配电自动化系统应用 DL/T634.5104-2009 实施细则》定义。
pub static DISTRIBUTION_DESCRIPTORS: &[TypeDescriptor] = &[
    // TI 42: 故障事件信息
    // IOA(3) + 故障类型(1) + 遥信/遥测可变字段（遥信条目内含 CP56Time2a）
    TypeDescriptor::new(TypeId::new(42), "M_FT_NA_1").with_data_len(None),
    // TI 200: 切换定值区
    // IOA(3) + SN(2)
    TypeDescriptor::new(TypeId::new(200), "C_SR_NA_1").with_data_len(Some(2)),
    // TI 201: 读定值区号
    // 控制方向: IOA(3)
    // 监视方向: IOA(3) + SN1(2) + SN2(2) + SN3(2)
    TypeDescriptor::new(TypeId::new(201), "C_RR_NA_1").with_data_len(None),
    // TI 202: 读参数和定值
    // 控制方向: IOA(3) + SN(2) + 参数IOA列表(可变)
    // 监视方向: IOA(3) + SN(2) + PI(1) + 参数条目(可变)
    TypeDescriptor::new(TypeId::new(202), "C_RS_NA_1").with_data_len(None),
    // TI 203: 写参数和定值
    // IOA(3) + SN(2) + PI(1) + [参数数据(仅预置时)]
    TypeDescriptor::new(TypeId::new(203), "C_WS_NA_1").with_data_len(None),
    // TI 206: 累计量（短浮点数）
    // IOA(3) + 电能量值(4) + QDS(1)
    TypeDescriptor::new(TypeId::new(206), "M_IT_NB_1").with_data_len(Some(5)),
    // TI 207: 带时标累计量（短浮点数）
    // IOA(3) + 电能量值(4) + QDS(1) + CP56Time2a(7)
    TypeDescriptor::new(TypeId::new(207), "M_IT_TC_1").with_data_len(Some(5)).with_timestamp(Some(7)),
    // TI 210: 文件传输（配电特有）
    // IOA(3) + 操作标识(1) + 文件内容(可变)
    TypeDescriptor::new(TypeId::new(210), "F_FR_NA_1").with_data_len(None),
    // TI 211: 软件升级
    // IOA(3) + CTYPE(1, bit7 为 S/E 启动/结束标志)
    TypeDescriptor::new(TypeId::new(211), "F_SR_NA_1").with_data_len(Some(1)),
];
