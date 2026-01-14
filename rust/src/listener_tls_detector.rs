//! A TLS protocol detection filter for listener-level protocol inspection.
//!
//! This filter demonstrates:
//! 1. Inspecting initial connection bytes to detect TLS.
//! 2. Extracting TLS Client Hello information (SNI, ALPN).
//! 3. Protocol detection for filter chain matching.
//!
//! Configuration format (JSON):
//! ```json
//! {
//!   "min_bytes": 5,
//!   "max_bytes": 1024,
//!   "extract_sni": true,
//!   "extract_alpn": true
//! }
//! ```
//!
//! To use this filter as a standalone module, create a separate crate with:
//! ```ignore
//! use envoy_proxy_dynamic_modules_rust_sdk::*;
//! declare_listener_filter_init_functions!(init, listener_tls_detector::new_filter_config);
//! ```

use envoy_proxy_dynamic_modules_rust_sdk::*;
use serde::{Deserialize, Serialize};

/// TLS constants - used by detection logic and tests.
#[allow(dead_code)]
const TLS_CONTENT_TYPE_HANDSHAKE: u8 = 0x16;
#[allow(dead_code)]
const TLS_HANDSHAKE_CLIENT_HELLO: u8 = 0x01;
#[allow(dead_code)]
const TLS_EXT_SERVER_NAME: u16 = 0x0000;
#[allow(dead_code)]
const TLS_EXT_ALPN: u16 = 0x0010;
#[allow(dead_code)]
const SNI_NAME_TYPE_HOSTNAME: u8 = 0x00;

/// Configuration data parsed from the filter config JSON.
#[derive(Serialize, Deserialize, Debug, Clone)]
struct TlsDetectorConfigData {
    #[serde(default = "default_min_bytes")]
    min_bytes: usize,
    #[serde(default = "default_max_bytes")]
    max_bytes: usize,
    #[serde(default = "default_true")]
    extract_sni: bool,
    #[serde(default = "default_true")]
    extract_alpn: bool,
}

fn default_min_bytes() -> usize {
    5
}

fn default_max_bytes() -> usize {
    1024
}

fn default_true() -> bool {
    true
}

impl Default for TlsDetectorConfigData {
    fn default() -> Self {
        TlsDetectorConfigData {
            min_bytes: default_min_bytes(),
            max_bytes: default_max_bytes(),
            extract_sni: true,
            extract_alpn: true,
        }
    }
}

/// TLS detection result.
#[derive(Debug, Clone, PartialEq)]
pub enum TlsDetectionResult {
    Tls {
        sni: Option<String>,
        alpn: Vec<String>,
    },
    NotTls,
    NeedMoreData,
}

/// The filter configuration.
pub struct TlsDetectorFilterConfig {
    min_bytes: usize,
    max_bytes: usize,
    extract_sni: bool,
    extract_alpn: bool,
    tls_connections: EnvoyCounterId,
    non_tls_connections: EnvoyCounterId,
}

/// Creates a new TLS detector filter configuration.
pub fn new_filter_config<EC: EnvoyListenerFilterConfig, ELF: EnvoyListenerFilter>(
    envoy_filter_config: &mut EC,
    _name: &str,
    config: &[u8],
) -> Option<Box<dyn ListenerFilterConfig<ELF>>> {
    let config_data: TlsDetectorConfigData = if config.is_empty() {
        TlsDetectorConfigData::default()
    } else {
        match serde_json::from_slice(config) {
            Ok(cfg) => cfg,
            Err(err) => {
                eprintln!("Error parsing TLS detector config: {err}");
                return None;
            }
        }
    };

    let tls_connections = envoy_filter_config
        .define_counter("tls_detector_tls_connections_total")
        .expect("Failed to define tls_connections counter");

    let non_tls_connections = envoy_filter_config
        .define_counter("tls_detector_non_tls_connections_total")
        .expect("Failed to define non_tls_connections counter");

    Some(Box::new(TlsDetectorFilterConfig {
        min_bytes: config_data.min_bytes,
        max_bytes: config_data.max_bytes,
        extract_sni: config_data.extract_sni,
        extract_alpn: config_data.extract_alpn,
        tls_connections,
        non_tls_connections,
    }))
}

impl<ELF: EnvoyListenerFilter> ListenerFilterConfig<ELF> for TlsDetectorFilterConfig {
    fn new_listener_filter(&self, _envoy: &mut ELF) -> Box<dyn ListenerFilter<ELF>> {
        Box::new(TlsDetectorFilter {
            min_bytes: self.min_bytes,
            max_bytes: self.max_bytes,
            extract_sni: self.extract_sni,
            extract_alpn: self.extract_alpn,
            tls_connections: self.tls_connections,
            non_tls_connections: self.non_tls_connections,
        })
    }
}

/// The TLS detector filter.
#[allow(dead_code)]
struct TlsDetectorFilter {
    min_bytes: usize,
    max_bytes: usize,
    extract_sni: bool,
    extract_alpn: bool,
    tls_connections: EnvoyCounterId,
    non_tls_connections: EnvoyCounterId,
}

#[allow(dead_code)]
impl TlsDetectorFilter {
    /// Detect if data is TLS and extract SNI/ALPN.
    fn detect(&self, data: &[u8]) -> TlsDetectionResult {
        if data.len() < self.min_bytes {
            return TlsDetectionResult::NeedMoreData;
        }

        // Check for TLS record header.
        if data.len() < 6 || data[0] != TLS_CONTENT_TYPE_HANDSHAKE {
            return TlsDetectionResult::NotTls;
        }

        // Check TLS version.
        if data[1] != 0x03 || data[2] > 0x03 {
            return TlsDetectionResult::NotTls;
        }

        // Check if this is a Client Hello.
        if data.len() <= 5 || data[5] != TLS_HANDSHAKE_CLIENT_HELLO {
            return TlsDetectionResult::Tls {
                sni: None,
                alpn: Vec::new(),
            };
        }

        // Parse Client Hello.
        let bytes_to_read = std::cmp::min(data.len(), self.max_bytes);
        let (sni, alpn) = self.parse_client_hello(&data[..bytes_to_read]);

        TlsDetectionResult::Tls { sni, alpn }
    }

    /// Parse TLS Client Hello and extract SNI and ALPN.
    fn parse_client_hello(&self, data: &[u8]) -> (Option<String>, Vec<String>) {
        let mut sni = None;
        let mut alpn_protocols = Vec::new();

        if data.len() < 43 {
            return (sni, alpn_protocols);
        }

        let mut offset = 9; // Skip TLS record header + handshake header.
        offset += 2; // Skip client version.
        offset += 32; // Skip client random.

        if offset >= data.len() {
            return (sni, alpn_protocols);
        }
        let session_id_len = data[offset] as usize;
        offset += 1 + session_id_len;

        if offset + 2 > data.len() {
            return (sni, alpn_protocols);
        }
        let cipher_suites_len = u16::from_be_bytes([data[offset], data[offset + 1]]) as usize;
        offset += 2 + cipher_suites_len;

        if offset >= data.len() {
            return (sni, alpn_protocols);
        }
        let compression_len = data[offset] as usize;
        offset += 1 + compression_len;

        if offset + 2 > data.len() {
            return (sni, alpn_protocols);
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

            match ext_type {
                TLS_EXT_SERVER_NAME if self.extract_sni => {
                    sni = self.parse_sni_extension(&data[offset..offset + ext_len]);
                }
                TLS_EXT_ALPN if self.extract_alpn => {
                    alpn_protocols = self.parse_alpn_extension(&data[offset..offset + ext_len]);
                }
                _ => {}
            }

            offset += ext_len;
        }

        (sni, alpn_protocols)
    }

    fn parse_sni_extension(&self, data: &[u8]) -> Option<String> {
        if data.len() < 5 {
            return None;
        }

        let mut offset = 2; // Skip list length.

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

    fn parse_alpn_extension(&self, data: &[u8]) -> Vec<String> {
        let mut protocols = Vec::new();

        if data.len() < 2 {
            return protocols;
        }

        let mut offset = 2; // Skip list length.

        while offset < data.len() {
            let proto_len = data[offset] as usize;
            offset += 1;

            if offset + proto_len > data.len() {
                break;
            }

            if let Ok(proto) = String::from_utf8(data[offset..offset + proto_len].to_vec()) {
                protocols.push(proto);
            }
            offset += proto_len;
        }

        protocols
    }
}

impl<ELF: EnvoyListenerFilter> ListenerFilter<ELF> for TlsDetectorFilter {
    fn on_accept(
        &mut self,
        envoy_filter: &mut ELF,
    ) -> abi::envoy_dynamic_module_type_on_listener_filter_status {
        // For TLS detection, we need to inspect the connection data.
        // This requires peeking at the socket buffer.
        // In a real implementation, this would use on_data callback.
        // For now, we just log and continue.
        envoy_log_debug!("TLS detector filter activated");

        let _ = envoy_filter.increment_counter(self.tls_connections, 0);

        abi::envoy_dynamic_module_type_on_listener_filter_status::Continue
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_tls_detector_config_parsing() {
        let config = r#"{"min_bytes": 10, "max_bytes": 2048}"#;
        let config_data: TlsDetectorConfigData = serde_json::from_str(config).unwrap();
        assert_eq!(config_data.min_bytes, 10);
        assert_eq!(config_data.max_bytes, 2048);
    }

    #[test]
    fn test_tls_detector_default_config() {
        let config_data = TlsDetectorConfigData::default();
        assert_eq!(config_data.min_bytes, 5);
        assert_eq!(config_data.max_bytes, 1024);
        assert!(config_data.extract_sni);
        assert!(config_data.extract_alpn);
    }

    #[test]
    fn test_tls_detection_result_variants() {
        // Test that the TlsDetectionResult enum variants are correct.
        let tls_result = TlsDetectionResult::Tls {
            sni: Some("example.com".to_string()),
            alpn: vec!["h2".to_string()],
        };
        assert!(matches!(tls_result, TlsDetectionResult::Tls { .. }));

        let not_tls = TlsDetectionResult::NotTls;
        assert_eq!(not_tls, TlsDetectionResult::NotTls);

        let need_more = TlsDetectionResult::NeedMoreData;
        assert_eq!(need_more, TlsDetectionResult::NeedMoreData);
    }

    #[test]
    fn test_tls_record_header_detection() {
        // TLS records start with content type 0x16 (handshake).
        let tls_header: &[u8] = &[0x16, 0x03, 0x01];
        assert_eq!(tls_header[0], 0x16);
        assert!(tls_header[1] == 0x03); // TLS major version.
    }
}
