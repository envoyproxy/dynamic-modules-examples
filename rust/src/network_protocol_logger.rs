//! A protocol detection and logging filter for network connections.
//!
//! This filter demonstrates:
//! 1. Detecting protocols from initial connection bytes.
//! 2. Logging protocol information for debugging.
//! 3. Pattern matching on binary data.
//!
//! Configuration format (JSON):
//! ```json
//! {
//!   "log_request": true,
//!   "log_response": true,
//!   "max_log_bytes": 1024,
//!   "detect_protocol": true
//! }
//! ```
//!
//! To use this filter as a standalone module, create a separate crate with:
//! ```ignore
//! use envoy_proxy_dynamic_modules_rust_sdk::*;
//! declare_network_filter_init_functions!(init, network_protocol_logger::new_filter_config);
//! ```

use envoy_proxy_dynamic_modules_rust_sdk::*;
use serde::{Deserialize, Serialize};

/// Configuration data parsed from the filter config JSON.
#[derive(Serialize, Deserialize, Debug, Clone)]
struct ProtocolLoggerConfigData {
    /// Whether to log request (downstream -> upstream) data.
    #[serde(default = "default_true")]
    log_request: bool,
    /// Whether to log response (upstream -> downstream) data.
    #[serde(default = "default_true")]
    log_response: bool,
    /// Maximum number of bytes to log per direction.
    #[serde(default = "default_max_log_bytes")]
    max_log_bytes: usize,
    /// Whether to attempt protocol detection from first bytes.
    #[serde(default = "default_true")]
    detect_protocol: bool,
}

fn default_true() -> bool {
    true
}

fn default_max_log_bytes() -> usize {
    1024
}

/// Known protocol signatures for detection.
const TLS_SIGNATURE: &[u8] = &[0x16, 0x03]; // TLS record header.
const HTTP_GET: &[u8] = b"GET ";
const HTTP_POST: &[u8] = b"POST ";
const HTTP_PUT: &[u8] = b"PUT ";
const HTTP_DELETE: &[u8] = b"DELETE ";
const HTTP_HEAD: &[u8] = b"HEAD ";
const MYSQL_HANDSHAKE: u8 = 0x0a; // MySQL initial handshake packet.

/// Detected protocol type.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Protocol {
    Unknown,
    Tls,
    Http,
    Redis,
    Mysql,
}

impl Protocol {
    fn as_str(&self) -> &'static str {
        match self {
            Protocol::Unknown => "unknown",
            Protocol::Tls => "tls",
            Protocol::Http => "http",
            Protocol::Redis => "redis",
            Protocol::Mysql => "mysql",
        }
    }
}

/// The filter configuration.
struct ProtocolLoggerFilterConfig {
    log_request: bool,
    log_response: bool,
    max_log_bytes: usize,
    detect_protocol: bool,
    connections_logged: EnvoyCounterId,
    request_bytes_histogram: EnvoyHistogramId,
    response_bytes_histogram: EnvoyHistogramId,
}

/// Creates a new protocol logger filter configuration.
pub fn new_filter_config<EC: EnvoyNetworkFilterConfig, ENF: EnvoyNetworkFilter>(
    envoy_filter_config: &mut EC,
    _name: &str,
    config: &[u8],
) -> Option<Box<dyn NetworkFilterConfig<ENF>>> {
    let config_data: ProtocolLoggerConfigData = if config.is_empty() {
        ProtocolLoggerConfigData {
            log_request: true,
            log_response: true,
            max_log_bytes: 1024,
            detect_protocol: true,
        }
    } else {
        match serde_json::from_slice(config) {
            Ok(cfg) => cfg,
            Err(err) => {
                eprintln!("Error parsing protocol logger config: {err}");
                return None;
            }
        }
    };

    let connections_logged = envoy_filter_config
        .define_counter("protocol_logger_connections_total")
        .expect("Failed to define connections_logged counter");

    let request_bytes_histogram = envoy_filter_config
        .define_histogram("protocol_logger_request_bytes")
        .expect("Failed to define request_bytes histogram");

    let response_bytes_histogram = envoy_filter_config
        .define_histogram("protocol_logger_response_bytes")
        .expect("Failed to define response_bytes histogram");

    Some(Box::new(ProtocolLoggerFilterConfig {
        log_request: config_data.log_request,
        log_response: config_data.log_response,
        max_log_bytes: config_data.max_log_bytes,
        detect_protocol: config_data.detect_protocol,
        connections_logged,
        request_bytes_histogram,
        response_bytes_histogram,
    }))
}

impl<ENF: EnvoyNetworkFilter> NetworkFilterConfig<ENF> for ProtocolLoggerFilterConfig {
    fn new_network_filter(&self, envoy: &mut ENF) -> Box<dyn NetworkFilter<ENF>> {
        let _ = envoy.increment_counter(self.connections_logged, 1);

        Box::new(ProtocolLoggerFilter {
            log_request: self.log_request,
            log_response: self.log_response,
            max_log_bytes: self.max_log_bytes,
            detect_protocol: self.detect_protocol,
            request_bytes_histogram: self.request_bytes_histogram,
            response_bytes_histogram: self.response_bytes_histogram,
            connection_id: envoy.get_connection_id(),
            remote_address: envoy.get_remote_address(),
            local_address: envoy.get_local_address(),
            detected_protocol: Protocol::Unknown,
            total_request_bytes: 0,
            total_response_bytes: 0,
            first_request_logged: false,
            first_response_logged: false,
        })
    }
}

/// The protocol logger filter.
struct ProtocolLoggerFilter {
    log_request: bool,
    log_response: bool,
    max_log_bytes: usize,
    detect_protocol: bool,
    request_bytes_histogram: EnvoyHistogramId,
    response_bytes_histogram: EnvoyHistogramId,
    connection_id: u64,
    remote_address: (String, u32),
    local_address: (String, u32),
    detected_protocol: Protocol,
    total_request_bytes: u64,
    total_response_bytes: u64,
    first_request_logged: bool,
    first_response_logged: bool,
}

/// Standalone protocol detector - can be tested without SDK dependencies.
pub struct ProtocolDetector;

impl ProtocolDetector {
    /// Detect protocol from the first bytes of data.
    pub fn detect(data: &[u8]) -> Protocol {
        if data.len() < 2 {
            return Protocol::Unknown;
        }

        // Check for TLS.
        if data.starts_with(TLS_SIGNATURE) {
            return Protocol::Tls;
        }

        // Check for HTTP methods.
        if data.starts_with(HTTP_GET)
            || data.starts_with(HTTP_POST)
            || data.starts_with(HTTP_PUT)
            || data.starts_with(HTTP_DELETE)
            || data.starts_with(HTTP_HEAD)
        {
            return Protocol::Http;
        }

        // Check for Redis.
        if data[0] == b'*' && data.contains(&b'\r') {
            return Protocol::Redis;
        }

        // Check for MySQL handshake.
        if data.len() >= 5 && data[4] == MYSQL_HANDSHAKE {
            return Protocol::Mysql;
        }

        Protocol::Unknown
    }

    /// Format bytes for logging (hex dump with ASCII).
    pub fn format_bytes_for_logging(data: &[u8], max_bytes: usize) -> String {
        let truncated = if data.len() > max_bytes {
            &data[..max_bytes]
        } else {
            data
        };

        let hex: String = truncated
            .iter()
            .map(|b| format!("{:02x}", b))
            .collect::<Vec<_>>()
            .join(" ");

        let ascii: String = truncated
            .iter()
            .map(|&b| {
                if b.is_ascii_graphic() || b == b' ' {
                    b as char
                } else {
                    '.'
                }
            })
            .collect();

        if data.len() > max_bytes {
            format!(
                "[{} bytes, showing first {}] HEX: {} ASCII: {}",
                data.len(),
                max_bytes,
                hex,
                ascii
            )
        } else {
            format!("[{} bytes] HEX: {} ASCII: {}", data.len(), hex, ascii)
        }
    }
}

impl ProtocolLoggerFilter {
    fn detect_protocol_from_bytes(&self, data: &[u8]) -> Protocol {
        ProtocolDetector::detect(data)
    }

    fn format_bytes_for_logging(&self, data: &[u8]) -> String {
        ProtocolDetector::format_bytes_for_logging(data, self.max_log_bytes)
    }
}

impl<ENF: EnvoyNetworkFilter> NetworkFilter<ENF> for ProtocolLoggerFilter {
    fn on_new_connection(
        &mut self,
        envoy_filter: &mut ENF,
    ) -> abi::envoy_dynamic_module_type_on_network_filter_data_status {
        envoy_log_info!(
            "[conn={}] New connection: remote={}:{} local={}:{} ssl={}",
            self.connection_id,
            self.remote_address.0,
            self.remote_address.1,
            self.local_address.0,
            self.local_address.1,
            envoy_filter.is_ssl()
        );

        abi::envoy_dynamic_module_type_on_network_filter_data_status::Continue
    }

    fn on_read(
        &mut self,
        envoy_filter: &mut ENF,
        data_length: usize,
        end_stream: bool,
    ) -> abi::envoy_dynamic_module_type_on_network_filter_data_status {
        self.total_request_bytes += data_length as u64;

        if self.log_request && !self.first_request_logged && data_length > 0 {
            let (chunks, _) = envoy_filter.get_read_buffer_chunks();

            // Collect data for logging.
            let mut data = Vec::new();
            for chunk in &chunks {
                data.extend_from_slice(chunk.as_slice());
                if data.len() >= self.max_log_bytes {
                    break;
                }
            }

            // Detect protocol from first request data.
            if self.detect_protocol && self.detected_protocol == Protocol::Unknown {
                self.detected_protocol = self.detect_protocol_from_bytes(&data);

                // Store detected protocol in filter state.
                let protocol_bytes = self.detected_protocol.as_str().as_bytes();
                envoy_filter.set_filter_state_bytes(b"detected_protocol", protocol_bytes);

                envoy_log_info!(
                    "[conn={}] Detected protocol: {}",
                    self.connection_id,
                    self.detected_protocol.as_str()
                );
            }

            let formatted = self.format_bytes_for_logging(&data);
            envoy_log_info!(
                "[conn={}] REQUEST: {} end_stream={}",
                self.connection_id,
                formatted,
                end_stream
            );

            self.first_request_logged = true;
        }

        abi::envoy_dynamic_module_type_on_network_filter_data_status::Continue
    }

    fn on_write(
        &mut self,
        envoy_filter: &mut ENF,
        data_length: usize,
        end_stream: bool,
    ) -> abi::envoy_dynamic_module_type_on_network_filter_data_status {
        self.total_response_bytes += data_length as u64;

        if self.log_response && !self.first_response_logged && data_length > 0 {
            let (chunks, _) = envoy_filter.get_write_buffer_chunks();

            // Collect data for logging.
            let mut data = Vec::new();
            for chunk in &chunks {
                data.extend_from_slice(chunk.as_slice());
                if data.len() >= self.max_log_bytes {
                    break;
                }
            }

            let formatted = self.format_bytes_for_logging(&data);
            envoy_log_info!(
                "[conn={}] RESPONSE: {} end_stream={}",
                self.connection_id,
                formatted,
                end_stream
            );

            self.first_response_logged = true;
        }

        abi::envoy_dynamic_module_type_on_network_filter_data_status::Continue
    }

    fn on_event(
        &mut self,
        envoy_filter: &mut ENF,
        event: abi::envoy_dynamic_module_type_network_connection_event,
    ) {
        match event {
            abi::envoy_dynamic_module_type_network_connection_event::RemoteClose => {
                envoy_log_info!(
                    "[conn={}] Connection closed by remote. protocol={} request_bytes={} response_bytes={}",
                    self.connection_id,
                    self.detected_protocol.as_str(),
                    self.total_request_bytes,
                    self.total_response_bytes
                );
            }
            abi::envoy_dynamic_module_type_network_connection_event::LocalClose => {
                envoy_log_info!(
                    "[conn={}] Connection closed locally. protocol={} request_bytes={} response_bytes={}",
                    self.connection_id,
                    self.detected_protocol.as_str(),
                    self.total_request_bytes,
                    self.total_response_bytes
                );
            }
            abi::envoy_dynamic_module_type_network_connection_event::Connected => {
                envoy_log_debug!(
                    "[conn={}] Upstream connection established",
                    self.connection_id
                );
            }
            _ => {}
        }

        // Record histograms on connection close.
        if matches!(
            event,
            abi::envoy_dynamic_module_type_network_connection_event::RemoteClose
                | abi::envoy_dynamic_module_type_network_connection_event::LocalClose
        ) {
            let _ = envoy_filter
                .record_histogram_value(self.request_bytes_histogram, self.total_request_bytes);
            let _ = envoy_filter
                .record_histogram_value(self.response_bytes_histogram, self.total_response_bytes);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_protocol_logger_config_parsing() {
        let config = r#"{"log_request": true, "log_response": false, "max_log_bytes": 512}"#;
        let config_data: ProtocolLoggerConfigData = serde_json::from_str(config).unwrap();
        assert!(config_data.log_request);
        assert!(!config_data.log_response);
        assert_eq!(config_data.max_log_bytes, 512);
    }

    #[test]
    fn test_protocol_logger_default_config() {
        let config = r#"{}"#;
        let config_data: ProtocolLoggerConfigData = serde_json::from_str(config).unwrap();
        assert!(config_data.log_request);
        assert!(config_data.log_response);
        assert_eq!(config_data.max_log_bytes, 1024);
        assert!(config_data.detect_protocol);
    }

    #[test]
    fn test_protocol_detection_tls() {
        let tls_data: &[u8] = &[0x16, 0x03, 0x01, 0x00, 0x05];
        assert_eq!(ProtocolDetector::detect(tls_data), Protocol::Tls);
    }

    #[test]
    fn test_protocol_detection_http() {
        let http_data = b"GET /path HTTP/1.1\r\n";
        assert_eq!(ProtocolDetector::detect(http_data), Protocol::Http);
    }

    #[test]
    fn test_protocol_detection_redis() {
        let redis_data = b"*1\r\n$4\r\nPING\r\n";
        assert_eq!(ProtocolDetector::detect(redis_data), Protocol::Redis);
    }

    #[test]
    fn test_protocol_detection_unknown() {
        let data = b"random data";
        assert_eq!(ProtocolDetector::detect(data), Protocol::Unknown);
    }

    #[test]
    fn test_format_bytes_for_logging() {
        let data = b"hello";
        let formatted = ProtocolDetector::format_bytes_for_logging(data, 1024);
        assert!(formatted.contains("5 bytes"));
        assert!(formatted.contains("68 65 6c 6c 6f")); // hex for "hello"
    }

    #[test]
    fn test_protocol_as_str() {
        assert_eq!(Protocol::Unknown.as_str(), "unknown");
        assert_eq!(Protocol::Tls.as_str(), "tls");
        assert_eq!(Protocol::Http.as_str(), "http");
        assert_eq!(Protocol::Redis.as_str(), "redis");
        assert_eq!(Protocol::Mysql.as_str(), "mysql");
    }
}
