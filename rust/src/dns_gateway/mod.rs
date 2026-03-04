//! A DNS gateway filter that intercepts DNS queries and returns virtual IPs for matched domains.
//!
//! This filter demonstrates:
//! 1. UDP listener filter structure with `UdpListenerFilterConfig` and `UdpListenerFilter` traits.
//! 2. DNS query parsing and response.
//! 3. Using protobof for configuration.
//!
//! See dns_gateway.proto for the protobuf definitions of the config.

pub mod cache_lookup;
mod proto;
mod virtual_ip_cache;

use envoy_proxy_dynamic_modules_rust_sdk::*;
use hickory_proto::op::{Message, MessageType, ResponseCode};
use hickory_proto::rr::{Name, RData, Record, RecordType};
use hickory_proto::serialize::binary::{BinDecodable, BinDecoder};
use std::collections::HashMap;
use std::net::Ipv4Addr;
use std::sync::Arc;
use virtual_ip_cache::{get_cache, init_cache, Destination};

#[derive(Clone)]
struct DomainMatcher {
    domain_pattern: String,
    metadata: HashMap<String, String>,
}

impl DomainMatcher {
    /// Matches a domain against this matcher's pattern.
    /// Supports exact matches and wildcard patterns like "*.aws.com".
    fn matches(&self, domain: &str) -> bool {
        let Some(base_domain) = self.domain_pattern.strip_prefix("*.") else {
            return domain == self.domain_pattern;
        };

        // For "*.aws.com", base_domain is "aws.com".
        // Domain must end with ".aws.com" and have at least one label before it.
        let Some(prefix) = domain.strip_suffix(base_domain) else {
            return false;
        };
        prefix.ends_with('.') && prefix.len() > 1
    }
}

/// The filter configuration that implements
/// [`envoy_proxy_dynamic_modules_rust_sdk::UdpListenerFilterConfig`].
///
/// This configuration is shared across all UDP listener filter instances.
pub struct DnsGatewayFilterConfig {
    domains: Arc<[DomainMatcher]>,
}

impl DnsGatewayFilterConfig {
    /// Creates a new DNS gateway filter configuration from the raw config bytes.
    ///
    /// The config arrives as a JSON-serialized google.protobuf.Struct
    /// wrapped in an Any: `{"@type":"...Struct", "value":{"base_ip":"...", ...}}`.
    pub fn new(config: &[u8]) -> Option<Self> {
        let config_str = match std::str::from_utf8(config) {
            Ok(s) => s,
            Err(err) => {
                eprintln!("Error parsing config as UTF-8: {err}");
                return None;
            }
        };

        let config_json: serde_json::Value = match serde_json::from_str(config_str) {
            Ok(v) => v,
            Err(err) => {
                eprintln!("Error parsing config JSON: {err}");
                return None;
            }
        };
        let value = &config_json["value"];

        let proto_config: proto::DnsGatewayConfig = match serde_json::from_value(value.clone()) {
            Ok(cfg) => cfg,
            Err(err) => {
                eprintln!("Error parsing DnsGatewayConfig: {err}");
                return None;
            }
        };

        if proto_config.base_ip.is_empty() {
            eprintln!("base_ip is required for DNS gateway");
            return None;
        }
        let base_ip: Ipv4Addr = match proto_config.base_ip.parse() {
            Ok(ip) => ip,
            Err(err) => {
                eprintln!("Invalid base_ip: {err}");
                return None;
            }
        };

        let prefix_len = match u8::try_from(proto_config.prefix_len) {
            Ok(v) => v,
            Err(err) => {
                eprintln!("Invalid prefix_len: {err}");
                return None;
            }
        };
        if !(1..=32).contains(&prefix_len) {
            eprintln!("prefix_len must be between 1 and 32, got {prefix_len}");
            return None;
        }

        init_cache(u32::from(base_ip), prefix_len);

        let domains: Arc<[DomainMatcher]> = proto_config
            .domains
            .into_iter()
            .map(|d| DomainMatcher {
                domain_pattern: d.domain,
                metadata: d.metadata.into_iter().collect(),
            })
            .collect::<Vec<_>>()
            .into();

        envoy_log_info!("Initialized with {} domains", domains.len());

        Some(DnsGatewayFilterConfig { domains })
    }
}

impl<ELF: EnvoyUdpListenerFilter> UdpListenerFilterConfig<ELF> for DnsGatewayFilterConfig {
    fn new_udp_listener_filter(&self, _envoy: &mut ELF) -> Box<dyn UdpListenerFilter<ELF>> {
        Box::new(DnsGatewayFilter {
            domains: Arc::clone(&self.domains),
        })
    }
}

/// The DNS gateway filter that implements
/// [`envoy_proxy_dynamic_modules_rust_sdk::UdpListenerFilter`].
///
/// Intercepts DNS queries and returns virtual IPs for domains matching configured matchers.
struct DnsGatewayFilter {
    domains: Arc<[DomainMatcher]>,
}

impl<ELF: EnvoyUdpListenerFilter> UdpListenerFilter<ELF> for DnsGatewayFilter {
    fn on_data(
        &mut self,
        envoy_filter: &mut ELF,
    ) -> abi::envoy_dynamic_module_type_on_udp_listener_filter_status {
        let (chunks, total_length) = envoy_filter.get_datagram_data();
        envoy_log_debug!(
            "Received UDP datagram, {} bytes, {} chunks",
            total_length,
            chunks.len()
        );
        let data: Vec<u8> = chunks.iter().flat_map(|c| c.as_slice()).copied().collect();

        // From the perspective of DNS gateway, the peer is the client that sent the DNS query.
        let peer = envoy_filter.get_peer_address();
        envoy_log_debug!("Peer address: {:?}", peer);

        let mut decoder = BinDecoder::new(&data);
        let query_message = match Message::read(&mut decoder) {
            Ok(msg) => msg,
            Err(e) => {
                envoy_log_warn!("Failed to parse DNS query: {}", e);
                return abi::envoy_dynamic_module_type_on_udp_listener_filter_status::Continue;
            }
        };

        envoy_log_debug!(
            "Parsed DNS message id={}, type={:?}, queries={}",
            query_message.id(),
            query_message.message_type(),
            query_message.queries().len()
        );

        if query_message.message_type() != MessageType::Query {
            envoy_log_warn!("Received non-query DNS message");
            return abi::envoy_dynamic_module_type_on_udp_listener_filter_status::Continue;
        }

        let question = match query_message.queries().first() {
            Some(q) => q,
            None => {
                envoy_log_warn!("DNS query has no questions");
                return abi::envoy_dynamic_module_type_on_udp_listener_filter_status::Continue;
            }
        };

        let domain_raw = question.name().to_utf8();
        // DNS names are fully qualified with a trailing dot (e.g. "api.aws.com.").
        // Strip it so our wildcard patterns like "*.aws.com" match correctly.
        let domain = domain_raw.strip_suffix('.').unwrap_or(&domain_raw);

        envoy_log_debug!(
            "{:?} record query for domain: {} (raw: {})",
            question.query_type(),
            domain,
            domain_raw
        );

        let matcher = match self.domains.iter().find(|m| m.matches(domain)) {
            Some(m) => m,
            None => {
                envoy_log_info!("No matcher for domain: {}", domain);
                return abi::envoy_dynamic_module_type_on_udp_listener_filter_status::Continue;
            }
        };

        envoy_log_info!(
            "Matched pattern '{}' for domain '{}'",
            matcher.domain_pattern,
            domain
        );

        let response_result = match question.query_type() {
            RecordType::A => {
                let destination = Destination::new(domain.to_string(), matcher.metadata.clone());
                let virtual_ip = match get_cache().allocate(destination) {
                    Some(ip) => ip,
                    None => {
                        envoy_log_error!("IP exhaustion, cannot allocate for {}", domain);
                        return abi::envoy_dynamic_module_type_on_udp_listener_filter_status::Continue;
                    }
                };
                envoy_log_info!("Allocated virtual IP {} for domain {}", virtual_ip, domain);
                build_dns_response(&query_message, question.name(), virtual_ip)
            }
            other => {
                envoy_log_info!(
                    "Returning NODATA for {:?} query (only A records supported)",
                    other
                );
                build_nodata_response(&query_message)
            }
        };

        let response_bytes = match response_result {
            Ok(bytes) => bytes,
            Err(e) => {
                envoy_log_error!("Failed to craft DNS response: {}", e);
                return abi::envoy_dynamic_module_type_on_udp_listener_filter_status::Continue;
            }
        };

        let (peer_addr, peer_port) = match peer {
            Some(p) => p,
            None => {
                envoy_log_error!("No peer address available, cannot send response");
                return abi::envoy_dynamic_module_type_on_udp_listener_filter_status::StopIteration;
            }
        };

        envoy_log_debug!(
            "Sending {} byte response to {}:{}",
            response_bytes.len(),
            peer_addr,
            peer_port
        );
        if !envoy_filter.send_datagram(&response_bytes, &peer_addr, peer_port) {
            envoy_log_error!("Failed to send datagram to {}:{}", peer_addr, peer_port);
        }

        abi::envoy_dynamic_module_type_on_udp_listener_filter_status::StopIteration
    }
}

fn build_dns_response(
    query_message: &Message,
    name: &Name,
    ip: Ipv4Addr,
) -> Result<Vec<u8>, Box<dyn std::error::Error>> {
    let mut response = query_message.clone();

    response.set_message_type(MessageType::Response);
    response.set_response_code(ResponseCode::NoError);
    response.set_recursion_available(true);
    response.set_authoritative(true);

    let record = Record::from_rdata(name.clone(), 600, RData::A(ip.into()));

    response.add_answer(record);

    let bytes = response.to_vec()?;
    Ok(bytes)
}

fn build_nodata_response(query_message: &Message) -> Result<Vec<u8>, Box<dyn std::error::Error>> {
    let mut response = query_message.clone();

    response.set_message_type(MessageType::Response);
    response.set_response_code(ResponseCode::NoError);
    response.set_recursion_available(true);
    response.set_authoritative(true);

    let bytes = response.to_vec()?;
    Ok(bytes)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_domain_matcher_wildcard() {
        let matcher = DomainMatcher {
            domain_pattern: "*.aws.com".to_string(),
            metadata: HashMap::new(),
        };

        assert!(matcher.matches("api.aws.com"));
        assert!(matcher.matches("s3.aws.com"));
        assert!(matcher.matches("lambda.aws.com"));
        assert!(matcher.matches("sub.api.aws.com"));

        assert!(!matcher.matches("aws.com"));
        assert!(!matcher.matches("xaws.com"));
        assert!(!matcher.matches("aws.com.evil.com"));
        assert!(!matcher.matches("api.aws.org"));
    }

    #[test]
    fn test_domain_matcher_exact() {
        let matcher = DomainMatcher {
            domain_pattern: "api.example.com".to_string(),
            metadata: HashMap::new(),
        };

        assert!(matcher.matches("api.example.com"));

        assert!(!matcher.matches("www.api.example.com"));
        assert!(!matcher.matches("example.com"));
        assert!(!matcher.matches("api.example.org"));
    }

    #[test]
    fn test_config_parsing_valid_struct() {
        let config = r#"{
            "@type": "type.googleapis.com/google.protobuf.Struct",
            "value": {
                "base_ip": "10.10.0.0",
                "prefix_len": 24,
                "domains": [
                    {
                        "domain": "*.aws.com",
                        "metadata": {
                            "cluster": "aws_cluster",
                            "region": "us-east-1"
                        }
                    }
                ]
            }
        }"#;

        let config = DnsGatewayFilterConfig::new(config.as_bytes()).unwrap();
        assert_eq!(config.domains.len(), 1);
        assert_eq!(config.domains[0].domain_pattern, "*.aws.com");
        assert_eq!(
            config.domains[0].metadata.get("cluster").unwrap(),
            "aws_cluster"
        );
        assert_eq!(
            config.domains[0].metadata.get("region").unwrap(),
            "us-east-1"
        );
    }

    #[test]
    fn test_config_parsing_multiple_domains() {
        let config = r#"{
            "@type": "type.googleapis.com/google.protobuf.Struct",
            "value": {
                "base_ip": "10.10.0.0",
                "prefix_len": 16,
                "domains": [
                    {"domain": "*.aws.com", "metadata": {"cluster": "aws"}},
                    {"domain": "*.google.com", "metadata": {"cluster": "google"}},
                    {"domain": "exact.example.com", "metadata": {"cluster": "exact"}}
                ]
            }
        }"#;

        let config = DnsGatewayFilterConfig::new(config.as_bytes()).unwrap();
        assert_eq!(config.domains.len(), 3);
    }

    #[test]
    fn test_config_parsing_missing_base_ip() {
        let config = r#"{
            "value": {
                "prefix_len": 24,
                "domains": []
            }
        }"#;

        assert!(DnsGatewayFilterConfig::new(config.as_bytes()).is_none());
    }

    #[test]
    fn test_config_parsing_missing_prefix_len() {
        // proto3 defaults missing uint32 to 0, which fails the 1..=32 range check.
        let config = r#"{
            "value": {
                "base_ip": "10.10.0.0",
                "domains": []
            }
        }"#;

        assert!(DnsGatewayFilterConfig::new(config.as_bytes()).is_none());
    }

    #[test]
    fn test_config_parsing_invalid_prefix_len() {
        let config = r#"{
            "value": {
                "base_ip": "10.10.0.0",
                "prefix_len": 33,
                "domains": []
            }
        }"#;

        assert!(DnsGatewayFilterConfig::new(config.as_bytes()).is_none());
    }

    #[test]
    fn test_config_parsing_invalid_json() {
        assert!(DnsGatewayFilterConfig::new(b"invalid json {").is_none());
    }

    #[test]
    fn test_config_parsing_non_string_metadata_value() {
        let config = r#"{
            "value": {
                "base_ip": "10.10.0.0",
                "prefix_len": 24,
                "domains": [
                    {"domain": "*.aws.com", "metadata": {"count": 42}}
                ]
            }
        }"#;

        assert!(DnsGatewayFilterConfig::new(config.as_bytes()).is_none());
    }

    #[test]
    fn test_domain_stripping_trailing_dot() {
        let domain_raw = "api.aws.com.";
        let domain = domain_raw.strip_suffix('.').unwrap_or(domain_raw);
        assert_eq!(domain, "api.aws.com");
    }

    #[test]
    fn test_domain_without_trailing_dot() {
        let domain_raw = "api.aws.com";
        let domain = domain_raw.strip_suffix('.').unwrap_or(domain_raw);
        assert_eq!(domain, "api.aws.com");
    }

    #[test]
    fn test_dns_response_building() {
        let mut query = Message::new();
        query.set_id(12345);
        query.set_message_type(MessageType::Query);
        query.set_recursion_desired(true);

        let name = Name::from_utf8("test.example.com").unwrap();
        let ip = Ipv4Addr::new(10, 10, 0, 1);

        let result = build_dns_response(&query, &name, ip);
        assert!(result.is_ok());

        let response_bytes = result.unwrap();
        assert!(!response_bytes.is_empty());

        let mut decoder = BinDecoder::new(&response_bytes);
        let response = Message::read(&mut decoder).unwrap();

        assert_eq!(response.id(), 12345);
        assert_eq!(response.message_type(), MessageType::Response);
        assert_eq!(response.response_code(), ResponseCode::NoError);
        assert!(response.recursion_available());
        assert_eq!(response.answers().len(), 1);
    }

    #[test]
    fn test_nodata_response_building() {
        let mut query = Message::new();
        query.set_id(54321);
        query.set_message_type(MessageType::Query);
        query.set_recursion_desired(false);

        let result = build_nodata_response(&query);
        assert!(result.is_ok());

        let response_bytes = result.unwrap();
        assert!(!response_bytes.is_empty());

        let mut decoder = BinDecoder::new(&response_bytes);
        let response = Message::read(&mut decoder).unwrap();

        assert_eq!(response.id(), 54321);
        assert_eq!(response.message_type(), MessageType::Response);
        assert_eq!(response.response_code(), ResponseCode::NoError);
        assert!(response.recursion_available());
        assert_eq!(response.answers().len(), 0);
    }
}
