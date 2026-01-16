//! A simple TCP echo filter that echoes back data received from clients.
//!
//! This filter demonstrates:
//! 1. Basic network filter structure with `NetworkFilterConfig` and `NetworkFilter` traits.
//! 2. Reading from and writing to the connection buffer.
//! 3. Tracking metrics (bytes echoed, active connections).
//!
//! Configuration:
//! The configuration is treated as raw bytes that will be used as a prefix for all echoed data.
//! If empty, data is echoed back without modification.
//!
//! To use this filter as a standalone module, create a separate crate with:
//! ```ignore
//! use envoy_proxy_dynamic_modules_rust_sdk::*;
//! declare_network_filter_init_functions!(init, network_echo::new_filter_config);
//! ```

use envoy_proxy_dynamic_modules_rust_sdk::*;

/// The filter configuration that implements
/// [`envoy_proxy_dynamic_modules_rust_sdk::NetworkFilterConfig`].
///
/// This configuration is shared across all connections handled by this filter chain.
pub struct EchoFilterConfig {
    /// The prefix to prepend to echoed data.
    prefix: Vec<u8>,
    /// Counter for total bytes echoed.
    bytes_echoed: EnvoyCounterId,
    /// Gauge for current active connections.
    active_connections: EnvoyGaugeId,
}

/// Creates a new echo filter configuration.
/// Config is treated as the raw prefix bytes to use (UTF-8 string).
pub fn new_filter_config<EC: EnvoyNetworkFilterConfig, ENF: EnvoyNetworkFilter>(
    envoy_filter_config: &mut EC,
    _name: &str,
    config: &[u8],
) -> Option<Box<dyn NetworkFilterConfig<ENF>>> {
    // Use config bytes directly as the prefix.
    let prefix = config.to_vec();

    let bytes_echoed = envoy_filter_config
        .define_counter("echo_bytes_total")
        .expect("Failed to define bytes_echoed counter");

    let active_connections = envoy_filter_config
        .define_gauge("echo_active_connections")
        .expect("Failed to define active_connections gauge");

    Some(Box::new(EchoFilterConfig {
        prefix,
        bytes_echoed,
        active_connections,
    }))
}

impl<ENF: EnvoyNetworkFilter> NetworkFilterConfig<ENF> for EchoFilterConfig {
    fn new_network_filter(&self, envoy: &mut ENF) -> Box<dyn NetworkFilter<ENF>> {
        // Increment active connections when a new filter is created.
        let _ = envoy.increase_gauge(self.active_connections, 1);

        Box::new(EchoFilter {
            prefix: self.prefix.clone(),
            bytes_echoed: self.bytes_echoed,
            active_connections: self.active_connections,
            total_bytes: 0,
        })
    }
}

/// The echo filter that implements [`envoy_proxy_dynamic_modules_rust_sdk::NetworkFilter`].
///
/// This filter echoes back all received data to the client, optionally with a prefix.
struct EchoFilter {
    /// The prefix to prepend to echoed data.
    prefix: Vec<u8>,
    /// Counter ID for tracking total bytes echoed.
    bytes_echoed: EnvoyCounterId,
    /// Gauge ID for tracking active connections.
    active_connections: EnvoyGaugeId,
    /// Total bytes echoed for this connection.
    total_bytes: u64,
}

impl<ENF: EnvoyNetworkFilter> NetworkFilter<ENF> for EchoFilter {
    fn on_new_connection(
        &mut self,
        envoy_filter: &mut ENF,
    ) -> abi::envoy_dynamic_module_type_on_network_filter_data_status {
        let (addr, port) = envoy_filter.get_remote_address();
        envoy_log_info!("New echo connection from {}:{}", addr, port);
        abi::envoy_dynamic_module_type_on_network_filter_data_status::Continue
    }

    fn on_read(
        &mut self,
        envoy_filter: &mut ENF,
        data_length: usize,
        _end_stream: bool,
    ) -> abi::envoy_dynamic_module_type_on_network_filter_data_status {
        if data_length == 0 {
            return abi::envoy_dynamic_module_type_on_network_filter_data_status::Continue;
        }

        // Get the read buffer chunks.
        let (chunks, _total_size) = envoy_filter.get_read_buffer_chunks();

        // Collect all data from chunks.
        let mut data = Vec::with_capacity(data_length);
        for chunk in &chunks {
            data.extend_from_slice(chunk.as_slice());
        }

        // Drain the read buffer since we've consumed it.
        envoy_filter.drain_read_buffer(data.len());

        // Prepare the response with optional prefix.
        let response = if self.prefix.is_empty() {
            data
        } else {
            let mut response = self.prefix.clone();
            response.extend_from_slice(&data);
            response
        };

        // Track bytes echoed.
        self.total_bytes += response.len() as u64;
        let _ = envoy_filter.increment_counter(self.bytes_echoed, response.len() as u64);

        // Write the response back to the client.
        envoy_filter.write(&response, false);

        abi::envoy_dynamic_module_type_on_network_filter_data_status::StopIteration
    }

    fn on_event(
        &mut self,
        envoy_filter: &mut ENF,
        event: abi::envoy_dynamic_module_type_network_connection_event,
    ) {
        match event {
            abi::envoy_dynamic_module_type_network_connection_event::RemoteClose
            | abi::envoy_dynamic_module_type_network_connection_event::LocalClose => {
                let _ = envoy_filter.decrease_gauge(self.active_connections, 1);
                envoy_log_info!(
                    "Echo connection closed. Total bytes echoed: {}",
                    self.total_bytes
                );
            }
            abi::envoy_dynamic_module_type_network_connection_event::Connected => {
                envoy_log_debug!("Echo connection established");
            }
            _ => {}
        }
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn test_echo_response_no_prefix() {
        let data = b"hello world";
        let prefix: Vec<u8> = vec![];

        let response = if prefix.is_empty() {
            data.to_vec()
        } else {
            let mut response = prefix.clone();
            response.extend_from_slice(data);
            response
        };

        assert_eq!(response, b"hello world".to_vec());
    }

    #[test]
    fn test_echo_response_with_prefix() {
        let data = b"hello";
        let prefix = b"ECHO: ".to_vec();

        let response = if prefix.is_empty() {
            data.to_vec()
        } else {
            let mut response = prefix.clone();
            response.extend_from_slice(data);
            response
        };

        assert_eq!(response, b"ECHO: hello".to_vec());
    }

    #[test]
    fn test_echo_empty_data() {
        let data: &[u8] = b"";
        assert!(data.is_empty());
    }
}
