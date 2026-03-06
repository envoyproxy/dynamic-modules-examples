//! Envoy Dynamic Modules Rust SDK Examples
//!
//! This crate contains example implementations of Envoy dynamic modules using the Rust SDK.
//!
//! # HTTP Filters
//!
//! The main library exports HTTP filter examples that work with `declare_init_functions!`:
//! - `passthrough` - A minimal filter that passes all data through unchanged.
//! - `access_logger` - Logs request/response information.
//! - `random_auth` - Randomly rejects requests (for testing).
//! - `zero_copy_regex_waf` - Zero-copy regex-based WAF filter.
//! - `header_mutation` - Adds/removes/modifies headers.
//! - `metrics` - Collects request/response metrics.
//!
//! # Network Filters
//!
//! Network filter examples are provided as public modules. To use them, create a separate
//! crate that includes this library and uses `declare_network_filter_init_functions!` with
//! the module's `new_filter_config` function.
//!
//! Available network filters:
//! - [`network_echo`] - Echoes data back to the client.
//! - [`network_rate_limiter`] - Limits concurrent connections.
//! - [`network_protocol_logger`] - Logs protocol information.
//! - [`network_redis`] - Redis RESP protocol parser and command filter.
//!
//! # Listener Filters
//!
//! Listener filter examples are provided as public modules. To use them, create a separate
//! crate that includes this library and uses `declare_listener_filter_init_functions!` with
//! the module's `new_filter_config` function.
//!
//! Available listener filters:
//! - [`listener_ip_allowlist`] - IP allowlist/blocklist filter.
//! - [`listener_tls_detector`] - TLS protocol detection filter.
//! - [`listener_sni_router`] - SNI-based routing filter.

use envoy_proxy_dynamic_modules_rust_sdk::*;

// HTTP filter examples.
mod http_access_logger;
mod http_header_mutation;
mod http_metrics;
mod http_passthrough;
mod http_random_auth;
mod http_zero_copy_regex_waf;

// Network filter examples.
// These modules can be used to create standalone network filter cdylibs.
// See each module's documentation for usage instructions.
pub mod network_echo;
pub mod network_protocol_logger;
pub mod network_rate_limiter;
pub mod network_redis;

// Listener filter examples.
// These modules can be used to create standalone listener filter cdylibs.
// See each module's documentation for usage instructions.
pub mod listener_ip_allowlist;
pub mod listener_sni_router;
pub mod listener_tls_detector;

declare_init_functions!(init, new_http_filter_config_fn);

/// This implements the [`envoy_proxy_dynamic_modules_rust_sdk::ProgramInitFunction`].
///
/// This is called exactly once when the module is loaded. It can be used to
/// initialize global state as well as check the runtime environment to ensure that
/// the module is running in a supported environment.
///
/// Returning `false` will cause Envoy to reject the config hence the
/// filter will not be loaded.
fn init() -> bool {
    true
}

/// This implements the [`envoy_proxy_dynamic_modules_rust_sdk::NewHttpFilterConfigFunction`].
///
/// This is the entrypoint every time a new HTTP filter is created via the DynamicModuleFilter config.
///
/// Each argument matches the corresponding argument in the Envoy config here:
/// https://www.envoyproxy.io/docs/envoy/latest/api-v3/extensions/dynamic_modules/v3/dynamic_modules.proto#envoy-v3-api-msg-extensions-dynamic-modules-v3-dynamicmoduleconfig
///
/// Returns None if the filter name or config is determined to be invalid by each filter's `new` function.
fn new_http_filter_config_fn<EC: EnvoyHttpFilterConfig, EHF: EnvoyHttpFilter>(
    envoy_filter_config: &mut EC,
    filter_name: &str,
    filter_config: &[u8],
) -> Option<Box<dyn HttpFilterConfig<EHF>>> {
    let filter_config = std::str::from_utf8(filter_config).unwrap();
    match filter_name {
        "passthrough" => Some(Box::new(http_passthrough::FilterConfig::new(filter_config))),
        "access_logger" => http_access_logger::FilterConfig::new(filter_config)
            .map(|config| Box::new(config) as Box<dyn HttpFilterConfig<EHF>>),
        "random_auth" => Some(Box::new(http_random_auth::FilterConfig::new(filter_config))),
        "zero_copy_regex_waf" => http_zero_copy_regex_waf::FilterConfig::new(filter_config)
            .map(|config| Box::new(config) as Box<dyn HttpFilterConfig<EHF>>),
        "header_mutation" => http_header_mutation::FilterConfig::new(filter_config)
            .map(|config| Box::new(config) as Box<dyn HttpFilterConfig<EHF>>),
        "metrics" => http_metrics::FilterConfig::new(filter_config, envoy_filter_config)
            .map(|config| Box::new(config) as Box<dyn HttpFilterConfig<EHF>>),
        _ => panic!("Unknown filter name: {filter_name}"),
    }
}
