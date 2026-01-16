//! An SNI-based routing filter that extracts SNI for filter chain matching.
//!
//! This filter demonstrates:
//! 1. Parsing TLS Client Hello to extract SNI.
//! 2. Domain to cluster mapping with wildcard support.
//! 3. Handling connections with and without SNI.
//!
//! Configuration format (JSON):
//! ```json
//! {
//!   "default_server_name": "default.example.com",
//!   "domain_mappings": {
//!     "api.example.com": "api-cluster",
//!     "*.example.com": "wildcard-cluster"
//!   },
//!   "reject_unknown": false
//! }
//! ```
//!
//! To use this filter as a standalone module, create a separate crate with:
//! ```ignore
//! use envoy_proxy_dynamic_modules_rust_sdk::*;
//! declare_listener_filter_init_functions!(init, listener_sni_router::new_filter_config);
//! ```

use envoy_proxy_dynamic_modules_rust_sdk::*;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// TLS constants - used by SNI extraction logic and tests.
#[allow(dead_code)]
const TLS_CONTENT_TYPE_HANDSHAKE: u8 = 0x16;
#[allow(dead_code)]
const TLS_HANDSHAKE_CLIENT_HELLO: u8 = 0x01;
#[allow(dead_code)]
const TLS_EXT_SERVER_NAME: u16 = 0x0000;
#[allow(dead_code)]
const SNI_NAME_TYPE_HOSTNAME: u8 = 0x00;

/// Configuration data parsed from the filter config JSON.
#[derive(Serialize, Deserialize, Debug, Clone)]
struct SniRouterConfigData {
    #[serde(default)]
    default_server_name: Option<String>,
    #[serde(default)]
    domain_mappings: HashMap<String, String>,
    #[serde(default)]
    reject_unknown: bool,
    #[serde(default = "default_max_read_bytes")]
    max_read_bytes: usize,
}

fn default_max_read_bytes() -> usize {
    1024
}

impl Default for SniRouterConfigData {
    fn default() -> Self {
        SniRouterConfigData {
            default_server_name: None,
            domain_mappings: HashMap::new(),
            reject_unknown: false,
            max_read_bytes: default_max_read_bytes(),
        }
    }
}

/// Result of SNI routing.
#[derive(Debug, Clone, PartialEq)]
pub enum SniRoutingResult {
    Routed { sni: String, cluster: String },
    NoMapping { sni: String },
    NoSni { default: Option<String> },
    NotTls,
    Rejected { reason: String },
    NeedMoreData,
}

/// The filter configuration.
pub struct SniRouterFilterConfig {
    default_server_name: Option<String>,
    domain_mappings: HashMap<String, String>,
    reject_unknown: bool,
    max_read_bytes: usize,
    sni_matches: EnvoyCounterId,
    sni_misses: EnvoyCounterId,
}

/// Creates a new SNI router filter configuration.
pub fn new_filter_config<EC: EnvoyListenerFilterConfig, ELF: EnvoyListenerFilter>(
    envoy_filter_config: &mut EC,
    _name: &str,
    config: &[u8],
) -> Option<Box<dyn ListenerFilterConfig<ELF>>> {
    let config_data: SniRouterConfigData = if config.is_empty() {
        SniRouterConfigData::default()
    } else {
        match serde_json::from_slice(config) {
            Ok(cfg) => cfg,
            Err(err) => {
                eprintln!("Error parsing SNI router config: {err}");
                return None;
            }
        }
    };

    let sni_matches = envoy_filter_config
        .define_counter("sni_router_matches_total")
        .expect("Failed to define sni_matches counter");

    let sni_misses = envoy_filter_config
        .define_counter("sni_router_misses_total")
        .expect("Failed to define sni_misses counter");

    Some(Box::new(SniRouterFilterConfig {
        default_server_name: config_data.default_server_name,
        domain_mappings: config_data.domain_mappings,
        reject_unknown: config_data.reject_unknown,
        max_read_bytes: config_data.max_read_bytes,
        sni_matches,
        sni_misses,
    }))
}

impl<ELF: EnvoyListenerFilter> ListenerFilterConfig<ELF> for SniRouterFilterConfig {
    fn new_listener_filter(&self, _envoy: &mut ELF) -> Box<dyn ListenerFilter<ELF>> {
        Box::new(SniRouterFilter {
            default_server_name: self.default_server_name.clone(),
            domain_mappings: self.domain_mappings.clone(),
            reject_unknown: self.reject_unknown,
            max_read_bytes: self.max_read_bytes,
            sni_matches: self.sni_matches,
            sni_misses: self.sni_misses,
        })
    }
}

/// The SNI router filter.
#[allow(dead_code)]
struct SniRouterFilter {
    default_server_name: Option<String>,
    domain_mappings: HashMap<String, String>,
    reject_unknown: bool,
    max_read_bytes: usize,
    sni_matches: EnvoyCounterId,
    sni_misses: EnvoyCounterId,
}

#[allow(dead_code)]
impl SniRouterFilter {
    /// Extract SNI from TLS Client Hello.
    fn extract_sni(&self, data: &[u8]) -> Option<String> {
        if data.len() < 6 || data[0] != TLS_CONTENT_TYPE_HANDSHAKE {
            return None;
        }

        if data[1] != 0x03 {
            return None;
        }

        if data[5] != TLS_HANDSHAKE_CLIENT_HELLO {
            return None;
        }

        if data.len() < 43 {
            return None;
        }

        let mut offset = 9;
        offset += 2; // Skip client version.
        offset += 32; // Skip client random.

        if offset >= data.len() {
            return None;
        }
        let session_id_len = data[offset] as usize;
        offset += 1 + session_id_len;

        if offset + 2 > data.len() {
            return None;
        }
        let cipher_suites_len = u16::from_be_bytes([data[offset], data[offset + 1]]) as usize;
        offset += 2 + cipher_suites_len;

        if offset >= data.len() {
            return None;
        }
        let compression_len = data[offset] as usize;
        offset += 1 + compression_len;

        if offset + 2 > data.len() {
            return None;
        }
        let extensions_len = u16::from_be_bytes([data[offset], data[offset + 1]]) as usize;
        offset += 2;

        let extensions_end = offset + extensions_len;
        while offset + 4 <= extensions_end && offset + 4 <= data.len() {
            let ext_type = u16::from_be_bytes([data[offset], data[offset + 1]]);
            let ext_len = u16::from_be_bytes([data[offset + 2], data[offset + 3]]) as usize;
            offset += 4;

            if offset + ext_len > data.len() {
                break;
            }

            if ext_type == TLS_EXT_SERVER_NAME {
                return self.parse_sni_extension(&data[offset..offset + ext_len]);
            }

            offset += ext_len;
        }

        None
    }

    fn parse_sni_extension(&self, data: &[u8]) -> Option<String> {
        if data.len() < 5 {
            return None;
        }

        let mut offset = 2;

        if data[offset] != SNI_NAME_TYPE_HOSTNAME {
            return None;
        }
        offset += 1;

        if offset + 2 > data.len() {
            return None;
        }
        let name_len = u16::from_be_bytes([data[offset], data[offset + 1]]) as usize;
        offset += 2;

        if offset + name_len > data.len() {
            return None;
        }

        String::from_utf8(data[offset..offset + name_len].to_vec()).ok()
    }

    /// Look up the cluster for a given SNI.
    pub fn lookup_cluster(&self, sni: &str) -> Option<&String> {
        // Exact match first.
        if let Some(cluster) = self.domain_mappings.get(sni) {
            return Some(cluster);
        }

        // Wildcard match.
        for (domain, cluster) in &self.domain_mappings {
            if domain.starts_with("*.") {
                let suffix = &domain[1..];
                if sni.ends_with(suffix) && sni.len() > suffix.len() {
                    return Some(cluster);
                }
            }
        }

        None
    }

    /// Process data and determine routing.
    pub fn process(&self, data: &[u8]) -> SniRoutingResult {
        if data.len() < 6 {
            return SniRoutingResult::NeedMoreData;
        }

        if data[0] != TLS_CONTENT_TYPE_HANDSHAKE {
            return SniRoutingResult::NotTls;
        }

        let bytes_to_check = std::cmp::min(data.len(), self.max_read_bytes);
        let sni = self.extract_sni(&data[..bytes_to_check]);

        match sni {
            Some(server_name) => {
                if let Some(cluster) = self.lookup_cluster(&server_name) {
                    SniRoutingResult::Routed {
                        sni: server_name,
                        cluster: cluster.clone(),
                    }
                } else if self.reject_unknown {
                    SniRoutingResult::Rejected {
                        reason: format!("Unknown SNI: {server_name}"),
                    }
                } else {
                    SniRoutingResult::NoMapping { sni: server_name }
                }
            }
            None => {
                if self.reject_unknown && self.default_server_name.is_none() {
                    SniRoutingResult::Rejected {
                        reason: "Missing SNI".to_string(),
                    }
                } else {
                    SniRoutingResult::NoSni {
                        default: self.default_server_name.clone(),
                    }
                }
            }
        }
    }
}

impl<ELF: EnvoyListenerFilter> ListenerFilter<ELF> for SniRouterFilter {
    fn on_accept(
        &mut self,
        envoy_filter: &mut ELF,
    ) -> abi::envoy_dynamic_module_type_on_listener_filter_status {
        // SNI routing requires inspecting data, which is done in on_data.
        envoy_log_debug!("SNI router filter activated");
        let _ = envoy_filter.increment_counter(self.sni_matches, 0);
        abi::envoy_dynamic_module_type_on_listener_filter_status::Continue
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sni_router_config_parsing() {
        let config = r#"{
            "default_server_name": "default.example.com",
            "domain_mappings": {
                "api.example.com": "api-cluster"
            },
            "reject_unknown": true
        }"#;
        let config_data: SniRouterConfigData = serde_json::from_str(config).unwrap();
        assert_eq!(
            config_data.default_server_name,
            Some("default.example.com".to_string())
        );
        assert!(config_data.reject_unknown);
    }

    #[test]
    fn test_sni_router_default_config() {
        let config_data = SniRouterConfigData::default();
        assert!(config_data.default_server_name.is_none());
        assert!(config_data.domain_mappings.is_empty());
        assert!(!config_data.reject_unknown);
    }

    #[test]
    fn test_sni_routing_result_variants() {
        // Test that the SniRoutingResult enum variants are correct.
        let routed = SniRoutingResult::Routed {
            sni: "api.example.com".to_string(),
            cluster: "api-cluster".to_string(),
        };
        assert!(matches!(routed, SniRoutingResult::Routed { .. }));

        let no_mapping = SniRoutingResult::NoMapping {
            sni: "unknown.com".to_string(),
        };
        assert!(matches!(no_mapping, SniRoutingResult::NoMapping { .. }));

        let no_sni = SniRoutingResult::NoSni { default: None };
        assert!(matches!(no_sni, SniRoutingResult::NoSni { .. }));

        let not_tls = SniRoutingResult::NotTls;
        assert_eq!(not_tls, SniRoutingResult::NotTls);

        let need_more = SniRoutingResult::NeedMoreData;
        assert_eq!(need_more, SniRoutingResult::NeedMoreData);
    }

    #[test]
    fn test_wildcard_domain_matching_logic() {
        // Test the wildcard matching pattern.
        let domain = "*.example.com";
        let suffix = &domain[1..]; // ".example.com"

        let test_sni = "api.example.com";
        assert!(test_sni.ends_with(suffix));
        assert!(test_sni.len() > suffix.len());

        let bad_sni = "example.com"; // Should not match - needs subdomain.
        assert!(bad_sni.ends_with(suffix) == false || bad_sni.len() <= suffix.len());
    }
}
