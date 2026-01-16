//! A simple connection rate limiter for network filters.
//!
//! This filter demonstrates:
//! 1. Connection counting and rate limiting.
//! 2. Shared state across filter instances using atomic counters.
//! 3. Rejecting connections when limits are exceeded.
//!
//! Configuration format (JSON):
//! ```json
//! {
//!   "max_connections": 100,
//!   "reject_message": "Too many connections"
//! }
//! ```
//!
//! To use this filter as a standalone module, create a separate crate with:
//! ```ignore
//! use envoy_proxy_dynamic_modules_rust_sdk::*;
//! declare_network_filter_init_functions!(init, network_rate_limiter::new_filter_config);
//! ```

use envoy_proxy_dynamic_modules_rust_sdk::*;
use serde::{Deserialize, Serialize};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;

/// Configuration data parsed from the filter config JSON.
#[derive(Serialize, Deserialize, Debug, Clone)]
struct RateLimiterConfigData {
    /// Maximum number of concurrent connections allowed.
    max_connections: u64,
    /// Message to send when rejecting connections.
    #[serde(default = "default_reject_message")]
    reject_message: String,
}

fn default_reject_message() -> String {
    "Connection limit exceeded".to_string()
}

/// Shared state for tracking active connections across all filter instances.
struct SharedConnectionState {
    /// Current number of active connections.
    active_connections: AtomicU64,
}

/// The filter configuration that implements
/// [`envoy_proxy_dynamic_modules_rust_sdk::NetworkFilterConfig`].
struct RateLimiterFilterConfig {
    /// Maximum number of concurrent connections allowed.
    max_connections: u64,
    /// Message to send when rejecting connections.
    reject_message: Vec<u8>,
    /// Shared state for tracking connections.
    shared_state: Arc<SharedConnectionState>,
    /// Counter for total connections accepted.
    connections_accepted: EnvoyCounterId,
    /// Counter for total connections rejected.
    connections_rejected: EnvoyCounterId,
    /// Gauge for current active connections.
    active_connections_gauge: EnvoyGaugeId,
}

/// Creates a new rate limiter filter configuration.
pub fn new_filter_config<EC: EnvoyNetworkFilterConfig, ENF: EnvoyNetworkFilter>(
    envoy_filter_config: &mut EC,
    _name: &str,
    config: &[u8],
) -> Option<Box<dyn NetworkFilterConfig<ENF>>> {
    let config_data: RateLimiterConfigData = match serde_json::from_slice(config) {
        Ok(cfg) => cfg,
        Err(err) => {
            eprintln!("Error parsing rate limiter config: {err}");
            return None;
        }
    };

    if config_data.max_connections == 0 {
        eprintln!("max_connections must be greater than 0");
        return None;
    }

    let connections_accepted = envoy_filter_config
        .define_counter("rate_limiter_connections_accepted_total")
        .expect("Failed to define connections_accepted counter");

    let connections_rejected = envoy_filter_config
        .define_counter("rate_limiter_connections_rejected_total")
        .expect("Failed to define connections_rejected counter");

    let active_connections_gauge = envoy_filter_config
        .define_gauge("rate_limiter_active_connections")
        .expect("Failed to define active_connections gauge");

    Some(Box::new(RateLimiterFilterConfig {
        max_connections: config_data.max_connections,
        reject_message: config_data.reject_message.into_bytes(),
        shared_state: Arc::new(SharedConnectionState {
            active_connections: AtomicU64::new(0),
        }),
        connections_accepted,
        connections_rejected,
        active_connections_gauge,
    }))
}

impl<ENF: EnvoyNetworkFilter> NetworkFilterConfig<ENF> for RateLimiterFilterConfig {
    fn new_network_filter(&self, _envoy: &mut ENF) -> Box<dyn NetworkFilter<ENF>> {
        Box::new(RateLimiterFilter {
            max_connections: self.max_connections,
            reject_message: self.reject_message.clone(),
            shared_state: Arc::clone(&self.shared_state),
            connections_accepted: self.connections_accepted,
            connections_rejected: self.connections_rejected,
            active_connections_gauge: self.active_connections_gauge,
            connection_counted: false,
        })
    }
}

/// The rate limiter filter that implements [`envoy_proxy_dynamic_modules_rust_sdk::NetworkFilter`].
struct RateLimiterFilter {
    /// Maximum number of concurrent connections allowed.
    max_connections: u64,
    /// Message to send when rejecting connections.
    reject_message: Vec<u8>,
    /// Shared state for tracking connections.
    shared_state: Arc<SharedConnectionState>,
    /// Counter ID for connections accepted.
    connections_accepted: EnvoyCounterId,
    /// Counter ID for connections rejected.
    connections_rejected: EnvoyCounterId,
    /// Gauge ID for active connections.
    active_connections_gauge: EnvoyGaugeId,
    /// Whether this connection was counted in the active connections.
    connection_counted: bool,
}

impl<ENF: EnvoyNetworkFilter> NetworkFilter<ENF> for RateLimiterFilter {
    fn on_new_connection(
        &mut self,
        envoy_filter: &mut ENF,
    ) -> abi::envoy_dynamic_module_type_on_network_filter_data_status {
        // Try to increment the connection count.
        let current = self
            .shared_state
            .active_connections
            .fetch_add(1, Ordering::SeqCst);

        if current >= self.max_connections {
            // Over limit, decrement and reject.
            self.shared_state
                .active_connections
                .fetch_sub(1, Ordering::SeqCst);

            let _ = envoy_filter.increment_counter(self.connections_rejected, 1);

            let (addr, port) = envoy_filter.get_remote_address();
            envoy_log_warn!(
                "Connection from {}:{} rejected. Current connections: {}, limit: {}",
                addr,
                port,
                current,
                self.max_connections
            );

            // Send rejection message and close the connection.
            envoy_filter.write(&self.reject_message, true);
            envoy_filter
                .close(abi::envoy_dynamic_module_type_network_connection_close_type::FlushWrite);

            return abi::envoy_dynamic_module_type_on_network_filter_data_status::StopIteration;
        }

        // Connection accepted.
        self.connection_counted = true;
        let _ = envoy_filter.increment_counter(self.connections_accepted, 1);
        let _ = envoy_filter.set_gauge(
            self.active_connections_gauge,
            self.shared_state.active_connections.load(Ordering::SeqCst),
        );

        let (addr, port) = envoy_filter.get_remote_address();
        envoy_log_info!(
            "Connection from {}:{} accepted. Active connections: {}",
            addr,
            port,
            current + 1
        );

        abi::envoy_dynamic_module_type_on_network_filter_data_status::Continue
    }

    fn on_read(
        &mut self,
        _envoy_filter: &mut ENF,
        _data_length: usize,
        _end_stream: bool,
    ) -> abi::envoy_dynamic_module_type_on_network_filter_data_status {
        // Pass through all data.
        abi::envoy_dynamic_module_type_on_network_filter_data_status::Continue
    }

    fn on_write(
        &mut self,
        _envoy_filter: &mut ENF,
        _data_length: usize,
        _end_stream: bool,
    ) -> abi::envoy_dynamic_module_type_on_network_filter_data_status {
        // Pass through all data.
        abi::envoy_dynamic_module_type_on_network_filter_data_status::Continue
    }

    fn on_event(
        &mut self,
        envoy_filter: &mut ENF,
        event: abi::envoy_dynamic_module_type_network_connection_event,
    ) {
        match event {
            abi::envoy_dynamic_module_type_network_connection_event::RemoteClose
            | abi::envoy_dynamic_module_type_network_connection_event::LocalClose => {
                if self.connection_counted {
                    let previous = self
                        .shared_state
                        .active_connections
                        .fetch_sub(1, Ordering::SeqCst);
                    let _ = envoy_filter
                        .set_gauge(self.active_connections_gauge, previous.saturating_sub(1));
                    envoy_log_debug!("Connection closed. Active connections: {}", previous - 1);
                }
            }
            _ => {}
        }
    }
}

impl Drop for RateLimiterFilter {
    fn drop(&mut self) {
        // Ensure we decrement the counter if the filter is dropped without on_event being called.
        if self.connection_counted {
            self.shared_state
                .active_connections
                .fetch_sub(1, Ordering::SeqCst);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_rate_limiter_config_parsing() {
        let config = r#"{"max_connections": 50}"#;
        let config_data: RateLimiterConfigData = serde_json::from_str(config).unwrap();
        assert_eq!(config_data.max_connections, 50);
        assert_eq!(config_data.reject_message, default_reject_message());
    }

    #[test]
    fn test_rate_limiter_config_with_message() {
        let config = r#"{"max_connections": 10, "reject_message": "Go away!"}"#;
        let config_data: RateLimiterConfigData = serde_json::from_str(config).unwrap();
        assert_eq!(config_data.max_connections, 10);
        assert_eq!(config_data.reject_message, "Go away!");
    }

    #[test]
    fn test_shared_state_atomic_operations() {
        let state = SharedConnectionState {
            active_connections: AtomicU64::new(0),
        };

        // Simulate accepting connections.
        let v1 = state.active_connections.fetch_add(1, Ordering::SeqCst);
        assert_eq!(v1, 0);

        let v2 = state.active_connections.fetch_add(1, Ordering::SeqCst);
        assert_eq!(v2, 1);

        // Check current value.
        assert_eq!(state.active_connections.load(Ordering::SeqCst), 2);

        // Simulate closing a connection.
        let v3 = state.active_connections.fetch_sub(1, Ordering::SeqCst);
        assert_eq!(v3, 2);
        assert_eq!(state.active_connections.load(Ordering::SeqCst), 1);
    }

    #[test]
    fn test_default_reject_message() {
        assert_eq!(default_reject_message(), "Connection limit exceeded");
    }
}
