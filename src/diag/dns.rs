//! DNS解決実装
//!
//! DNS名前解決とレコード取得を行う。

use crate::cli::diag::{DnsArgs, DnsRecordType};
use crate::common::error::NelstError;
use hickory_resolver::Resolver;
use hickory_resolver::config::{NameServerConfig, ResolverConfig};
use hickory_resolver::name_server::TokioConnectionProvider;
use hickory_resolver::proto::rr::RecordType;
use hickory_resolver::proto::xfer::Protocol;
use serde::Serialize;
use std::net::{IpAddr, SocketAddr};
use std::time::{Duration, Instant};
use tracing::{debug, info};

/// TokioResolverの型エイリアス
type TokioResolver = Resolver<TokioConnectionProvider>;

/// DNSレコード
#[derive(Debug, Clone, Serialize)]
pub struct DnsRecord {
    /// レコードタイプ
    pub record_type: String,
    /// レコード値
    pub value: String,
    /// TTL（秒）
    pub ttl: u32,
}

/// DNS解決結果
#[derive(Debug, Clone, Serialize)]
pub struct DnsResult {
    /// クエリ対象
    pub query: String,
    /// レコードタイプ
    pub query_type: String,
    /// 使用したDNSサーバ
    pub dns_server: String,
    /// プロトコル
    pub protocol: String,
    /// 解決時間（ミリ秒）
    pub resolve_time_ms: f64,
    /// レコード一覧
    pub records: Vec<DnsRecord>,
    /// エラーメッセージ（あれば）
    pub error: Option<String>,
}

/// DnsRecordTypeをRecordTypeに変換
fn to_record_type(rt: &DnsRecordType) -> Vec<RecordType> {
    match rt {
        DnsRecordType::A => vec![RecordType::A],
        DnsRecordType::Aaaa => vec![RecordType::AAAA],
        DnsRecordType::Mx => vec![RecordType::MX],
        DnsRecordType::Txt => vec![RecordType::TXT],
        DnsRecordType::Ns => vec![RecordType::NS],
        DnsRecordType::Cname => vec![RecordType::CNAME],
        DnsRecordType::Soa => vec![RecordType::SOA],
        DnsRecordType::Ptr => vec![RecordType::PTR],
        DnsRecordType::All => vec![
            RecordType::A,
            RecordType::AAAA,
            RecordType::MX,
            RecordType::TXT,
            RecordType::NS,
            RecordType::CNAME,
        ],
    }
}

/// リゾルバを作成
fn create_resolver(
    server: Option<IpAddr>,
    use_tcp: bool,
    _timeout: Duration,
) -> Result<TokioResolver, NelstError> {
    let config = if let Some(server_ip) = server {
        let protocol = if use_tcp {
            Protocol::Tcp
        } else {
            Protocol::Udp
        };
        let socket_addr = SocketAddr::new(server_ip, 53);
        let ns_config = NameServerConfig::new(socket_addr, protocol);
        ResolverConfig::from_parts(None, vec![], vec![ns_config])
    } else {
        ResolverConfig::default()
    };

    let builder = Resolver::builder_with_config(config, TokioConnectionProvider::default());
    Ok(builder.build())
}

/// DNS解決を実行
pub async fn run(args: &DnsArgs) -> Result<DnsResult, NelstError> {
    let timeout = Duration::from_millis(args.timeout);
    let protocol = if args.tcp { "TCP" } else { "UDP" };
    let dns_server = args
        .server
        .map(|s| s.to_string())
        .unwrap_or_else(|| "system default".to_string());

    info!(
        "DNS lookup for {} (type: {:?}, server: {}, protocol: {})",
        args.target, args.record_type, dns_server, protocol
    );

    let resolver = create_resolver(args.server, args.tcp, timeout)?;
    let record_types = to_record_type(&args.record_type);
    let mut all_records = Vec::new();
    let mut total_time = 0.0;
    let mut last_error = None;

    for rt in record_types {
        let start = Instant::now();

        match lookup_records(&resolver, &args.target, rt).await {
            Ok(records) => {
                let elapsed = start.elapsed().as_secs_f64() * 1000.0;
                total_time += elapsed;
                debug!(
                    "Found {} {:?} records in {:.2}ms",
                    records.len(),
                    rt,
                    elapsed
                );
                all_records.extend(records);
            }
            Err(e) => {
                let elapsed = start.elapsed().as_secs_f64() * 1000.0;
                total_time += elapsed;
                debug!("No {:?} records found: {}", rt, e);
                last_error = Some(e.to_string());
            }
        }
    }

    let result = DnsResult {
        query: args.target.clone(),
        query_type: format!("{:?}", args.record_type),
        dns_server,
        protocol: protocol.to_string(),
        resolve_time_ms: total_time,
        records: all_records.clone(),
        error: if all_records.is_empty() {
            last_error
        } else {
            None
        },
    };

    if !all_records.is_empty() {
        info!(
            "Found {} records for {} in {:.2}ms",
            all_records.len(),
            args.target,
            total_time
        );
    }

    Ok(result)
}

/// 特定のレコードタイプを検索
async fn lookup_records(
    resolver: &TokioResolver,
    name: &str,
    record_type: RecordType,
) -> Result<Vec<DnsRecord>, NelstError> {
    let mut records = Vec::new();

    match record_type {
        RecordType::A => {
            let lookup = resolver
                .ipv4_lookup(name)
                .await
                .map_err(|e| NelstError::connection(format!("A record lookup failed: {}", e)))?;
            for ip in lookup.iter() {
                records.push(DnsRecord {
                    record_type: "A".to_string(),
                    value: ip.to_string(),
                    ttl: 0,
                });
            }
        }
        RecordType::AAAA => {
            let lookup = resolver
                .ipv6_lookup(name)
                .await
                .map_err(|e| NelstError::connection(format!("AAAA record lookup failed: {}", e)))?;
            for ip in lookup.iter() {
                records.push(DnsRecord {
                    record_type: "AAAA".to_string(),
                    value: ip.to_string(),
                    ttl: 0,
                });
            }
        }
        RecordType::MX => {
            let lookup = resolver
                .mx_lookup(name)
                .await
                .map_err(|e| NelstError::connection(format!("MX record lookup failed: {}", e)))?;
            for mx in lookup.iter() {
                records.push(DnsRecord {
                    record_type: "MX".to_string(),
                    value: format!("{} {}", mx.preference(), mx.exchange()),
                    ttl: 0,
                });
            }
        }
        RecordType::TXT => {
            let lookup = resolver
                .txt_lookup(name)
                .await
                .map_err(|e| NelstError::connection(format!("TXT record lookup failed: {}", e)))?;
            for txt in lookup.iter() {
                let txt_data: String = txt
                    .iter()
                    .map(|d| String::from_utf8_lossy(d).to_string())
                    .collect::<Vec<_>>()
                    .join("");
                records.push(DnsRecord {
                    record_type: "TXT".to_string(),
                    value: txt_data,
                    ttl: 0,
                });
            }
        }
        RecordType::NS => {
            let lookup = resolver
                .ns_lookup(name)
                .await
                .map_err(|e| NelstError::connection(format!("NS record lookup failed: {}", e)))?;
            for ns in lookup.iter() {
                records.push(DnsRecord {
                    record_type: "NS".to_string(),
                    value: ns.to_string(),
                    ttl: 0,
                });
            }
        }
        RecordType::CNAME => {
            let lookup = resolver
                .lookup(name, RecordType::CNAME)
                .await
                .map_err(|e| {
                    NelstError::connection(format!("CNAME record lookup failed: {}", e))
                })?;
            for record in lookup.record_iter() {
                let rdata = record.data();
                if let Some(cname) = rdata.as_cname() {
                    records.push(DnsRecord {
                        record_type: "CNAME".to_string(),
                        value: cname.to_string(),
                        ttl: record.ttl(),
                    });
                }
            }
        }
        RecordType::SOA => {
            let lookup = resolver
                .soa_lookup(name)
                .await
                .map_err(|e| NelstError::connection(format!("SOA record lookup failed: {}", e)))?;
            for soa in lookup.iter() {
                records.push(DnsRecord {
                    record_type: "SOA".to_string(),
                    value: format!(
                        "{} {} {} {} {} {} {}",
                        soa.mname(),
                        soa.rname(),
                        soa.serial(),
                        soa.refresh(),
                        soa.retry(),
                        soa.expire(),
                        soa.minimum()
                    ),
                    ttl: 0,
                });
            }
        }
        RecordType::PTR => {
            let ip_addr: IpAddr = name.parse().map_err(|_| {
                NelstError::config(format!("Invalid IP address for PTR lookup: {}", name))
            })?;
            let lookup = resolver
                .reverse_lookup(ip_addr)
                .await
                .map_err(|e| NelstError::connection(format!("PTR record lookup failed: {}", e)))?;
            for ptr in lookup.iter() {
                records.push(DnsRecord {
                    record_type: "PTR".to_string(),
                    value: ptr.to_string(),
                    ttl: 0,
                });
            }
        }
        _ => {}
    }

    Ok(records)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_to_record_type_a() {
        let types = to_record_type(&DnsRecordType::A);
        assert_eq!(types.len(), 1);
        assert!(matches!(types[0], RecordType::A));
    }

    #[test]
    fn test_to_record_type_aaaa() {
        let types = to_record_type(&DnsRecordType::Aaaa);
        assert_eq!(types.len(), 1);
        assert!(matches!(types[0], RecordType::AAAA));
    }

    #[test]
    fn test_to_record_type_mx() {
        let types = to_record_type(&DnsRecordType::Mx);
        assert_eq!(types.len(), 1);
        assert!(matches!(types[0], RecordType::MX));
    }

    #[test]
    fn test_to_record_type_txt() {
        let types = to_record_type(&DnsRecordType::Txt);
        assert_eq!(types.len(), 1);
        assert!(matches!(types[0], RecordType::TXT));
    }

    #[test]
    fn test_to_record_type_ns() {
        let types = to_record_type(&DnsRecordType::Ns);
        assert_eq!(types.len(), 1);
        assert!(matches!(types[0], RecordType::NS));
    }

    #[test]
    fn test_to_record_type_cname() {
        let types = to_record_type(&DnsRecordType::Cname);
        assert_eq!(types.len(), 1);
        assert!(matches!(types[0], RecordType::CNAME));
    }

    #[test]
    fn test_to_record_type_soa() {
        let types = to_record_type(&DnsRecordType::Soa);
        assert_eq!(types.len(), 1);
        assert!(matches!(types[0], RecordType::SOA));
    }

    #[test]
    fn test_to_record_type_ptr() {
        let types = to_record_type(&DnsRecordType::Ptr);
        assert_eq!(types.len(), 1);
        assert!(matches!(types[0], RecordType::PTR));
    }

    #[test]
    fn test_to_record_type_all() {
        let types = to_record_type(&DnsRecordType::All);
        assert!(types.len() > 1);
        assert!(types.contains(&RecordType::A));
        assert!(types.contains(&RecordType::AAAA));
        assert!(types.contains(&RecordType::MX));
        assert!(types.contains(&RecordType::TXT));
        assert!(types.contains(&RecordType::NS));
        assert!(types.contains(&RecordType::CNAME));
    }

    #[test]
    fn test_dns_record() {
        let record = DnsRecord {
            record_type: "A".to_string(),
            value: "93.184.216.34".to_string(),
            ttl: 3600,
        };
        assert_eq!(record.record_type, "A");
        assert_eq!(record.ttl, 3600);
    }

    #[test]
    fn test_dns_record_mx() {
        let record = DnsRecord {
            record_type: "MX".to_string(),
            value: "10 mail.example.com".to_string(),
            ttl: 7200,
        };
        assert_eq!(record.record_type, "MX");
        assert!(record.value.contains("mail"));
    }

    #[test]
    fn test_dns_result() {
        let result = DnsResult {
            query: "example.com".to_string(),
            query_type: "A".to_string(),
            dns_server: "8.8.8.8".to_string(),
            protocol: "UDP".to_string(),
            resolve_time_ms: 25.5,
            records: vec![],
            error: None,
        };
        assert_eq!(result.query, "example.com");
        assert!(result.error.is_none());
    }

    #[test]
    fn test_dns_result_with_error() {
        let result = DnsResult {
            query: "nonexistent.invalid".to_string(),
            query_type: "A".to_string(),
            dns_server: "8.8.8.8".to_string(),
            protocol: "UDP".to_string(),
            resolve_time_ms: 100.0,
            records: vec![],
            error: Some("NXDOMAIN".to_string()),
        };
        assert!(result.records.is_empty());
        assert!(result.error.is_some());
    }

    #[test]
    fn test_dns_result_with_multiple_records() {
        let result = DnsResult {
            query: "google.com".to_string(),
            query_type: "A".to_string(),
            dns_server: "system default".to_string(),
            protocol: "UDP".to_string(),
            resolve_time_ms: 15.0,
            records: vec![
                DnsRecord {
                    record_type: "A".to_string(),
                    value: "142.250.80.46".to_string(),
                    ttl: 300,
                },
                DnsRecord {
                    record_type: "A".to_string(),
                    value: "142.250.80.47".to_string(),
                    ttl: 300,
                },
            ],
            error: None,
        };
        assert_eq!(result.records.len(), 2);
    }
}
