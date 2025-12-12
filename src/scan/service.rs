//! サービス検出モジュール
//!
//! オープンポートで動作しているサービスを特定する。

use crate::common::error::Result;
use serde::{Deserialize, Serialize};
use std::net::SocketAddr;
use std::time::Duration;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;
use tokio::time::timeout;

/// サービス情報
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServiceInfo {
    pub port: u16,
    pub name: Option<String>,
    pub version: Option<String>,
    pub banner: Option<String>,
    pub product: Option<String>,
}

/// バナー取得とサービス検出
pub async fn detect_service(
    addr: SocketAddr,
    timeout_ms: u64,
    grab_banner: bool,
) -> Result<ServiceInfo> {
    let port = addr.port();
    let timeout_duration = Duration::from_millis(timeout_ms);

    // まずバナー取得を試みる
    let banner = if grab_banner {
        timeout(timeout_duration, grab_banner_data(addr))
            .await
            .ok()
            .flatten()
    } else {
        None
    };

    // サービス名を推測
    let (name, version, product) = identify_service(port, banner.as_deref());

    Ok(ServiceInfo {
        port,
        name,
        version,
        banner,
        product,
    })
}

/// バナーを取得
async fn grab_banner_data(addr: SocketAddr) -> Option<String> {
    let mut stream = TcpStream::connect(addr).await.ok()?;

    // ポートに応じたプローブを送信
    let probe = get_probe_data(addr.port());
    if let Some(probe_data) = probe {
        stream.write_all(probe_data).await.ok()?;
    }

    // 応答を読み取り
    let mut buffer = vec![0u8; 1024];
    let timeout_result = timeout(Duration::from_millis(2000), stream.read(&mut buffer)).await;

    match timeout_result {
        Ok(Ok(n)) if n > 0 => {
            let banner = String::from_utf8_lossy(&buffer[..n]).trim().to_string();
            if !banner.is_empty() {
                Some(banner)
            } else {
                None
            }
        }
        _ => None,
    }
}

/// ポートに応じたプローブデータを取得
fn get_probe_data(port: u16) -> Option<&'static [u8]> {
    match port {
        // HTTP
        80 | 8080 | 8000 | 8888 => Some(b"GET / HTTP/1.0\r\n\r\n"),
        // HTTPS (TLSなのでプローブ不要)
        443 | 8443 => None,
        // SMTP
        25 | 587 | 465 => Some(b"EHLO test\r\n"),
        // FTP
        21 => None, // 接続時にバナーが返る
        // SSH
        22 => None, // 接続時にバナーが返る
        // Telnet
        23 => None,
        // MySQL
        3306 => None, // 接続時にハンドシェイクが返る
        // PostgreSQL
        5432 => None,
        // Redis
        6379 => Some(b"PING\r\n"),
        // MongoDB
        27017 => None,
        // その他
        _ => None,
    }
}

/// バナーからサービスを識別
fn identify_service(
    port: u16,
    banner: Option<&str>,
) -> (Option<String>, Option<String>, Option<String>) {
    // まずバナーから判定
    if let Some(b) = banner {
        let b_lower = b.to_lowercase();

        // SSH
        if b.starts_with("SSH-") {
            let parts: Vec<&str> = b.split('-').collect();
            let version = if parts.len() >= 3 {
                Some(parts[2..].join("-"))
            } else {
                None
            };
            return (
                Some("ssh".to_string()),
                version,
                Some("OpenSSH".to_string()),
            );
        }

        // HTTP
        if b.starts_with("HTTP/") {
            let server = extract_http_server(b);
            return (Some("http".to_string()), None, server);
        }

        // FTP
        if b.starts_with("220")
            && (b_lower.contains("ftp")
                || b_lower.contains("filezilla")
                || b_lower.contains("vsftpd"))
        {
            let version = extract_ftp_version(b);
            return (Some("ftp".to_string()), version.clone(), version);
        }

        // SMTP
        if b.starts_with("220")
            && (b_lower.contains("smtp")
                || b_lower.contains("esmtp")
                || b_lower.contains("postfix")
                || b_lower.contains("sendmail"))
        {
            return (Some("smtp".to_string()), None, extract_smtp_server(b));
        }

        // MySQL
        if b.contains("mysql") || (b.len() > 4 && b.as_bytes()[4] == 0x0a) {
            return (
                Some("mysql".to_string()),
                extract_mysql_version(b),
                Some("MySQL".to_string()),
            );
        }

        // Redis
        if b.starts_with("+PONG") {
            return (Some("redis".to_string()), None, Some("Redis".to_string()));
        }

        // PostgreSQL
        if b_lower.contains("postgresql") {
            return (
                Some("postgresql".to_string()),
                None,
                Some("PostgreSQL".to_string()),
            );
        }

        // MongoDB
        if b_lower.contains("mongodb") || b.contains("ismaster") {
            return (
                Some("mongodb".to_string()),
                None,
                Some("MongoDB".to_string()),
            );
        }
    }

    // ポート番号からデフォルトサービスを推測
    let default_service = get_default_service(port);
    (default_service, None, None)
}

/// ポート番号からデフォルトサービス名を取得
fn get_default_service(port: u16) -> Option<String> {
    let service = match port {
        20 => "ftp-data",
        21 => "ftp",
        22 => "ssh",
        23 => "telnet",
        25 => "smtp",
        53 => "dns",
        80 => "http",
        110 => "pop3",
        111 => "rpcbind",
        135 => "msrpc",
        139 => "netbios-ssn",
        143 => "imap",
        443 => "https",
        445 => "microsoft-ds",
        465 => "smtps",
        587 => "submission",
        993 => "imaps",
        995 => "pop3s",
        1433 => "ms-sql-s",
        1521 => "oracle",
        3306 => "mysql",
        3389 => "ms-wbt-server",
        5432 => "postgresql",
        5900 => "vnc",
        6379 => "redis",
        8080 => "http-proxy",
        8443 => "https-alt",
        27017 => "mongodb",
        _ => return None,
    };
    Some(service.to_string())
}

/// HTTPサーバヘッダーを抽出
fn extract_http_server(banner: &str) -> Option<String> {
    for line in banner.lines() {
        let lower = line.to_lowercase();
        if lower.starts_with("server:") {
            return Some(line[7..].trim().to_string());
        }
    }
    None
}

/// FTPバージョンを抽出
fn extract_ftp_version(banner: &str) -> Option<String> {
    // "220-FileZilla Server 1.0.0" のようなパターン
    let parts: Vec<&str> = banner.splitn(2, ' ').collect();
    if parts.len() >= 2 {
        Some(parts[1].trim().to_string())
    } else {
        None
    }
}

/// SMTPサーバを抽出
fn extract_smtp_server(banner: &str) -> Option<String> {
    // "220 mail.example.com ESMTP Postfix" のようなパターン
    if banner.len() > 4 {
        Some(banner[4..].trim().to_string())
    } else {
        None
    }
}

/// MySQLバージョンを抽出
fn extract_mysql_version(banner: &str) -> Option<String> {
    // MySQLのハンドシェイクパケットからバージョンを抽出するのは複雑
    // 簡易的に文字列からマッチ
    if let Some(start) = banner.find(char::is_numeric) {
        let version_part: String = banner[start..]
            .chars()
            .take_while(|c| c.is_numeric() || *c == '.')
            .collect();
        if !version_part.is_empty() {
            return Some(version_part);
        }
    }
    None
}

/// 複数ポートのサービス検出を実行
pub async fn detect_services(
    target: std::net::IpAddr,
    ports: &[u16],
    timeout_ms: u64,
    grab_banner: bool,
    concurrency: usize,
) -> Vec<ServiceInfo> {
    use std::sync::Arc;
    use tokio::sync::Semaphore;

    let semaphore = Arc::new(Semaphore::new(concurrency));
    let mut handles = Vec::new();

    for &port in ports {
        let semaphore = semaphore.clone();
        let addr = SocketAddr::new(target, port);

        let handle = tokio::spawn(async move {
            let _permit = semaphore.acquire().await.unwrap();
            detect_service(addr, timeout_ms, grab_banner).await.ok()
        });

        handles.push(handle);
    }

    let mut results = Vec::new();
    for handle in handles {
        if let Ok(Some(service)) = handle.await {
            results.push(service);
        }
    }

    results
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_get_default_service() {
        assert_eq!(get_default_service(22), Some("ssh".to_string()));
        assert_eq!(get_default_service(80), Some("http".to_string()));
        assert_eq!(get_default_service(443), Some("https".to_string()));
        assert_eq!(get_default_service(3306), Some("mysql".to_string()));
        assert_eq!(get_default_service(65000), None);
    }

    #[test]
    fn test_identify_service_from_banner() {
        // SSHバナー
        let (name, version, _) = identify_service(22, Some("SSH-2.0-OpenSSH_8.9"));
        assert_eq!(name, Some("ssh".to_string()));
        assert!(version.is_some());

        // HTTPレスポンス
        let (name, _, server) = identify_service(80, Some("HTTP/1.1 200 OK\r\nServer: nginx/1.18.0"));
        assert_eq!(name, Some("http".to_string()));
        assert_eq!(server, Some("nginx/1.18.0".to_string()));

        // Redis
        let (name, _, product) = identify_service(6379, Some("+PONG"));
        assert_eq!(name, Some("redis".to_string()));
        assert_eq!(product, Some("Redis".to_string()));
    }

    #[test]
    fn test_identify_service_from_port_only() {
        let (name, _, _) = identify_service(22, None);
        assert_eq!(name, Some("ssh".to_string()));

        let (name, _, _) = identify_service(443, None);
        assert_eq!(name, Some("https".to_string()));
    }

    #[test]
    fn test_get_probe_data() {
        assert!(get_probe_data(80).is_some()); // HTTP
        assert!(get_probe_data(22).is_none()); // SSH (no probe needed)
        assert!(get_probe_data(6379).is_some()); // Redis PING
    }

    #[test]
    fn test_identify_service_ftp_banner() {
        // FTPバナー
        let (name, version, _) = identify_service(21, Some("220-FileZilla Server 1.0.0"));
        assert_eq!(name, Some("ftp".to_string()));
        assert!(version.is_some());
    }

    #[test]
    fn test_identify_service_smtp_banner() {
        // SMTPバナー
        let (name, _, product) = identify_service(25, Some("220 mail.example.com ESMTP Postfix"));
        assert_eq!(name, Some("smtp".to_string()));
        assert!(product.is_some());
    }

    #[test]
    fn test_identify_service_mysql_banner() {
        // MySQLは特定のバイナリプロトコルだが、テキストを含む場合もある
        let (name, _, product) = identify_service(3306, Some("5.7.32-mysql"));
        assert_eq!(name, Some("mysql".to_string()));
        assert_eq!(product, Some("MySQL".to_string()));
    }

    #[test]
    fn test_identify_service_unknown_port_no_banner() {
        // 未知のポートでバナーなし
        let (name, version, product) = identify_service(54321, None);
        assert!(name.is_none());
        assert!(version.is_none());
        assert!(product.is_none());
    }

    #[test]
    fn test_service_info_struct() {
        let info = ServiceInfo {
            port: 22,
            name: Some("ssh".to_string()),
            version: Some("OpenSSH_8.9".to_string()),
            banner: Some("SSH-2.0-OpenSSH_8.9".to_string()),
            product: Some("OpenSSH".to_string()),
        };
        assert_eq!(info.port, 22);
        assert_eq!(info.name, Some("ssh".to_string()));
    }
}
