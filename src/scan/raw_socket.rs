//! Raw Socket基盤モジュール
//!
//! SYN/FIN/Xmas/NULLスキャンに必要なRaw Socket操作を提供する。

use crate::common::error::{NelstError, Result};
use pnet::packet::ip::IpNextHeaderProtocols;
use pnet::packet::tcp::{MutableTcpPacket, TcpFlags, TcpPacket};
use pnet::transport::{
    TransportChannelType, TransportProtocol, TransportReceiver, TransportSender, transport_channel,
};
use std::net::{IpAddr, Ipv4Addr};

/// TCPフラグの組み合わせ
#[derive(Debug, Clone, Copy)]
pub enum TcpScanFlags {
    /// SYNスキャン: SYNフラグのみ
    Syn,
    /// FINスキャン: FINフラグのみ
    Fin,
    /// Xmasスキャン: FIN + PSH + URG
    Xmas,
    /// NULLスキャン: フラグなし
    Null,
}

impl TcpScanFlags {
    /// フラグ値を取得（u8として返す）
    pub fn flags(&self) -> u8 {
        match self {
            TcpScanFlags::Syn => TcpFlags::SYN,
            TcpScanFlags::Fin => TcpFlags::FIN,
            TcpScanFlags::Xmas => TcpFlags::FIN | TcpFlags::PSH | TcpFlags::URG,
            TcpScanFlags::Null => 0,
        }
    }

    /// 名前を取得
    pub fn name(&self) -> &'static str {
        match self {
            TcpScanFlags::Syn => "SYN",
            TcpScanFlags::Fin => "FIN",
            TcpScanFlags::Xmas => "Xmas",
            TcpScanFlags::Null => "NULL",
        }
    }
}

/// root権限をチェック
pub fn check_root_privileges() -> Result<()> {
    #[cfg(unix)]
    {
        // SAFETY: geteuid is always safe to call
        if unsafe { libc::geteuid() } != 0 {
            return Err(NelstError::permission_with_hint(
                "This scan method requires root privileges",
                "Run with 'sudo nelst scan port -m syn ...'",
            ));
        }
    }
    #[cfg(windows)]
    {
        // Windowsでは管理者権限チェックが複雑なため、実行時エラーに任せる
    }
    Ok(())
}

/// Raw TCPソケットチャネルを作成
pub fn create_tcp_channel() -> Result<(TransportSender, TransportReceiver)> {
    check_root_privileges()?;

    let protocol = TransportProtocol::Ipv4(IpNextHeaderProtocols::Tcp);
    let (tx, rx) =
        transport_channel(4096, TransportChannelType::Layer4(protocol)).map_err(|e| {
            NelstError::permission_with_hint(
                format!("Failed to create raw socket: {}", e),
                "Ensure you have CAP_NET_RAW capability or run as root",
            )
        })?;

    Ok((tx, rx))
}

/// TCPチェックサムを計算
fn tcp_checksum(source: Ipv4Addr, dest: Ipv4Addr, tcp_packet: &[u8]) -> u16 {
    let mut sum: u32 = 0;

    // 疑似ヘッダー
    let src_octets = source.octets();
    let dst_octets = dest.octets();
    sum += u16::from_be_bytes([src_octets[0], src_octets[1]]) as u32;
    sum += u16::from_be_bytes([src_octets[2], src_octets[3]]) as u32;
    sum += u16::from_be_bytes([dst_octets[0], dst_octets[1]]) as u32;
    sum += u16::from_be_bytes([dst_octets[2], dst_octets[3]]) as u32;
    sum += 6u32; // TCP protocol number
    sum += tcp_packet.len() as u32;

    // TCPパケット
    let mut i = 0;
    while i < tcp_packet.len() - 1 {
        sum += u16::from_be_bytes([tcp_packet[i], tcp_packet[i + 1]]) as u32;
        i += 2;
    }
    if i < tcp_packet.len() {
        sum += (tcp_packet[i] as u32) << 8;
    }

    // 折り返し
    while sum >> 16 != 0 {
        sum = (sum & 0xFFFF) + (sum >> 16);
    }

    !sum as u16
}

/// TCPパケットを構築
pub fn build_tcp_packet(
    source_ip: Ipv4Addr,
    source_port: u16,
    dest_ip: Ipv4Addr,
    dest_port: u16,
    flags: TcpScanFlags,
    seq: u32,
) -> Vec<u8> {
    // TCPヘッダーサイズ: 20バイト（オプションなし）
    let tcp_header_len = 20;
    let mut buffer = vec![0u8; tcp_header_len];

    {
        let mut tcp_packet = MutableTcpPacket::new(&mut buffer).unwrap();
        tcp_packet.set_source(source_port);
        tcp_packet.set_destination(dest_port);
        tcp_packet.set_sequence(seq);
        tcp_packet.set_acknowledgement(0);
        tcp_packet.set_data_offset(5); // 5 * 4 = 20 bytes
        tcp_packet.set_reserved(0);
        tcp_packet.set_flags(flags.flags());
        tcp_packet.set_window(65535);
        tcp_packet.set_urgent_ptr(0);
        tcp_packet.set_checksum(0);
    }

    // チェックサムを計算して設定
    let checksum = tcp_checksum(source_ip, dest_ip, &buffer);
    {
        let mut tcp_packet = MutableTcpPacket::new(&mut buffer).unwrap();
        tcp_packet.set_checksum(checksum);
    }

    buffer
}

/// 受信したTCPパケットを解析
#[derive(Debug, Clone)]
#[allow(dead_code)]
pub struct TcpResponse {
    pub source_port: u16,
    pub dest_port: u16,
    pub flags: u8,
    pub is_syn_ack: bool,
    pub is_rst: bool,
}

impl TcpResponse {
    /// TCPパケットから解析
    pub fn from_packet(packet: &TcpPacket) -> Self {
        let flags = packet.get_flags();
        Self {
            source_port: packet.get_source(),
            dest_port: packet.get_destination(),
            flags,
            is_syn_ack: (flags & TcpFlags::SYN) != 0 && (flags & TcpFlags::ACK) != 0,
            is_rst: (flags & TcpFlags::RST) != 0,
        }
    }
}

/// ローカルIPアドレスを取得
pub fn get_local_ip(target: IpAddr) -> Result<Ipv4Addr> {
    // UDPソケットを使って接続先へのルートを確認
    let socket = std::net::UdpSocket::bind("0.0.0.0:0")
        .map_err(|e| NelstError::connection(format!("Failed to bind UDP socket: {}", e)))?;

    socket
        .connect((target, 80))
        .map_err(|e| NelstError::connection(format!("Failed to connect: {}", e)))?;

    let local_addr = socket
        .local_addr()
        .map_err(|e| NelstError::connection(format!("Failed to get local address: {}", e)))?;

    match local_addr.ip() {
        IpAddr::V4(ip) => Ok(ip),
        IpAddr::V6(_) => Err(NelstError::argument(
            "IPv6 is not supported for raw socket scanning".to_string(),
        )),
    }
}

/// ランダムなソースポートを生成
pub fn random_source_port() -> u16 {
    use rand::Rng;
    let mut rng = rand::thread_rng();
    rng.r#gen::<u16>() % 16384 + 49152 // 49152-65535
}

/// ランダムなシーケンス番号を生成
pub fn random_seq() -> u32 {
    use rand::Rng;
    let mut rng = rand::thread_rng();
    rng.r#gen()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_tcp_scan_flags() {
        assert_eq!(TcpScanFlags::Syn.flags(), TcpFlags::SYN);
        assert_eq!(TcpScanFlags::Fin.flags(), TcpFlags::FIN);
        assert_eq!(
            TcpScanFlags::Xmas.flags(),
            TcpFlags::FIN | TcpFlags::PSH | TcpFlags::URG
        );
        assert_eq!(TcpScanFlags::Null.flags(), 0);
    }

    #[test]
    fn test_tcp_scan_flags_name() {
        assert_eq!(TcpScanFlags::Syn.name(), "SYN");
        assert_eq!(TcpScanFlags::Fin.name(), "FIN");
        assert_eq!(TcpScanFlags::Xmas.name(), "Xmas");
        assert_eq!(TcpScanFlags::Null.name(), "NULL");
    }

    #[test]
    fn test_random_source_port() {
        for _ in 0..100 {
            let port = random_source_port();
            assert!(port >= 49152);
        }
    }

    #[test]
    fn test_build_tcp_packet() {
        let source_ip = Ipv4Addr::new(192, 168, 1, 100);
        let dest_ip = Ipv4Addr::new(192, 168, 1, 1);
        let packet = build_tcp_packet(source_ip, 12345, dest_ip, 80, TcpScanFlags::Syn, 1000);

        // TCPヘッダーは最小20バイト
        assert_eq!(packet.len(), 20);

        // パケットを解析
        let tcp = TcpPacket::new(&packet).unwrap();
        assert_eq!(tcp.get_source(), 12345);
        assert_eq!(tcp.get_destination(), 80);
        assert_eq!(tcp.get_sequence(), 1000);
        assert_eq!(tcp.get_flags(), TcpFlags::SYN);
    }

    #[test]
    fn test_tcp_response_from_packet() {
        // SYN-ACKパケットを作成
        let mut buffer = vec![0u8; 20];
        {
            let mut tcp = MutableTcpPacket::new(&mut buffer).unwrap();
            tcp.set_source(80);
            tcp.set_destination(12345);
            tcp.set_flags(TcpFlags::SYN | TcpFlags::ACK);
        }

        let tcp = TcpPacket::new(&buffer).unwrap();
        let response = TcpResponse::from_packet(&tcp);

        assert_eq!(response.source_port, 80);
        assert_eq!(response.dest_port, 12345);
        assert!(response.is_syn_ack);
        assert!(!response.is_rst);
    }

    #[test]
    fn test_tcp_response_rst_packet() {
        // RSTパケットを作成
        let mut buffer = vec![0u8; 20];
        {
            let mut tcp = MutableTcpPacket::new(&mut buffer).unwrap();
            tcp.set_source(443);
            tcp.set_destination(54321);
            tcp.set_flags(TcpFlags::RST);
        }

        let tcp = TcpPacket::new(&buffer).unwrap();
        let response = TcpResponse::from_packet(&tcp);

        assert_eq!(response.source_port, 443);
        assert!(!response.is_syn_ack);
        assert!(response.is_rst);
    }

    #[test]
    fn test_build_tcp_packet_all_scan_types() {
        let source_ip = Ipv4Addr::new(10, 0, 0, 1);
        let dest_ip = Ipv4Addr::new(10, 0, 0, 2);

        // FINスキャン
        let fin_packet = build_tcp_packet(source_ip, 1000, dest_ip, 80, TcpScanFlags::Fin, 100);
        let fin_tcp = TcpPacket::new(&fin_packet).unwrap();
        assert_eq!(fin_tcp.get_flags(), TcpFlags::FIN);

        // Xmasスキャン
        let xmas_packet = build_tcp_packet(source_ip, 1000, dest_ip, 80, TcpScanFlags::Xmas, 100);
        let xmas_tcp = TcpPacket::new(&xmas_packet).unwrap();
        assert_eq!(
            xmas_tcp.get_flags(),
            TcpFlags::FIN | TcpFlags::PSH | TcpFlags::URG
        );

        // NULLスキャン
        let null_packet = build_tcp_packet(source_ip, 1000, dest_ip, 80, TcpScanFlags::Null, 100);
        let null_tcp = TcpPacket::new(&null_packet).unwrap();
        assert_eq!(null_tcp.get_flags(), 0);
    }

    #[test]
    fn test_build_tcp_packet_checksum_nonzero() {
        let source_ip = Ipv4Addr::new(192, 168, 1, 1);
        let dest_ip = Ipv4Addr::new(192, 168, 1, 2);
        let packet = build_tcp_packet(source_ip, 12345, dest_ip, 80, TcpScanFlags::Syn, 1000);

        let tcp = TcpPacket::new(&packet).unwrap();
        // チェックサムが計算されていることを確認（0以外）
        assert_ne!(tcp.get_checksum(), 0);
    }
}
