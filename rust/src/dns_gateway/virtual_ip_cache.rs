//! Virtual IP cache for mapping domains to synthetic IPv4 addresses.
//!
//! This module provides a thread-safe cache that allocates sequential virtual IPs from a
//! configured subnet. The DNS gateway filter populates this cache, and the cache lookup
//! network filter reads from it.

use dashmap::mapref::entry::Entry;
use dashmap::DashMap;
use envoy_proxy_dynamic_modules_rust_sdk::*;
use parking_lot::Mutex;
use std::collections::HashMap;
use std::net::Ipv4Addr;
use std::sync::{Arc, OnceLock};

/// A destination entry in the virtual IP cache.
///
/// Maps a fully qualified domain name (e.g. "bucket-1.aws.com") to its
/// associated metadata (e.g. the upstream cluster to use).
#[derive(Clone, Debug)]
pub struct Destination {
    domain: String,
    metadata: HashMap<String, String>,
}

impl Destination {
    pub fn new(domain: String, metadata: HashMap<String, String>) -> Self {
        Self { domain, metadata }
    }

    pub fn domain(&self) -> &str {
        &self.domain
    }

    pub fn metadata(&self) -> &HashMap<String, String> {
        &self.metadata
    }
}

/// Thread-safe cache for virtual IP allocation and lookup.
///
/// Allocates sequential IPs from a configured base address within a CIDR subnet.
/// Deduplicates allocations by domain name.
pub struct VirtualIpCache {
    base_ip: u32,
    capacity: u32,
    alloc_offset: Mutex<u32>,
    ip_to_destination: DashMap<Ipv4Addr, Destination>,
    domain_to_ip: DashMap<String, Ipv4Addr>,
}

impl VirtualIpCache {
    pub fn new(base_ip: u32, prefix_len: u8) -> Self {
        assert!(
            (1..=32).contains(&prefix_len),
            "prefix_len must be between 1 and 32"
        );
        let subnet_size = 1u32 << (32 - prefix_len);
        let host_mask = subnet_size - 1;
        let capacity = subnet_size - (base_ip & host_mask);
        envoy_log_info!(
            "Creating cache with prefix_len={}, capacity={}",
            prefix_len,
            capacity
        );
        Self {
            base_ip,
            capacity,
            alloc_offset: Mutex::new(0),
            ip_to_destination: DashMap::new(),
            domain_to_ip: DashMap::new(),
        }
    }

    /// Allocates a virtual IP for the given destination.
    ///
    /// Returns the same IP if the domain was previously allocated.
    /// Returns `None` if the subnet is exhausted.
    pub fn allocate(&self, destination: Destination) -> Option<Ipv4Addr> {
        if let Some(ip) = self.domain_to_ip.get(&destination.domain) {
            return Some(*ip);
        }

        match self.domain_to_ip.entry(destination.domain.clone()) {
            Entry::Occupied(entry) => Some(*entry.get()),
            Entry::Vacant(entry) => {
                let mut offset = self.alloc_offset.lock();

                if *offset >= self.capacity {
                    envoy_log_error!(
                        "IP allocation exhausted, tried to allocate #{} but max is {}",
                        *offset, self.capacity
                    );
                    return None;
                }

                let ip = Ipv4Addr::from(self.base_ip + *offset);
                *offset += 1;

                envoy_log_info!(
                    "Allocated virtual IP {} for domain {}",
                    ip,
                    destination.domain
                );

                self.ip_to_destination.insert(ip, destination);
                entry.insert(ip);

                Some(ip)
            }
        }
    }

    /// Looks up the destination for a given virtual IP.
    pub fn lookup(&self, ip: Ipv4Addr) -> Option<Destination> {
        self.ip_to_destination.get(&ip).as_deref().cloned()
    }
}

static VIRTUAL_IP_CACHE: OnceLock<Arc<VirtualIpCache>> = OnceLock::new();

/// Initializes the global virtual IP cache. First call wins; subsequent calls are ignored.
pub fn init_cache(base_ip: u32, prefix_len: u8) {
    let cache = Arc::new(VirtualIpCache::new(base_ip, prefix_len));

    if VIRTUAL_IP_CACHE.set(cache).is_err() {
        envoy_log_warn!("Cache already initialized, ignoring duplicate init");
        return;
    }

    envoy_log_info!(
        "Initialized with base IP {}, prefix_len {}",
        Ipv4Addr::from(base_ip),
        prefix_len
    );
}

pub fn get_cache() -> &'static Arc<VirtualIpCache> {
    VIRTUAL_IP_CACHE
        .get()
        .expect("cache not initialized, dns_gateway must be configured first")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cache_new() {
        let cache = VirtualIpCache::new(0x0A0A0000, 24); // 10.10.0.0/24
        assert_eq!(cache.base_ip, 0x0A0A0000);
        assert_eq!(cache.capacity, 256);
    }

    #[test]
    fn test_prefix_len_calculations() {
        let cache_24 = VirtualIpCache::new(0, 24);
        assert_eq!(cache_24.capacity, 256); // 2^(32-24) = 256

        let cache_16 = VirtualIpCache::new(0, 16);
        assert_eq!(cache_16.capacity, 65536); // 2^(32-16) = 65536

        let cache_32 = VirtualIpCache::new(0, 32);
        assert_eq!(cache_32.capacity, 1); // 2^(32-32) = 1

        let cache_8 = VirtualIpCache::new(0, 8);
        assert_eq!(cache_8.capacity, 16777216); // 2^(32-8) = 16777216
    }

    #[test]
    #[should_panic(expected = "prefix_len must be between 1 and 32")]
    fn test_invalid_prefix_len_zero() {
        VirtualIpCache::new(0, 0);
    }

    #[test]
    #[should_panic(expected = "prefix_len must be between 1 and 32")]
    fn test_invalid_prefix_len_too_large() {
        VirtualIpCache::new(0, 33);
    }

    #[test]
    fn test_unaligned_base_ip_caps_capacity() {
        // 10.10.0.200/24 — only 56 IPs remain in the subnet (200..=255)
        let cache = VirtualIpCache::new(u32::from(Ipv4Addr::new(10, 10, 0, 200)), 24);
        assert_eq!(cache.capacity, 56);
    }

    #[test]
    fn test_unaligned_base_ip_exhaustion() {
        // 10.10.0.252/24 — only 4 IPs remain (252, 253, 254, 255)
        let base = u32::from(Ipv4Addr::new(10, 10, 0, 252));
        let cache = VirtualIpCache::new(base, 24);
        assert_eq!(cache.capacity, 4);

        for i in 0..4 {
            let dest = Destination::new(format!("domain{}.com", i), HashMap::new());
            assert!(cache.allocate(dest).is_some());
        }

        let ip_last = cache.allocate(Destination::new("last-ok.com".to_string(), HashMap::new()));
        assert!(ip_last.is_none());

        let first = cache.lookup(Ipv4Addr::new(10, 10, 0, 252)).unwrap();
        assert_eq!(first.domain(), "domain0.com");
        let last = cache.lookup(Ipv4Addr::new(10, 10, 0, 255)).unwrap();
        assert_eq!(last.domain(), "domain3.com");
    }

    #[test]
    fn test_aligned_base_ip_full_subnet() {
        let cache = VirtualIpCache::new(u32::from(Ipv4Addr::new(10, 10, 0, 0)), 24);
        assert_eq!(cache.capacity, 256);
    }

    #[test]
    fn test_allocate_sequential_ips() {
        let cache = VirtualIpCache::new(0x0A0A0000, 24); // 10.10.0.0/24

        let dest1 = Destination::new("api.aws.com".to_string(), HashMap::new());
        let dest2 = Destination::new("s3.aws.com".to_string(), HashMap::new());

        let ip1 = cache.allocate(dest1).unwrap();
        let ip2 = cache.allocate(dest2).unwrap();

        assert_eq!(ip1, Ipv4Addr::new(10, 10, 0, 0));
        assert_eq!(ip2, Ipv4Addr::new(10, 10, 0, 1));
    }

    #[test]
    fn test_allocate_same_domain_returns_same_ip() {
        let cache = VirtualIpCache::new(0x0A0A0000, 24);

        let dest = Destination::new("api.aws.com".to_string(), HashMap::new());

        let ip1 = cache.allocate(dest.clone()).unwrap();
        let ip2 = cache.allocate(dest.clone()).unwrap();

        assert_eq!(ip1, ip2);
    }

    #[test]
    fn test_lookup_allocated_ip() {
        let cache = VirtualIpCache::new(0x0A0A0000, 24);

        let mut metadata = HashMap::new();
        metadata.insert("cluster".to_string(), "aws_cluster".to_string());

        let dest = Destination::new("api.aws.com".to_string(), metadata);

        let ip = cache.allocate(dest).unwrap();

        let result = cache.lookup(ip).unwrap();
        assert_eq!(result.domain(), "api.aws.com");
        assert_eq!(result.metadata().get("cluster").unwrap(), "aws_cluster");
    }

    #[test]
    fn test_lookup_unallocated_ip() {
        let cache = VirtualIpCache::new(0x0A0A0000, 24);
        let unallocated_ip = Ipv4Addr::new(10, 10, 0, 100);

        assert!(cache.lookup(unallocated_ip).is_none());
    }

    #[test]
    fn test_allocation_exhaustion_returns_none() {
        let cache = VirtualIpCache::new(0x0A0A0000, 30); // 4 IPs available (2^(32-30))

        for i in 0..4 {
            let dest = Destination::new(format!("domain{}.com", i), HashMap::new());
            assert!(
                cache.allocate(dest).is_some(),
                "allocation {} should succeed",
                i
            );
        }

        let overflow = Destination::new("overflow.com".to_string(), HashMap::new());
        assert!(cache.allocate(overflow).is_none());
    }

    #[test]
    fn test_metadata_preserved() {
        let cache = VirtualIpCache::new(0x0A0A0000, 24);

        let mut metadata = HashMap::new();
        metadata.insert("key1".to_string(), "value1".to_string());
        metadata.insert("key2".to_string(), "value2".to_string());

        let dest = Destination::new("test.com".to_string(), metadata);

        let ip = cache.allocate(dest).unwrap();
        let result = cache.lookup(ip).unwrap();

        assert_eq!(result.metadata().len(), 2);
        assert_eq!(result.metadata().get("key1").unwrap(), "value1");
        assert_eq!(result.metadata().get("key2").unwrap(), "value2");
    }
}
