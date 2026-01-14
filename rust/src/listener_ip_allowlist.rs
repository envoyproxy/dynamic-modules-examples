//! An IP allowlist/blocklist filter for connection-level access control.
//!
//! This filter demonstrates:
//! 1. Inspecting connection addresses before the connection is established.
//! 2. IP allowlist/blocklist rules with CIDR support.
//! 3. Working with IPv4 and IPv6 addresses.
//!
//! Configuration format (JSON):
//! ```json
//! {
//!   "mode": "allowlist",
//!   "addresses": ["192.168.1.0/24", "10.0.0.1"],
//!   "log_blocked": true
//! }
//! ```
//!
//! Modes:
//! - "allowlist": Only allow connections from listed addresses (block all others).
//! - "blocklist": Block connections from listed addresses (allow all others).
//!
//! To use this filter as a standalone module, create a separate crate with:
//! ```ignore
//! use envoy_proxy_dynamic_modules_rust_sdk::*;
//! declare_listener_filter_init_functions!(init, listener_ip_allowlist::new_filter_config);
//! ```

use envoy_proxy_dynamic_modules_rust_sdk::*;
use serde::{Deserialize, Serialize};
use std::net::{IpAddr, Ipv4Addr, Ipv6Addr};

/// Filter mode.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum FilterMode {
    Allowlist,
    Blocklist,
}

/// Configuration data parsed from the filter config JSON.
#[derive(Serialize, Deserialize, Debug, Clone)]
struct IpAllowlistConfigData {
    /// Filter mode: allowlist or blocklist.
    mode: FilterMode,
    /// List of IP addresses or CIDR ranges.
    addresses: Vec<String>,
    /// Whether to log blocked connections.
    #[serde(default = "default_true")]
    log_blocked: bool,
}

fn default_true() -> bool {
    true
}

/// Parsed IP rule - either a single IP or a CIDR range.
#[derive(Debug, Clone)]
pub enum IpRule {
    Single(IpAddr),
    CidrV4 { network: u32, prefix_len: u8 },
    CidrV6 { network: u128, prefix_len: u8 },
}

impl IpRule {
    /// Parse an IP rule from a string.
    pub fn parse(s: &str) -> Option<Self> {
        if let Some((ip_str, prefix_str)) = s.split_once('/') {
            // CIDR notation.
            let prefix_len: u8 = prefix_str.parse().ok()?;

            if let Ok(ip) = ip_str.parse::<Ipv4Addr>() {
                if prefix_len > 32 {
                    return None;
                }
                let network = u32::from(ip);
                return Some(IpRule::CidrV4 {
                    network,
                    prefix_len,
                });
            }

            if let Ok(ip) = ip_str.parse::<Ipv6Addr>() {
                if prefix_len > 128 {
                    return None;
                }
                let network = u128::from(ip);
                return Some(IpRule::CidrV6 {
                    network,
                    prefix_len,
                });
            }

            None
        } else {
            // Single IP address.
            s.parse::<IpAddr>().ok().map(IpRule::Single)
        }
    }

    /// Check if an IP address matches this rule.
    pub fn matches(&self, ip: &IpAddr) -> bool {
        match (self, ip) {
            (IpRule::Single(rule_ip), addr) => rule_ip == addr,
            (
                IpRule::CidrV4 {
                    network,
                    prefix_len,
                },
                IpAddr::V4(addr),
            ) => {
                let addr_bits = u32::from(*addr);
                let mask = if *prefix_len == 0 {
                    0
                } else {
                    !0u32 << (32 - prefix_len)
                };
                (addr_bits & mask) == (network & mask)
            }
            (
                IpRule::CidrV6 {
                    network,
                    prefix_len,
                },
                IpAddr::V6(addr),
            ) => {
                let addr_bits = u128::from(*addr);
                let mask = if *prefix_len == 0 {
                    0
                } else {
                    !0u128 << (128 - prefix_len)
                };
                (addr_bits & mask) == (network & mask)
            }
            _ => false, // IPv4 rule vs IPv6 address or vice versa.
        }
    }
}

/// The filter configuration.
pub struct IpAllowlistFilterConfig {
    mode: FilterMode,
    rules: Vec<IpRule>,
    log_blocked: bool,
    allowed_connections: EnvoyCounterId,
    blocked_connections: EnvoyCounterId,
}

/// Creates a new IP allowlist filter configuration.
pub fn new_filter_config<EC: EnvoyListenerFilterConfig, ELF: EnvoyListenerFilter>(
    envoy_filter_config: &mut EC,
    _name: &str,
    config: &[u8],
) -> Option<Box<dyn ListenerFilterConfig<ELF>>> {
    let config_data: IpAllowlistConfigData = match serde_json::from_slice(config) {
        Ok(cfg) => cfg,
        Err(err) => {
            eprintln!("Error parsing IP allowlist config: {err}");
            return None;
        }
    };

    // Parse IP rules.
    let mut rules = Vec::new();
    for addr in &config_data.addresses {
        match IpRule::parse(addr) {
            Some(rule) => rules.push(rule),
            None => {
                eprintln!("Invalid IP address or CIDR: {addr}");
                return None;
            }
        }
    }

    if rules.is_empty() {
        eprintln!("At least one IP address is required");
        return None;
    }

    let allowed_connections = envoy_filter_config
        .define_counter("ip_filter_allowed_connections_total")
        .expect("Failed to define allowed_connections counter");

    let blocked_connections = envoy_filter_config
        .define_counter("ip_filter_blocked_connections_total")
        .expect("Failed to define blocked_connections counter");

    Some(Box::new(IpAllowlistFilterConfig {
        mode: config_data.mode,
        rules,
        log_blocked: config_data.log_blocked,
        allowed_connections,
        blocked_connections,
    }))
}

impl<ELF: EnvoyListenerFilter> ListenerFilterConfig<ELF> for IpAllowlistFilterConfig {
    fn new_listener_filter(&self, _envoy: &mut ELF) -> Box<dyn ListenerFilter<ELF>> {
        Box::new(IpAllowlistFilter {
            mode: self.mode.clone(),
            rules: self.rules.clone(),
            log_blocked: self.log_blocked,
            allowed_connections: self.allowed_connections,
            blocked_connections: self.blocked_connections,
        })
    }
}

/// The IP allowlist filter.
struct IpAllowlistFilter {
    mode: FilterMode,
    rules: Vec<IpRule>,
    log_blocked: bool,
    allowed_connections: EnvoyCounterId,
    blocked_connections: EnvoyCounterId,
}

impl IpAllowlistFilter {
    /// Check if an IP address matches any of the rules.
    fn matches_any_rule(&self, ip: &IpAddr) -> bool {
        self.rules.iter().any(|rule| rule.matches(ip))
    }

    /// Determine if a connection should be allowed based on mode and rules.
    fn should_allow(&self, ip: &IpAddr) -> bool {
        let matches = self.matches_any_rule(ip);
        match self.mode {
            FilterMode::Allowlist => matches,  // Allow only if in list.
            FilterMode::Blocklist => !matches, // Allow only if NOT in list.
        }
    }
}

impl<ELF: EnvoyListenerFilter> ListenerFilter<ELF> for IpAllowlistFilter {
    fn on_accept(
        &mut self,
        envoy_filter: &mut ELF,
    ) -> abi::envoy_dynamic_module_type_on_listener_filter_status {
        // Get the remote address.
        let (addr_str, port) = match envoy_filter.get_remote_address() {
            Some(addr) => addr,
            None => {
                // If we can't get the address, allow by default.
                envoy_log_warn!("Could not get remote address. Allowing connection.");
                return abi::envoy_dynamic_module_type_on_listener_filter_status::Continue;
            }
        };

        // Parse the IP address.
        let ip: IpAddr = match addr_str.parse() {
            Ok(ip) => ip,
            Err(_) => {
                envoy_log_warn!(
                    "Could not parse IP address: {}. Allowing connection.",
                    addr_str
                );
                return abi::envoy_dynamic_module_type_on_listener_filter_status::Continue;
            }
        };

        if self.should_allow(&ip) {
            let _ = envoy_filter.increment_counter(self.allowed_connections, 1);
            envoy_log_debug!("Connection from {}:{} allowed", addr_str, port);
            abi::envoy_dynamic_module_type_on_listener_filter_status::Continue
        } else {
            let _ = envoy_filter.increment_counter(self.blocked_connections, 1);

            if self.log_blocked {
                let mode_str = match self.mode {
                    FilterMode::Allowlist => "not in allowlist",
                    FilterMode::Blocklist => "in blocklist",
                };
                envoy_log_warn!(
                    "Connection from {}:{} blocked ({})",
                    addr_str,
                    port,
                    mode_str
                );
            }

            // Close the socket to reject the connection.
            envoy_filter.set_downstream_transport_failure_reason("IP address blocked by filter");

            abi::envoy_dynamic_module_type_on_listener_filter_status::Continue
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_ip_allowlist_config_parsing() {
        let config = r#"{"mode": "allowlist", "addresses": ["192.168.1.0/24", "10.0.0.1"]}"#;
        let config_data: IpAllowlistConfigData = serde_json::from_str(config).unwrap();
        assert_eq!(config_data.mode, FilterMode::Allowlist);
        assert_eq!(config_data.addresses.len(), 2);
    }

    #[test]
    fn test_ip_blocklist_config_parsing() {
        let config = r#"{"mode": "blocklist", "addresses": ["10.0.0.0/8"], "log_blocked": false}"#;
        let config_data: IpAllowlistConfigData = serde_json::from_str(config).unwrap();
        assert_eq!(config_data.mode, FilterMode::Blocklist);
        assert!(!config_data.log_blocked);
    }

    #[test]
    fn test_ip_rule_parse_single_ipv4() {
        let rule = IpRule::parse("192.168.1.1").unwrap();
        assert!(matches!(rule, IpRule::Single(IpAddr::V4(_))));
    }

    #[test]
    fn test_ip_rule_parse_cidr_ipv4() {
        let rule = IpRule::parse("192.168.1.0/24").unwrap();
        assert!(matches!(rule, IpRule::CidrV4 { .. }));
    }

    #[test]
    fn test_ip_rule_parse_single_ipv6() {
        let rule = IpRule::parse("::1").unwrap();
        assert!(matches!(rule, IpRule::Single(IpAddr::V6(_))));
    }

    #[test]
    fn test_ip_rule_parse_cidr_ipv6() {
        let rule = IpRule::parse("2001:db8::/32").unwrap();
        assert!(matches!(rule, IpRule::CidrV6 { .. }));
    }

    #[test]
    fn test_ip_rule_parse_invalid() {
        assert!(IpRule::parse("invalid").is_none());
        assert!(IpRule::parse("192.168.1.0/33").is_none());
        assert!(IpRule::parse("::1/129").is_none());
    }

    #[test]
    fn test_ip_rule_matches_single() {
        let rule = IpRule::parse("192.168.1.1").unwrap();
        let ip: IpAddr = "192.168.1.1".parse().unwrap();
        let other_ip: IpAddr = "192.168.1.2".parse().unwrap();

        assert!(rule.matches(&ip));
        assert!(!rule.matches(&other_ip));
    }

    #[test]
    fn test_ip_rule_matches_cidr() {
        let rule = IpRule::parse("192.168.1.0/24").unwrap();
        let ip_in_range: IpAddr = "192.168.1.100".parse().unwrap();
        let ip_out_of_range: IpAddr = "192.168.2.1".parse().unwrap();

        assert!(rule.matches(&ip_in_range));
        assert!(!rule.matches(&ip_out_of_range));
    }

    #[test]
    fn test_cidr_edge_cases() {
        // /0 should match everything.
        let rule_v4 = IpRule::parse("0.0.0.0/0").unwrap();
        let any_ip: IpAddr = "1.2.3.4".parse().unwrap();
        assert!(rule_v4.matches(&any_ip));

        // /32 should match only exact IP.
        let rule_exact = IpRule::parse("192.168.1.1/32").unwrap();
        let exact_ip: IpAddr = "192.168.1.1".parse().unwrap();
        let other_ip: IpAddr = "192.168.1.2".parse().unwrap();
        assert!(rule_exact.matches(&exact_ip));
        assert!(!rule_exact.matches(&other_ip));
    }
}
