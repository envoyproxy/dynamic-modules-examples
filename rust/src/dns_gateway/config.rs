use std::collections::HashMap;

#[derive(Clone, PartialEq, serde::Deserialize, serde::Serialize)]
pub struct DnsGatewayConfig {
    /// Base IPv4 address for virtual IP allocation (e.g. "10.10.0.0").
    #[serde(default)]
    pub base_ip: String,
    /// CIDR prefix length (1-32). A /24 gives 256 IPs.
    #[serde(default)]
    pub prefix_len: u32,
    /// Each entry defines a domain pattern and associated metadata.
    #[serde(default)]
    pub domains: Vec<DomainMatcher>,
}

#[derive(Clone, PartialEq, serde::Deserialize, serde::Serialize)]
pub struct DomainMatcher {
    /// Exact domain ("example.com") or wildcard pattern ("*.example.com").
    #[serde(default)]
    pub domain: String,
    /// String key-value pairs exposed in Envoy filter state as `envoy.dns_gateway.metadata.<key>.`
    #[serde(default)]
    pub metadata: HashMap<String, String>,
}
