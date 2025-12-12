//! セキュリティスキャンサブコマンドの定義

#![allow(dead_code)]

use clap::{Args, Subcommand, ValueEnum};
use std::net::IpAddr;

/// セキュリティスキャンのサブコマンド
#[derive(Subcommand, Debug)]
pub enum ScanCommands {
    /// ポートスキャン
    Port(PortScanArgs),
}

/// スキャン手法
#[derive(ValueEnum, Clone, Debug, Default)]
pub enum ScanMethod {
    /// TCP Connectスキャン（root権限不要）
    #[default]
    Tcp,
    /// SYNスキャン（要root権限）
    Syn,
    /// FINスキャン（要root権限）
    Fin,
    /// Xmasスキャン（要root権限）
    Xmas,
    /// NULLスキャン（要root権限）
    Null,
    /// UDPスキャン
    Udp,
}

/// ポートスキャンの引数
#[derive(Args, Debug)]
pub struct PortScanArgs {
    /// ターゲットホスト (例: 192.168.1.100)
    #[arg(short, long)]
    pub target: IpAddr,

    /// スキャン手法
    #[arg(short, long, value_enum, default_value_t = ScanMethod::Tcp)]
    pub method: ScanMethod,

    /// ポート範囲 (例: 1-1024, 80,443,8080)
    #[arg(long, default_value = "1-1024")]
    pub ports: String,

    /// 並列スキャン数
    #[arg(short, long, default_value_t = 100)]
    pub concurrency: usize,

    /// タイムアウト（ミリ秒）
    #[arg(long, default_value_t = 1000)]
    pub timeout: u64,

    /// よく使われるポート上位N件のみ
    #[arg(long)]
    pub top_ports: Option<usize>,

    /// サービス検出を有効化
    #[arg(long)]
    pub service_detection: bool,

    /// バナー取得を有効化（サービス検出時に使用）
    #[arg(long)]
    pub grab_banner: bool,

    /// SSL/TLS検査を有効化
    #[arg(long)]
    pub ssl_check: bool,

    /// ホスト名（SSL証明書検証用）
    #[arg(long)]
    pub hostname: Option<String>,

    /// 結果出力ファイル
    #[arg(short, long, value_name = "FILE")]
    pub output: Option<String>,
}

/// ポート範囲をパースしてVec<u16>に変換
pub fn parse_ports(ports_str: &str) -> Result<Vec<u16>, String> {
    let mut result = Vec::new();

    for part in ports_str.split(',') {
        let part = part.trim();
        if part.contains('-') {
            // 範囲指定 (例: 1-1024)
            let range: Vec<&str> = part.split('-').collect();
            if range.len() != 2 {
                return Err(format!("Invalid port range: {}", part));
            }
            let start: u16 = range[0]
                .trim()
                .parse()
                .map_err(|_| format!("Invalid port number: {}", range[0]))?;
            let end: u16 = range[1]
                .trim()
                .parse()
                .map_err(|_| format!("Invalid port number: {}", range[1]))?;
            if start > end {
                return Err(format!("Invalid port range: {} > {}", start, end));
            }
            result.extend(start..=end);
        } else {
            // 単一ポート
            let port: u16 = part
                .parse()
                .map_err(|_| format!("Invalid port number: {}", part))?;
            result.push(port);
        }
    }

    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_ports_range() {
        let ports = parse_ports("1-100").unwrap();
        assert_eq!(ports.len(), 100);
        assert_eq!(ports[0], 1);
        assert_eq!(ports[99], 100);
    }

    #[test]
    fn test_parse_ports_list() {
        let ports = parse_ports("22,80,443").unwrap();
        assert_eq!(ports, vec![22, 80, 443]);
    }

    #[test]
    fn test_parse_ports_mixed() {
        let ports = parse_ports("22,80-82,443").unwrap();
        assert_eq!(ports, vec![22, 80, 81, 82, 443]);
    }

    #[test]
    fn test_parse_ports_spaces() {
        let ports = parse_ports(" 22 , 80 , 443 ").unwrap();
        assert_eq!(ports, vec![22, 80, 443]);
    }

    #[test]
    fn test_parse_ports_invalid() {
        assert!(parse_ports("invalid").is_err());
        assert!(parse_ports("100-50").is_err());
    }
}
