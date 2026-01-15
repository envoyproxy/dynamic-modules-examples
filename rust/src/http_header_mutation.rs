use envoy_proxy_dynamic_modules_rust_sdk::*;
use serde::{Deserialize, Serialize};

/// This implements the [`envoy_proxy_dynamic_modules_rust_sdk::HttpFilterConfig`] trait.
///
/// The trait corresponds to a Envoy filter chain configuration.
#[derive(Serialize, Deserialize, Debug)]
pub struct FilterConfig {
    request_headers: Vec<(String, String)>,
    remove_request_headers: Vec<String>,
    response_headers: Vec<(String, String)>,
    remove_response_headers: Vec<String>,
}

impl FilterConfig {
    /// This is the constructor for the [`FilterConfig`].
    ///
    /// filter_config is the filter config from the Envoy config here:
    /// https://www.envoyproxy.io/docs/envoy/latest/api-v3/extensions/dynamic_modules/v3/dynamic_modules.proto#envoy-v3-api-msg-extensions-dynamic-modules-v3-dynamicmoduleconfig
    pub fn new(filter_config: &str) -> Option<Self> {
        let filter_config: FilterConfig = match serde_json::from_str(filter_config) {
            Ok(cfg) => cfg,
            Err(err) => {
                eprintln!("Error parsing filter config: {err}");
                return None;
            }
        };
        Some(filter_config)
    }
}

impl<EHF: EnvoyHttpFilter> HttpFilterConfig<EHF> for FilterConfig {
    /// This is called for each new HTTP filter.
    fn new_http_filter(&self, _envoy: &mut EHF) -> Box<dyn HttpFilter<EHF>> {
        Box::new(Filter {
            request_headers: self.request_headers.clone(),
            remove_request_headers: self.remove_request_headers.clone(),
            response_headers: self.response_headers.clone(),
            remove_response_headers: self.remove_response_headers.clone(),
        })
    }
}

/// This implements the [`envoy_proxy_dynamic_modules_rust_sdk::HttpFilter`] trait.
///
/// This sets the request and response headers to the values specified in the filter config.
pub struct Filter {
    request_headers: Vec<(String, String)>,
    remove_request_headers: Vec<String>,
    response_headers: Vec<(String, String)>,
    remove_response_headers: Vec<String>,
}

/// This implements the [`envoy_proxy_dynamic_modules_rust_sdk::HttpFilter`] trait.
impl<EHF: EnvoyHttpFilter> HttpFilter<EHF> for Filter {
    fn on_request_headers(
        &mut self,
        envoy_filter: &mut EHF,
        _end_of_stream: bool,
    ) -> abi::envoy_dynamic_module_type_on_http_filter_request_headers_status {
        for (key, value) in &self.request_headers {
            envoy_filter.set_request_header(key, value.as_bytes());
        }
        for key in &self.remove_request_headers {
            envoy_filter.remove_request_header(key);
        }
        abi::envoy_dynamic_module_type_on_http_filter_request_headers_status::Continue
    }

    fn on_response_headers(
        &mut self,
        envoy_filter: &mut EHF,
        _end_of_stream: bool,
    ) -> abi::envoy_dynamic_module_type_on_http_filter_response_headers_status {
        let downstream_addr = envoy_filter
            .get_attribute_string(abi::envoy_dynamic_module_type_attribute_id::SourceAddress)
            .expect("Failed to get source address");
        let downstream_addr = Vec::from(downstream_addr.as_slice());
        envoy_filter.set_response_header("X-Downstream-Address", downstream_addr.as_slice());

        let upstream_addr = envoy_filter
            .get_attribute_string(abi::envoy_dynamic_module_type_attribute_id::UpstreamAddress)
            .expect("Failed to get upstream address");
        let upstream_addr = Vec::from(upstream_addr.as_slice());
        envoy_filter.set_response_header("X-Upstream-Address", upstream_addr.as_slice());

        let response_code = envoy_filter
            .get_attribute_int(abi::envoy_dynamic_module_type_attribute_id::ResponseCode)
            .expect("Failed to get response code");
        let response_code = response_code.to_string();
        envoy_filter.set_response_header("X-Response-Code", response_code.as_bytes());

        for (key, value) in &self.response_headers {
            envoy_filter.set_response_header(key, value.as_bytes());
        }
        for key in &self.remove_response_headers {
            envoy_filter.remove_response_header(key);
        }
        abi::envoy_dynamic_module_type_on_http_filter_response_headers_status::Continue
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    /// This demonstrates how to write a test without Envoy using a mock provided by the SDK.
    fn test_filter() {
        let mut envoy_filter = envoy_proxy_dynamic_modules_rust_sdk::MockEnvoyHttpFilter::new();
        let mut filter = Filter {
            request_headers: vec![("X-Foo".to_string(), "bar".to_string())],
            remove_request_headers: vec!["To-Remove".to_string()],
            response_headers: vec![("X-Bar".to_string(), "foo".to_string())],
            remove_response_headers: vec!["To-Remove".to_string()],
        };
        envoy_filter
            .expect_get_attribute_string()
            .returning(|id| match id {
                abi::envoy_dynamic_module_type_attribute_id::SourceAddress => {
                    return Some(EnvoyBuffer::new("1.1.1.1:12345"));
                }
                abi::envoy_dynamic_module_type_attribute_id::UpstreamAddress => {
                    return Some(EnvoyBuffer::new("2.2.2.2:12345"));
                }
                _ => panic!("Unexpected attribute id"),
            });
        envoy_filter
            .expect_get_attribute_int()
            .returning(|id| match id {
                abi::envoy_dynamic_module_type_attribute_id::ResponseCode => {
                    return Some(200);
                }
                _ => panic!("Unexpected attribute id"),
            });
        envoy_filter
            .expect_set_response_header()
            .returning(|key, value| {
                assert_eq!(key, "X-Downstream-Address");
                assert_eq!(value, b"1.1.1.1:12345");
                return true;
            })
            .once();
        envoy_filter
            .expect_set_response_header()
            .returning(|key, value| {
                assert_eq!(key, "X-Upstream-Address");
                assert_eq!(value, b"2.2.2.2:12345");
                return true;
            })
            .once();
        envoy_filter
            .expect_set_response_header()
            .returning(|key, value| {
                assert_eq!(key, "X-Response-Code");
                assert_eq!(value, b"200");
                return true;
            })
            .once();
        envoy_filter
            .expect_set_request_header()
            .returning(|key, value| {
                assert_eq!(key, "X-Foo");
                assert_eq!(value, b"bar");
                return true;
            });
        envoy_filter
            .expect_remove_request_header()
            .returning(|key| {
                assert_eq!(key, "To-Remove");
                return true;
            });
        envoy_filter
            .expect_set_response_header()
            .returning(|key, value| {
                assert_eq!(key, "X-Bar");
                assert_eq!(value, b"foo");
                return true;
            });
        envoy_filter
            .expect_remove_response_header()
            .returning(|key| {
                assert_eq!(key, "To-Remove");
                return true;
            });
        assert_eq!(
            filter.on_request_headers(&mut envoy_filter, false),
            abi::envoy_dynamic_module_type_on_http_filter_request_headers_status::Continue
        );
        assert_eq!(
            filter.on_response_headers(&mut envoy_filter, false),
            abi::envoy_dynamic_module_type_on_http_filter_response_headers_status::Continue
        );
    }
}
