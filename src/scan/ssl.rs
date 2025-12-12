//! SSL/TLS検査モジュール
//!
//! ターゲットのSSL/TLS設定を検査し、証明書情報を取得する。

use crate::common::error::{NelstError, Result};
use rustls::pki_types::ServerName;
use rustls::{ClientConfig, RootCertStore};
use serde::{Deserialize, Serialize};
use std::net::SocketAddr;
use std::sync::Arc;
use std::time::Duration;
use tokio::net::TcpStream;
use tokio::time::timeout;
use tokio_rustls::client::TlsStream;
use tokio_rustls::TlsConnector;
use tracing::{debug, warn};
use x509_parser::prelude::*;

/// SSL/TLS検査結果
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SslInfo {
    pub port: u16,
    pub tls_version: Option<String>,
    pub cipher_suite: Option<String>,
    pub certificate: Option<CertificateInfo>,
    pub chain_length: usize,
    pub is_valid: bool,
    pub errors: Vec<String>,
}

/// 証明書情報
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CertificateInfo {
    pub subject: String,
    pub issuer: String,
    pub serial_number: String,
    pub not_before: String,
    pub not_after: String,
    pub is_expired: bool,
    pub days_until_expiry: i64,
    pub san: Vec<String>,
    pub signature_algorithm: String,
    pub public_key_algorithm: String,
    pub public_key_bits: Option<u32>,
}

/// SSL/TLS検査を実行
pub async fn inspect_ssl(
    addr: SocketAddr,
    hostname: &str,
    timeout_ms: u64,
) -> Result<SslInfo> {
    let port = addr.port();
    let timeout_duration = Duration::from_millis(timeout_ms);

    // TLS接続を試行
    let result = timeout(timeout_duration, connect_tls(addr, hostname)).await;

    match result {
        Ok(Ok(info)) => Ok(info),
        Ok(Err(e)) => {
            debug!("SSL inspection failed for {}: {}", addr, e);
            Ok(SslInfo {
                port,
                tls_version: None,
                cipher_suite: None,
                certificate: None,
                chain_length: 0,
                is_valid: false,
                errors: vec![e.to_string()],
            })
        }
        Err(_) => {
            debug!("SSL inspection timed out for {}", addr);
            Ok(SslInfo {
                port,
                tls_version: None,
                cipher_suite: None,
                certificate: None,
                chain_length: 0,
                is_valid: false,
                errors: vec!["Connection timed out".to_string()],
            })
        }
    }
}

/// TLS接続を確立して情報を取得
async fn connect_tls(addr: SocketAddr, hostname: &str) -> Result<SslInfo> {
    let port = addr.port();
    
    // Root証明書ストアを作成
    let mut root_store = RootCertStore::empty();
    root_store.extend(webpki_roots::TLS_SERVER_ROOTS.iter().cloned());

    // TLS設定を作成
    let config = ClientConfig::builder()
        .with_root_certificates(root_store)
        .with_no_client_auth();

    let connector = TlsConnector::from(Arc::new(config));

    // TCP接続
    let stream = TcpStream::connect(addr).await.map_err(|e| {
        NelstError::scan(format!("TCP connection failed: {}", e))
    })?;

    // サーバー名を作成
    let server_name = ServerName::try_from(hostname.to_string()).map_err(|_| {
        NelstError::scan("Invalid hostname")
    })?;

    // TLSハンドシェイク
    let tls_stream: TlsStream<TcpStream> = match connector.connect(server_name, stream).await {
        Ok(stream) => stream,
        Err(e) => {
            // 証明書検証エラーでも情報は取得したい
            warn!("TLS handshake failed: {}", e);
            return Ok(SslInfo {
                port,
                tls_version: None,
                cipher_suite: None,
                certificate: None,
                chain_length: 0,
                is_valid: false,
                errors: vec![format!("TLS handshake failed: {}", e)],
            });
        }
    };

    // 接続情報を取得
    let (_, conn) = tls_stream.get_ref();

    let tls_version = conn.protocol_version().map(|v| format!("{:?}", v));
    let cipher_suite = conn.negotiated_cipher_suite().map(|c| format!("{:?}", c.suite()));

    // ピア証明書を取得
    let peer_certs = conn.peer_certificates();
    let chain_length = peer_certs.map(|c| c.len()).unwrap_or(0);

    let certificate = if let Some(certs) = peer_certs {
        if let Some(cert_der) = certs.first() {
            parse_certificate(cert_der.as_ref()).ok()
        } else {
            None
        }
    } else {
        None
    };

    Ok(SslInfo {
        port,
        tls_version,
        cipher_suite,
        certificate,
        chain_length,
        is_valid: true,
        errors: vec![],
    })
}

/// DER形式の証明書をパース
fn parse_certificate(der: &[u8]) -> Result<CertificateInfo> {
    let (_, cert) = X509Certificate::from_der(der).map_err(|e| {
        NelstError::scan(format!("Failed to parse certificate: {:?}", e))
    })?;

    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs() as i64;

    let not_after_timestamp = cert.validity().not_after.timestamp();
    let is_expired = not_after_timestamp < now;
    let days_until_expiry = (not_after_timestamp - now) / 86400;

    // SAN（Subject Alternative Names）を取得
    let san = cert
        .subject_alternative_name()
        .ok()
        .flatten()
        .map(|ext| {
            ext.value
                .general_names
                .iter()
                .filter_map(|name| match name {
                    GeneralName::DNSName(dns) => Some(dns.to_string()),
                    GeneralName::IPAddress(ip) => Some(format!("{:?}", ip)),
                    _ => None,
                })
                .collect()
        })
        .unwrap_or_default();

    // 公開鍵情報
    let public_key = cert.public_key();
    let public_key_algorithm = format!("{:?}", public_key.algorithm.algorithm);
    let public_key_bits = match public_key.parsed() {
        Ok(pk) => match pk {
            x509_parser::public_key::PublicKey::RSA(rsa) => Some(rsa.key_size() as u32),
            x509_parser::public_key::PublicKey::EC(ec) => Some(ec.key_size() as u32),
            _ => None,
        },
        Err(_) => None,
    };

    Ok(CertificateInfo {
        subject: cert.subject().to_string(),
        issuer: cert.issuer().to_string(),
        serial_number: cert.serial.to_string(),
        not_before: cert.validity().not_before.to_rfc2822().unwrap_or_else(|_| "Unknown".to_string()),
        not_after: cert.validity().not_after.to_rfc2822().unwrap_or_else(|_| "Unknown".to_string()),
        is_expired,
        days_until_expiry,
        san,
        signature_algorithm: format!("{:?}", cert.signature_algorithm.algorithm),
        public_key_algorithm,
        public_key_bits,
    })
}

/// 複数ポートのSSL検査を実行
pub async fn inspect_ssl_ports(
    target: std::net::IpAddr,
    ports: &[u16],
    hostname: &str,
    timeout_ms: u64,
    concurrency: usize,
) -> Vec<SslInfo> {
    use std::sync::Arc;
    use tokio::sync::Semaphore;

    let semaphore = Arc::new(Semaphore::new(concurrency));
    let hostname = hostname.to_string();
    let mut handles = Vec::new();

    for &port in ports {
        let semaphore = semaphore.clone();
        let hostname = hostname.clone();
        let addr = SocketAddr::new(target, port);

        let handle = tokio::spawn(async move {
            let _permit = semaphore.acquire().await.unwrap();
            inspect_ssl(addr, &hostname, timeout_ms).await.ok()
        });

        handles.push(handle);
    }

    let mut results = Vec::new();
    for handle in handles {
        if let Ok(Some(info)) = handle.await {
            results.push(info);
        }
    }

    results
}

/// SSL/TLS検査結果を表示
#[allow(dead_code)]
pub fn format_ssl_info(info: &SslInfo) -> String {
    let mut output = String::new();

    output.push_str(&format!("Port: {}\n", info.port));

    if let Some(ref version) = info.tls_version {
        output.push_str(&format!("TLS Version: {}\n", version));
    }

    if let Some(ref cipher) = info.cipher_suite {
        output.push_str(&format!("Cipher Suite: {}\n", cipher));
    }

    output.push_str(&format!("Chain Length: {}\n", info.chain_length));
    output.push_str(&format!("Valid: {}\n", info.is_valid));

    if let Some(ref cert) = info.certificate {
        output.push_str("\nCertificate:\n");
        output.push_str(&format!("  Subject: {}\n", cert.subject));
        output.push_str(&format!("  Issuer: {}\n", cert.issuer));
        output.push_str(&format!("  Serial: {}\n", cert.serial_number));
        output.push_str(&format!("  Not Before: {}\n", cert.not_before));
        output.push_str(&format!("  Not After: {}\n", cert.not_after));
        output.push_str(&format!("  Expired: {}\n", cert.is_expired));
        output.push_str(&format!("  Days Until Expiry: {}\n", cert.days_until_expiry));
        if !cert.san.is_empty() {
            output.push_str(&format!("  SAN: {}\n", cert.san.join(", ")));
        }
        output.push_str(&format!("  Signature Algorithm: {}\n", cert.signature_algorithm));
        output.push_str(&format!("  Public Key Algorithm: {}\n", cert.public_key_algorithm));
        if let Some(bits) = cert.public_key_bits {
            output.push_str(&format!("  Public Key Bits: {}\n", bits));
        }
    }

    if !info.errors.is_empty() {
        output.push_str("\nErrors:\n");
        for error in &info.errors {
            output.push_str(&format!("  - {}\n", error));
        }
    }

    output
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_inspect_ssl_invalid_port() {
        // CryptoProviderを設定
        let _ = rustls::crypto::ring::default_provider().install_default();
        
        // 未使用ポートへのSSL検査
        let addr = SocketAddr::new("127.0.0.1".parse().unwrap(), 59999);
        let result = inspect_ssl(addr, "localhost", 1000).await;
        assert!(result.is_ok());
        let info = result.unwrap();
        assert!(!info.is_valid);
        assert!(!info.errors.is_empty());
    }

    #[test]
    fn test_format_ssl_info() {
        let info = SslInfo {
            port: 443,
            tls_version: Some("TLSv1.3".to_string()),
            cipher_suite: Some("TLS_AES_256_GCM_SHA384".to_string()),
            certificate: Some(CertificateInfo {
                subject: "CN=example.com".to_string(),
                issuer: "CN=Let's Encrypt".to_string(),
                serial_number: "12345".to_string(),
                not_before: "2024-01-01".to_string(),
                not_after: "2025-01-01".to_string(),
                is_expired: false,
                days_until_expiry: 365,
                san: vec!["example.com".to_string(), "www.example.com".to_string()],
                signature_algorithm: "sha256WithRSAEncryption".to_string(),
                public_key_algorithm: "rsaEncryption".to_string(),
                public_key_bits: Some(2048),
            }),
            chain_length: 2,
            is_valid: true,
            errors: vec![],
        };

        let output = format_ssl_info(&info);
        assert!(output.contains("Port: 443"));
        assert!(output.contains("TLSv1.3"));
        assert!(output.contains("example.com"));
    }

    #[test]
    fn test_ssl_info_default_invalid() {
        let info = SslInfo {
            port: 443,
            tls_version: None,
            cipher_suite: None,
            certificate: None,
            chain_length: 0,
            is_valid: false,
            errors: vec!["Connection refused".to_string()],
        };
        assert!(!info.is_valid);
        assert_eq!(info.errors.len(), 1);
        assert_eq!(info.chain_length, 0);
    }

    #[test]
    fn test_certificate_info_expired() {
        let cert = CertificateInfo {
            subject: "CN=expired.example.com".to_string(),
            issuer: "CN=Test CA".to_string(),
            serial_number: "123".to_string(),
            not_before: "2020-01-01".to_string(),
            not_after: "2021-01-01".to_string(),
            is_expired: true,
            days_until_expiry: -365,
            san: vec![],
            signature_algorithm: "sha256".to_string(),
            public_key_algorithm: "RSA".to_string(),
            public_key_bits: Some(2048),
        };
        assert!(cert.is_expired);
        assert!(cert.days_until_expiry < 0);
    }

    #[test]
    fn test_certificate_info_with_san() {
        let cert = CertificateInfo {
            subject: "CN=example.com".to_string(),
            issuer: "CN=Test CA".to_string(),
            serial_number: "456".to_string(),
            not_before: "2024-01-01".to_string(),
            not_after: "2025-01-01".to_string(),
            is_expired: false,
            days_until_expiry: 365,
            san: vec![
                "example.com".to_string(),
                "www.example.com".to_string(),
                "api.example.com".to_string(),
            ],
            signature_algorithm: "sha256".to_string(),
            public_key_algorithm: "RSA".to_string(),
            public_key_bits: Some(4096),
        };
        assert_eq!(cert.san.len(), 3);
        assert!(cert.san.contains(&"api.example.com".to_string()));
        assert_eq!(cert.public_key_bits, Some(4096));
    }

    #[test]
    fn test_format_ssl_info_with_errors() {
        let info = SslInfo {
            port: 8443,
            tls_version: None,
            cipher_suite: None,
            certificate: None,
            chain_length: 0,
            is_valid: false,
            errors: vec![
                "Connection refused".to_string(),
                "Timeout".to_string(),
            ],
        };
        let output = format_ssl_info(&info);
        assert!(output.contains("Port: 8443"));
        assert!(output.contains("Valid: false"));
        assert!(output.contains("Connection refused"));
        assert!(output.contains("Timeout"));
    }
}
