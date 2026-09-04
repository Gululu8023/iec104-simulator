//! PCAP/PCAPNG 导出模块。
//!
//! 将应用内捕获的 TCP 负载导出为 Wireshark 可打开的 PCAP/PCAPNG 格式文件。
//! 在负载周围合成 IPv4/IPv6 + TCP 头部（DLT_RAW），TCP 序列号为合成值但保持流一致性。

use std::{
    collections::HashMap,
    io::Cursor,
    net::{IpAddr, Ipv4Addr, Ipv6Addr, SocketAddr},
    time::Duration,
};

use chrono::{DateTime, Utc};
use pcap_file::{
    DataLink, Endianness, TsResolution,
    pcap::{PcapHeader, PcapPacket, PcapWriter},
    pcapng::{
        PcapNgWriter,
        blocks::{
            enhanced_packet::EnhancedPacketBlock,
            interface_description::{InterfaceDescriptionBlock, InterfaceDescriptionOption},
        },
    },
};

/// 抓包格式枚举。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CaptureFormat {
    /// PCAP 格式（传统格式）
    Pcap,
    /// PCAPNG 格式（新一代格式，支持更多元数据）
    Pcapng,
}

/// 捕获的数据包。
#[derive(Debug, Clone)]
pub struct CapturePacket {
    /// 数据包时间戳（UTC）
    pub timestamp: DateTime<Utc>,
    /// 连接 ID
    pub connection_id: String,
    /// 源地址（IP:端口）
    pub src: SocketAddr,
    /// 目标地址（IP:端口）
    pub dst: SocketAddr,
    /// 应用层负载数据
    pub payload: Vec<u8>,
}

/// TCP 流的唯一标识。
///
/// 使用规范化的地址对（a, b）来标识双向流，确保 (A→B) 和 (B→A) 使用同一个 FlowKey。
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
struct FlowKey {
    /// 流的第一个端点（规范化后的较小地址）
    a: SocketAddr,
    /// 流的第二个端点（规范化后的较大地址）
    b: SocketAddr,
}

impl FlowKey {
    /// 创建流标识，自动规范化地址顺序。
    fn new(src: SocketAddr, dst: SocketAddr) -> Self {
        // 使用字典序比较，确保同一个流的两个方向使用相同的 FlowKey
        if canonical_socket_addr_key(&src) <= canonical_socket_addr_key(&dst) {
            Self { a: src, b: dst }
        } else {
            Self { a: dst, b: src }
        }
    }
}

/// TCP 流的状态，用于跟踪合成的序列号。
#[derive(Debug, Clone)]
struct FlowState {
    /// a → b 方向的下一个序列号
    a_to_b_seq: u32,
    /// b → a 方向的下一个序列号
    b_to_a_seq: u32,
}

/// 构建 PCAP 格式文件。
///
/// 将捕获的数据包列表转换为 PCAP 格式的字节数组。
pub fn build_pcap(packets: &[CapturePacket]) -> Vec<u8> {
    let cursor = Cursor::new(Vec::with_capacity(24 + packets.len() * 64));
    let header = PcapHeader {
        snaplen: 65535,
        datalink: DataLink::RAW,
        ts_resolution: TsResolution::MicroSecond,
        endianness: Endianness::Little,
        ..Default::default()
    };
    let mut writer = match PcapWriter::with_header(cursor, header) {
        Ok(writer) => writer,
        Err(_) => return Vec::new(),
    };

    // 维护每个流的序列号状态
    let mut flow_states: HashMap<FlowKey, FlowState> = HashMap::new();

    // 按时间戳排序，确保 PCAP 文件中的数据包按时间顺序排列
    let mut ordered = packets.to_vec();
    ordered.sort_by_key(|p| p.timestamp);

    for packet in ordered {
        // 为每个应用层负载合成完整的 IP+TCP 数据包
        let ip_packet = synthesize_ip_tcp_packet(&mut flow_states, packet.src, packet.dst, &packet.payload);
        let timestamp = timestamp_to_duration(&packet.timestamp);
        let pcap_packet = PcapPacket::new_owned(timestamp, ip_packet.len() as u32, ip_packet);
        let _ = writer.write_packet(&pcap_packet);
    }

    writer.into_writer().into_inner()
}

/// 构建 PCAPNG 格式文件。
///
/// 将捕获的数据包列表转换为 PCAPNG 格式的字节数组。
pub fn build_pcapng(packets: &[CapturePacket]) -> Vec<u8> {
    let cursor = Cursor::new(Vec::with_capacity(512 + packets.len() * 80));
    let mut writer = match PcapNgWriter::with_endianness(cursor, Endianness::Little) {
        Ok(writer) => writer,
        Err(_) => return Vec::new(),
    };

    // 写入接口描述块（PCAPNG 必需）
    let interface = InterfaceDescriptionBlock {
        linktype: DataLink::RAW,
        snaplen: 65535,
        options: vec![InterfaceDescriptionOption::IfTsResol(9)], // 纳秒精度
    };
    let _ = writer.write_pcapng_block(interface);

    // 维护每个流的序列号状态
    let mut flow_states: HashMap<FlowKey, FlowState> = HashMap::new();

    // 按时间戳排序
    let mut ordered = packets.to_vec();
    ordered.sort_by_key(|p| p.timestamp);

    for packet in ordered {
        // 为每个应用层负载合成完整的 IP+TCP 数据包
        let ip_packet = synthesize_ip_tcp_packet(&mut flow_states, packet.src, packet.dst, &packet.payload);
        let timestamp = timestamp_to_duration(&packet.timestamp);
        let block = EnhancedPacketBlock {
            interface_id: 0,
            timestamp,
            original_len: ip_packet.len() as u32,
            data: std::borrow::Cow::Owned(ip_packet),
            options: vec![],
        };
        let _ = writer.write_pcapng_block(block);
    }

    writer.into_inner().into_inner()
}

/// 将时间戳转换为 Duration。
fn timestamp_to_duration(timestamp: &DateTime<Utc>) -> Duration {
    let secs = timestamp.timestamp();
    if secs <= 0 {
        return Duration::from_secs(0);
    }
    Duration::new(secs as u64, timestamp.timestamp_subsec_nanos())
}

/// 生成套接字地址的规范化键，用于字典序比较。
fn canonical_socket_addr_key(addr: &SocketAddr) -> Vec<u8> {
    let mut out = Vec::with_capacity(18);
    match addr.ip() {
        IpAddr::V4(ip) => {
            out.push(4); // IPv4 标记
            out.extend_from_slice(&ip.octets());
        }
        IpAddr::V6(ip) => {
            out.push(6); // IPv6 标记
            out.extend_from_slice(&ip.octets());
        }
    }
    out.extend_from_slice(&addr.port().to_be_bytes());
    out
}

/// 合成 IP+TCP 数据包。
///
/// 根据源/目标地址和负载，构建完整的 IP 和 TCP 头部，并维护流的序列号状态。
fn synthesize_ip_tcp_packet(
    flow_states: &mut HashMap<FlowKey, FlowState>,
    src: SocketAddr,
    dst: SocketAddr,
    payload: &[u8],
) -> Vec<u8> {
    let key = FlowKey::new(src, dst);
    let flow_a = key.a;
    let flow_b = key.b;

    // 获取或初始化流状态
    let state = flow_states.entry(key).or_insert_with(|| FlowState { a_to_b_seq: 1, b_to_a_seq: 1 });

    // 根据数据包方向选择序列号和确认号
    let (seq, ack) = if src == flow_a && dst == flow_b {
        (state.a_to_b_seq, state.b_to_a_seq)
    } else if src == flow_b && dst == flow_a {
        (state.b_to_a_seq, state.a_to_b_seq)
    } else {
        // 不应该发生；回退到稳定但确定性的值
        (1, 0)
    };

    // TCP 标志：空负载为 ACK，有负载为 PSH|ACK
    let flags = if payload.is_empty() { 0x10u16 } else { 0x18u16 };

    // 根据 IP 版本构建数据包
    let ip_packet = match (src.ip(), dst.ip()) {
        (IpAddr::V4(src_ip), IpAddr::V4(dst_ip)) => {
            build_ipv4_tcp_packet(src_ip, dst_ip, src.port(), dst.port(), seq, ack, flags, payload)
        }
        (IpAddr::V6(src_ip), IpAddr::V6(dst_ip)) => {
            build_ipv6_tcp_packet(src_ip, dst_ip, src.port(), dst.port(), seq, ack, flags, payload)
        }
        // 混合 IP 族：尽力而为（跳过校验和，仍然在 IPv4 上写入仅负载的 TCP）
        (src_ip, dst_ip) => build_ipv4_tcp_packet(
            map_ip_to_ipv4(src_ip),
            map_ip_to_ipv4(dst_ip),
            src.port(),
            dst.port(),
            seq,
            ack,
            flags,
            payload,
        ),
    };

    // 更新序列号（合成）
    let delta = payload.len() as u32;
    if src == flow_a && dst == flow_b {
        state.a_to_b_seq = state.a_to_b_seq.saturating_add(delta);
    } else if src == flow_b && dst == flow_a {
        state.b_to_a_seq = state.b_to_a_seq.saturating_add(delta);
    }

    ip_packet
}

/// 将 IP 地址映射为 IPv4 地址（用于混合 IP 族的情况）。
fn map_ip_to_ipv4(ip: IpAddr) -> Ipv4Addr {
    match ip {
        IpAddr::V4(v4) => v4,
        IpAddr::V6(v6) => {
            // 从 IPv6 地址的最后 32 位提取 IPv4 地址
            let segments = v6.segments();
            Ipv4Addr::new((segments[6] >> 8) as u8, segments[6] as u8, (segments[7] >> 8) as u8, segments[7] as u8)
        }
    }
}

/// 构建 IPv4 + TCP 数据包。
///
/// 手动构建完整的 IPv4 头部（20 字节）和 TCP 头部（20 字节），并计算校验和。
fn build_ipv4_tcp_packet(
    src_ip: Ipv4Addr,
    dst_ip: Ipv4Addr,
    src_port: u16,
    dst_port: u16,
    seq: u32,
    ack: u32,
    flags: u16,
    payload: &[u8],
) -> Vec<u8> {
    let ip_header_len = 20usize;
    let tcp_header_len = 20usize;
    let total_len = ip_header_len + tcp_header_len + payload.len();

    // === 构建 IPv4 头部 ===
    let mut ip_header = [0u8; 20];
    ip_header[0] = 0x45; // 版本 4 + IHL=5（20 字节）
    ip_header[1] = 0; // DSCP + ECN
    ip_header[2..4].copy_from_slice(&(total_len as u16).to_be_bytes()); // 总长度
    ip_header[4..6].copy_from_slice(&0u16.to_be_bytes()); // 标识符
    ip_header[6..8].copy_from_slice(&0x4000u16.to_be_bytes()); // 标志：DF（不分片）
    ip_header[8] = 64; // TTL
    ip_header[9] = 6; // 协议：TCP
    ip_header[10..12].copy_from_slice(&0u16.to_be_bytes()); // 校验和占位符
    ip_header[12..16].copy_from_slice(&src_ip.octets()); // 源 IP
    ip_header[16..20].copy_from_slice(&dst_ip.octets()); // 目标 IP

    // 计算并填充 IP 头部校验和
    let ip_checksum = checksum16(&ip_header);
    ip_header[10..12].copy_from_slice(&ip_checksum.to_be_bytes());

    // === 构建 TCP 头部 ===
    let mut tcp_header = [0u8; 20];
    tcp_header[0..2].copy_from_slice(&src_port.to_be_bytes()); // 源端口
    tcp_header[2..4].copy_from_slice(&dst_port.to_be_bytes()); // 目标端口
    tcp_header[4..8].copy_from_slice(&seq.to_be_bytes()); // 序列号
    tcp_header[8..12].copy_from_slice(&ack.to_be_bytes()); // 确认号
    tcp_header[12] = 0x50; // 数据偏移=5（20 字节）
    tcp_header[13] = (flags & 0xFF) as u8; // TCP 标志
    tcp_header[14..16].copy_from_slice(&65535u16.to_be_bytes()); // 窗口大小
    tcp_header[16..18].copy_from_slice(&0u16.to_be_bytes()); // 校验和占位符
    tcp_header[18..20].copy_from_slice(&0u16.to_be_bytes()); // 紧急指针

    // 计算 TCP 校验和（需要伪头部）
    let tcp_len = (tcp_header_len + payload.len()) as u16;
    let mut pseudo = Vec::with_capacity(12 + tcp_len as usize);
    pseudo.extend_from_slice(&src_ip.octets()); // 源 IP
    pseudo.extend_from_slice(&dst_ip.octets()); // 目标 IP
    pseudo.push(0); // 保留字节
    pseudo.push(6); // 协议：TCP
    pseudo.extend_from_slice(&tcp_len.to_be_bytes()); // TCP 长度
    pseudo.extend_from_slice(&tcp_header); // TCP 头部
    pseudo.extend_from_slice(payload); // TCP 负载
    let tcp_checksum = checksum16(&pseudo);
    tcp_header[16..18].copy_from_slice(&tcp_checksum.to_be_bytes());

    // 组装完整数据包
    let mut out = Vec::with_capacity(total_len);
    out.extend_from_slice(&ip_header);
    out.extend_from_slice(&tcp_header);
    out.extend_from_slice(payload);
    out
}

/// 构建 IPv6 + TCP 数据包。
///
/// 手动构建完整的 IPv6 头部（40 字节）和 TCP 头部（20 字节），并计算校验和。
fn build_ipv6_tcp_packet(
    src_ip: Ipv6Addr,
    dst_ip: Ipv6Addr,
    src_port: u16,
    dst_port: u16,
    seq: u32,
    ack: u32,
    flags: u16,
    payload: &[u8],
) -> Vec<u8> {
    let ip_header_len = 40usize;
    let tcp_header_len = 20usize;
    let payload_len = tcp_header_len + payload.len();

    // === 构建 IPv6 头部 ===
    let mut ip_header = [0u8; 40];
    ip_header[0] = 0x60; // 版本 6
    ip_header[4..6].copy_from_slice(&(payload_len as u16).to_be_bytes()); // 负载长度
    ip_header[6] = 6; // 下一个头部：TCP
    ip_header[7] = 64; // 跳数限制
    ip_header[8..24].copy_from_slice(&src_ip.octets()); // 源 IP
    ip_header[24..40].copy_from_slice(&dst_ip.octets()); // 目标 IP

    // === 构建 TCP 头部 ===
    let mut tcp_header = [0u8; 20];
    tcp_header[0..2].copy_from_slice(&src_port.to_be_bytes());
    tcp_header[2..4].copy_from_slice(&dst_port.to_be_bytes());
    tcp_header[4..8].copy_from_slice(&seq.to_be_bytes());
    tcp_header[8..12].copy_from_slice(&ack.to_be_bytes());
    tcp_header[12] = 0x50;
    tcp_header[13] = (flags & 0xFF) as u8;
    tcp_header[14..16].copy_from_slice(&65535u16.to_be_bytes());
    tcp_header[16..18].copy_from_slice(&0u16.to_be_bytes());
    tcp_header[18..20].copy_from_slice(&0u16.to_be_bytes());

    // 计算 TCP 校验和（IPv6 伪头部）
    let mut pseudo = Vec::with_capacity(40 + payload_len);
    pseudo.extend_from_slice(&src_ip.octets()); // 源 IP（16 字节）
    pseudo.extend_from_slice(&dst_ip.octets()); // 目标 IP（16 字节）
    pseudo.extend_from_slice(&(payload_len as u32).to_be_bytes()); // 上层协议长度
    pseudo.extend_from_slice(&[0u8, 0u8, 0u8]); // 保留字节
    pseudo.push(6); // 下一个头部：TCP
    pseudo.extend_from_slice(&tcp_header);
    pseudo.extend_from_slice(payload);
    let tcp_checksum = checksum16(&pseudo);
    tcp_header[16..18].copy_from_slice(&tcp_checksum.to_be_bytes());

    // 组装完整数据包
    let mut out = Vec::with_capacity(ip_header_len + payload_len);
    out.extend_from_slice(&ip_header);
    out.extend_from_slice(&tcp_header);
    out.extend_from_slice(payload);
    out
}

/// 计算 16 位校验和（用于 IP 和 TCP 头部）。
///
/// 使用标准的 Internet 校验和算法：将数据按 16 位字累加，进位折叠，最后取反。
fn checksum16(data: &[u8]) -> u16 {
    let mut sum: u32 = 0;
    let mut idx = 0;

    // 每次处理 2 个字节（16 位）
    while idx + 1 < data.len() {
        let word = u16::from_be_bytes([data[idx], data[idx + 1]]) as u32;
        sum = sum.wrapping_add(word);
        idx += 2;
    }

    // 处理剩余的单个字节（如果有）
    if idx < data.len() {
        sum = sum.wrapping_add((data[idx] as u32) << 8);
    }

    // 折叠进位
    while (sum >> 16) != 0 {
        sum = (sum & 0xFFFF) + (sum >> 16);
    }

    // 取反得到校验和
    !(sum as u16)
}
