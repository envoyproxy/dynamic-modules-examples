use envoy_proxy_dynamic_modules_rust_sdk::*;

/// This implements the [`envoy_proxy_dynamic_modules_rust_sdk::HttpFilterConfig`] trait.
///
/// The trait corresponds to a Envoy filter chain configuration.
pub struct FilterConfig {
    _filter_config: String,
}

impl FilterConfig {
    /// This is the constructor for the [`FilterConfig`].
    ///
    /// filter_config is the filter config from the Envoy config here:
    /// https://www.envoyproxy.io/docs/envoy/latest/api-v3/extensions/dynamic_modules/v3/dynamic_modules.proto#envoy-v3-api-msg-extensions-dynamic-modules-v3-dynamicmoduleconfig
    pub fn new(filter_config: &str) -> Self {
        Self {
            _filter_config: filter_config.to_string(),
        }
    }
}

impl<EC: EnvoyHttpFilterConfig, EHF: EnvoyHttpFilter> HttpFilterConfig<EC, EHF> for FilterConfig {
    /// This is called for each new HTTP filter.
    fn new_http_filter(&mut self, _envoy: &mut EC) -> Box<dyn HttpFilter<EHF>> {
        Box::new(Filter {})
    }
}

/// This implements the [`envoy_proxy_dynamic_modules_rust_sdk::HttpFilter`] trait.
///
/// This is a passthrough filter that does nothing.
pub struct Filter {}

/// This implements the [`envoy_proxy_dynamic_modules_rust_sdk::HttpFilter`] trait.
///
/// Default implementation of all methods is to return `Continue`.
impl<EHF: EnvoyHttpFilter> HttpFilter<EHF> for Filter {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    /// This demonstrates how to write a test without Envoy using a mock provided by the SDK.
    fn test_filter() {
        let mut envoy_filter = envoy_proxy_dynamic_modules_rust_sdk::MockEnvoyHttpFilter::new();
        let mut passthrough_filter = Filter {};
        assert_eq!(
            passthrough_filter.on_request_headers(&mut envoy_filter, false),
            abi::envoy_dynamic_module_type_on_http_filter_request_headers_status::Continue
        );
        assert_eq!(
            passthrough_filter.on_request_body(&mut envoy_filter, false),
            abi::envoy_dynamic_module_type_on_http_filter_request_body_status::Continue
        );
        assert_eq!(
            passthrough_filter.on_request_trailers(&mut envoy_filter),
            abi::envoy_dynamic_module_type_on_http_filter_request_trailers_status::Continue
        );
        assert_eq!(
            passthrough_filter.on_response_headers(&mut envoy_filter, false),
            abi::envoy_dynamic_module_type_on_http_filter_response_headers_status::Continue
        );
        assert_eq!(
            passthrough_filter.on_response_body(&mut envoy_filter, false),
            abi::envoy_dynamic_module_type_on_http_filter_response_body_status::Continue
        );
        assert_eq!(
            passthrough_filter.on_response_trailers(&mut envoy_filter),
            abi::envoy_dynamic_module_type_on_http_filter_response_trailers_status::Continue
        );
    }
}
