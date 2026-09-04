//! CLI 工具：解析 `data/*.pcap[ng]` 报文并打印 ASDU 信息。
//!
//! ```bash
//! cargo run --bin pcap_dump data/104总召.pcapng
//! cargo run --bin pcap_dump --limit 10 data/104总召.pcapng  # 只解析前10个报文
//! cargo run --bin pcap_dump --skip 5 --limit 10 data/*.pcap  # 跳过前5个，解析接下来的10个
//! ```

use std::{
    env,
    fs::File,
    io::{BufReader, Seek},
    path::Path,
};

use iec60870_parser::{
    FrameType, StreamError,
    decoder::StreamDecoder,
    stream::BytesStream,
    types::{Apdu, AsduConverter, FrameView},
};
use pcap_parser::{
    LegacyPcapReader, PcapBlockOwned, PcapError, PcapNGReader, pcap::LegacyPcapBlock, pcapng::Block,
    traits::PcapReaderIterator,
};

/// 命令行参数配置
struct Config {
    /// 要处理的文件列表
    files: Vec<String>,
    /// 跳过前 N 个 TCP 报文
    skip: usize,
    /// 最多处理 N 个 TCP 报文 (0 表示不限制)
    limit: usize,
}

impl Config {
    fn parse_args(args: &[String]) -> Result<Self, String> {
        let mut files = Vec::new();
        let mut skip = 0;
        let mut limit = 0;
        let mut i = 1;

        while i < args.len() {
            match args[i].as_str() {
                "--skip" | "-s" => {
                    i += 1;
                    if i >= args.len() {
                        return Err("--skip 参数后需要跟一个数字".to_string());
                    }
                    skip = args[i].parse().map_err(|_| format!("--skip 参数无效: {}", args[i]))?;
                }
                "--limit" | "-l" => {
                    i += 1;
                    if i >= args.len() {
                        return Err("--limit 参数后需要跟一个数字".to_string());
                    }
                    limit = args[i].parse().map_err(|_| format!("--limit 参数无效: {}", args[i]))?;
                }
                "--help" | "-h" => {
                    return Err("".to_string()); // 触发帮助信息
                }
                arg if arg.starts_with('-') => {
                    return Err(format!("未知参数: {}", arg));
                }
                file => {
                    files.push(file.to_string());
                }
            }
            i += 1;
        }

        if files.is_empty() {
            return Err("".to_string()); // 触发帮助信息
        }

        Ok(Config { files, skip, limit })
    }

    fn print_help() {
        println!(
            "用法: cargo run --bin pcap_dump [选项] <pcap/pcapng 文件> [...更多文件]

选项:
  --skip, -s <N>    跳过前 N 个 TCP 报文 (默认: 0)
  --limit, -l <N>   最多解析 N 个 TCP 报文，0 表示不限制 (默认: 0)
  --help, -h        显示此帮助信息

示例:
  cargo run --bin pcap_dump data/104总召.pcapng
  cargo run --bin pcap_dump --limit 10 data/104总召.pcapng
  cargo run --bin pcap_dump --skip 5 --limit 10 data/*.pcap
"
        );
    }
}

fn main() {
    let args: Vec<String> = env::args().collect();

    let config = match Config::parse_args(&args) {
        Ok(cfg) => cfg,
        Err(msg) => {
            if !msg.is_empty() {
                eprintln!("错误: {}\n", msg);
            }
            Config::print_help();
            std::process::exit(if msg.is_empty() { 0 } else { 1 });
        }
    };

    let converter = AsduConverter;
    for path in &config.files {
        if let Err(err) = process_path(path, &converter, config.skip, config.limit) {
            eprintln!("{path}: {err}");
        }
    }
}

fn process_path(path: &str, converter: &AsduConverter, skip: usize, limit: usize) -> Result<(), String> {
    let mut file = File::open(path).map_err(|e| format!("无法打开 {path}: {e}"))?;
    let ext = Path::new(path).extension().and_then(|e| e.to_str()).unwrap_or("").to_lowercase();
    println!("=== 解析 {path} ===");
    if skip > 0 {
        println!("跳过前 {} 个 TCP 报文", skip);
    }
    if limit > 0 {
        println!("最多处理 {} 个 TCP 报文", limit);
    }
    if ext == "pcapng" {
        decode_pcap_ng(&mut file, converter, skip, limit)
    } else {
        decode_pcap(&mut file, converter, skip, limit)
    }
}

fn decode_pcap(file: &mut File, converter: &AsduConverter, skip: usize, limit: usize) -> Result<(), String> {
    file.rewind().map_err(|e| format!("文件重置失败: {e}"))?;
    let buf = BufReader::new(file);
    let mut reader = LegacyPcapReader::new(65536, buf).map_err(|e| format!("pcap reader: {e}"))?;
    let mut decoder = FlowDecoder::new();
    let mut packet_count = 0;
    let mut processed_count = 0;

    loop {
        match reader.next() {
            Ok((offset, block)) => {
                if let PcapBlockOwned::Legacy(pkt) = block {
                    // 检查是否有 TCP payload (用于计数)
                    if extract_tcp_payload(pkt.data).is_some() {
                        packet_count += 1;

                        // 应用 skip/limit 逻辑
                        if packet_count <= skip {
                            // 跳过这个报文
                        } else if limit == 0 || processed_count < limit {
                            process_legacy_packet(pkt, &mut decoder, converter);
                            processed_count += 1;
                        } else {
                            // 已达到 limit，停止处理
                            println!("\n已处理 {} 个报文，达到限制，停止解析", processed_count);
                            break;
                        }
                    }
                }
                reader.consume(offset);
            }
            Err(PcapError::Eof) => break,
            Err(PcapError::Incomplete(_)) => {
                reader.refill().map_err(|e| format!("pcap refill: {e}"))?;
            }
            Err(err) => return Err(format!("pcap block error: {err}")),
        }
    }

    println!("\n总计: 发现 {} 个 TCP 报文，已处理 {} 个", packet_count, processed_count);
    Ok(())
}

fn decode_pcap_ng(file: &mut File, converter: &AsduConverter, skip: usize, limit: usize) -> Result<(), String> {
    file.rewind().map_err(|e| format!("文件重置失败: {e}"))?;
    let buf = BufReader::new(file);
    let mut reader = PcapNGReader::new(1_024 * 1024, buf).map_err(|e| format!("pcapng reader: {e}"))?;
    let mut decoder = FlowDecoder::new();
    let mut packet_count = 0;
    let mut processed_count = 0;

    loop {
        match reader.next() {
            Ok((offset, block)) => {
                // 先检查这个 block 是否包含 TCP payload
                let has_tcp_payload = match &block {
                    PcapBlockOwned::NG(Block::EnhancedPacket(epb)) => extract_tcp_payload(epb.data).is_some(),
                    PcapBlockOwned::NG(Block::SimplePacket(spb)) => extract_tcp_payload(spb.data).is_some(),
                    _ => false,
                };

                if has_tcp_payload {
                    packet_count += 1;

                    // 应用 skip/limit 逻辑
                    if packet_count <= skip {
                        // 跳过这个报文
                    } else if limit == 0 || processed_count < limit {
                        process_block(block, &mut decoder, converter)?;
                        processed_count += 1;
                    } else {
                        // 已达到 limit，停止处理
                        println!("\n已处理 {} 个报文，达到限制，停止解析", processed_count);
                        break;
                    }
                }
                reader.consume(offset);
            }
            Err(PcapError::Eof) => break,
            Err(PcapError::Incomplete(_)) => {
                reader.refill().map_err(|e| format!("pcapng refill: {e}"))?;
            }
            Err(err) => return Err(format!("pcapng block error: {err}")),
        }
    }

    println!("\n总计: 发现 {} 个 TCP 报文，已处理 {} 个", packet_count, processed_count);
    Ok(())
}

fn process_block(block: PcapBlockOwned, decoder: &mut FlowDecoder, converter: &AsduConverter) -> Result<(), String> {
    let data = match block {
        PcapBlockOwned::NG(Block::EnhancedPacket(epb)) => epb.data,
        PcapBlockOwned::NG(Block::SimplePacket(spb)) => spb.data,
        _ => return Ok(()),
    };
    if let Some(payload) = extract_tcp_payload(data) {
        decoder.feed(payload, converter);
    }
    Ok(())
}

fn process_legacy_packet(pkt: LegacyPcapBlock<'_>, decoder: &mut FlowDecoder, converter: &AsduConverter) {
    if let Some(payload) = extract_tcp_payload(pkt.data) {
        decoder.feed(payload, converter);
    }
}

struct FlowDecoder {
    decoder: StreamDecoder<BytesStream>,
}

impl FlowDecoder {
    fn new() -> Self {
        Self { decoder: StreamDecoder::new(BytesStream::default()) }
    }

    fn feed(&mut self, payload: &[u8], converter: &AsduConverter) {
        self.decoder.push(payload);
        loop {
            match self.decoder.poll_frame() {
                Ok(Some(frame)) => match frame.to_apdu(converter) {
                    Ok(apdu) => print_apdu(&frame, &apdu),
                    Err(err) => {
                        // 显示更详细的错误信息,包括 TI 类型
                        if let Some(header) = frame.asdu_header() {
                            eprintln!("转换 APDU 失败: {err}, TI={}, CA={}", header.type_id.raw(), header.common_addr);
                        } else {
                            eprintln!("转换 APDU 失败: {err}");
                        }
                    }
                },
                Ok(None) => break,
                Err(StreamError::Parse(err)) => {
                    eprintln!("解析错误: {err}");
                    // 当前损坏帧已在 decoder 内被消费，继续处理后续缓冲区数据。
                    continue;
                }
                Err(StreamError::BufferOverflow { .. }) => {
                    eprintln!("帧长度超出上限，尝试重同步并继续解析");
                    continue;
                }
            }
        }
    }
}

fn print_apdu(frame: &FrameView, apdu: &Apdu<'_>) {
    match frame.apci.frame_type {
        FrameType::I => {
            if let Some(asdu) = &apdu.asdu {
                let header = asdu.header();
                print!(
                    "APDU(I): send_seq={}, recv_seq={}, TI={}, VSQ=0x{:02X}, COT={}, CA={}",
                    frame.apci.send_seq,
                    frame.apci.recv_seq,
                    header.type_id.raw(),
                    header.vsq,
                    header.cot,
                    header.common_addr,
                );

                // 根据 ASDU 类型选择格式化方式
                match asdu {
                    iec60870_parser::types::AsduFrame::File(file_set) => {
                        println!();
                        print_file_transfer_asdu(file_set);
                    }
                    _ => {
                        println!(", ASDU={:#?}", asdu);
                    }
                }
            } else {
                println!("APDU(I): send_seq={}, recv_seq={}, ASDU=解析失败", frame.apci.send_seq, frame.apci.recv_seq);
            }
        }
        FrameType::S => {
            println!("APDU(S): ack_nr={}", frame.apci.recv_seq);
        }
        FrameType::U => {
            let byte = frame.apci.u_control_byte().unwrap_or(0);
            let uc = frame.apci.u_control().map(|u| format!("{:?}", u)).unwrap_or_else(|| "Unknown".into());
            println!("APDU(U): raw=0x{byte:02X}, control={uc}");
        }
    }
}

/// 格式化打印文件传输 ASDU（利用 Display trait）
fn print_file_transfer_asdu(file_set: &iec60870_parser::parser::asdu::file_transfer::FileTransferSet) {
    let header = &file_set.header;
    let count = header.vsq & 0x7F;
    let sq = (header.vsq & 0x80) != 0;

    println!("  文件传输 ASDU:");
    println!("    类型: TI={} ({})", header.type_id.raw(), get_file_transfer_type_name(header.type_id.raw()));
    println!("    信息体数量: {}, SQ={}", count, if sq { "1 (顺序)" } else { "0 (非顺序)" });
    println!("    记录:");

    for (idx, record) in file_set.items.iter().enumerate() {
        println!("      [{}] {}", idx, record);
    }
}

/// 获取文件传输类型名称
fn get_file_transfer_type_name(ti: u8) -> &'static str {
    match ti {
        120 => "文件准备就绪 F_FR_NA_1",
        121 => "节准备就绪 F_SR_NA_1",
        122 => "选择/调用文件 F_SC_NA_1",
        123 => "最后节/段 F_LS_NA_1",
        124 => "确认文件/节 F_AF_NA_1",
        125 => "段传输 F_SG_NA_1",
        126 => "目录 F_DR_TA_1",
        127 => "查询归档 F_SC_NB_1",
        _ => "未知",
    }
}

/// 提取 TCP payload。当前仅处理 Ethernet/IPv4/TCP。
fn extract_tcp_payload(packet: &[u8]) -> Option<&[u8]> {
    if packet.len() < 54 {
        return None;
    }
    let ethertype = u16::from_be_bytes([packet[12], packet[13]]);
    if ethertype != 0x0800 {
        return None;
    }
    let ip_header_len = (packet[14] & 0x0F) as usize * 4;
    if packet.len() < 14 + ip_header_len + 20 {
        return None;
    }
    let protocol = packet[23];
    if protocol != 6 {
        return None;
    }
    let tcp_offset = 14 + ip_header_len;
    let src_port = u16::from_be_bytes([packet[tcp_offset], packet[tcp_offset + 1]]);
    let dst_port = u16::from_be_bytes([packet[tcp_offset + 2], packet[tcp_offset + 3]]);
    if src_port != 2404 && dst_port != 2404 {
        return None;
    }
    let data_offset = ((packet[tcp_offset + 12] >> 4) * 4) as usize;
    let payload_offset = tcp_offset + data_offset;
    if payload_offset > packet.len() {
        return None;
    }
    Some(&packet[payload_offset..])
}
