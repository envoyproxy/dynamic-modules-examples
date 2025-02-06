use envoy_proxy_dynamic_modules_rust_sdk::*;

/// This implements the [`envoy_proxy_dynamic_modules_rust_sdk::HttpFilterConfig`] trait.
pub struct PassthroughHttpFilterConfig {}

impl<EC: EnvoyHttpFilterConfig, EHF: EnvoyHttpFilter> HttpFilterConfig<EC, EHF>
    for PassthroughHttpFilterConfig
{
    fn new_http_filter(&self, _envoy: &mut EC) -> Box<dyn HttpFilter<EHF>> {
        Box::new(PassthroughHttpFilter {})
    }
}

/// This implements the [`envoy_proxy_dynamic_modules_rust_sdk::HttpFilter`] trait.
///
/// This is a passthrough filter that does nothing.
pub struct PassthroughHttpFilter {}

impl<EHF: EnvoyHttpFilter> HttpFilter<EHF> for PassthroughHttpFilter {}
