use std::time::Instant;

use envoy_proxy_dynamic_modules_rust_sdk::*;

/// This implements the [`envoy_proxy_dynamic_modules_rust_sdk::HttpFilterConfig`] trait.
///
/// The trait corresponds to a Envoy filter chain configuration.
pub struct FilterConfig {
    _filter_config: String,
    route_latency: EnvoyHistogramVecId,
}

impl FilterConfig {
    /// This is the constructor for the [`FilterConfig`].
    ///
    /// filter_config is the filter config from the Envoy config here:
    /// https://www.envoyproxy.io/docs/envoy/latest/api-v3/extensions/dynamic_modules/v3/dynamic_modules.proto#envoy-v3-api-msg-extensions-dynamic-modules-v3-dynamicmoduleconfig
    pub fn new<EC: EnvoyHttpFilterConfig>(
        filter_config: &str,
        envoy_filter_config: &mut EC,
    ) -> Self {
        Self {
            _filter_config: filter_config.to_string(),
            // Handles to metrics such as counters, gauges, and histograms are allocated at filter config creation time. These handles
            // are opaque ids that can be used to record statistics during the lifecycle of the filter. These handles last until the
            // filter config is destroyed.
            route_latency: envoy_filter_config
                .define_histogram_vec("route_latency_ms", &["route_name"])
                .unwrap(),
        }
    }
}

impl<EHF: EnvoyHttpFilter> HttpFilterConfig<EHF> for FilterConfig {
    /// This is called for each new HTTP filter.
    fn new_http_filter(&mut self, _envoy: &mut EHF) -> Box<dyn HttpFilter<EHF>> {
        Box::new(Filter {
            start_time: None,
            route_name: None,
            route_latency: self.route_latency,
        })
    }
}

/// This implements the [`envoy_proxy_dynamic_modules_rust_sdk::HttpFilter`] trait.
///
/// This is a metrics filter that records per-route metrics of the request.
pub struct Filter {
    start_time: Option<Instant>,
    route_latency: EnvoyHistogramVecId,
    route_name: Option<String>,
}

impl Filter {
    /// This records the latency of the request. Note that it uses the handle to the histogram vector that was allocated at filter config creation time.
    fn record_latency<EHF: EnvoyHttpFilter>(&mut self, envoy_filter: &mut EHF) {
        let Some(start_time) = self.start_time else {
            return;
        };
        let Some(route_name) = self.route_name.take() else {
            return;
        };
        envoy_filter
            .record_histogram_value_vec(
                self.route_latency,
                &[&route_name],
                start_time.elapsed().as_millis() as u64,
            )
            .unwrap();
    }
}

/// This implements the [`envoy_proxy_dynamic_modules_rust_sdk::HttpFilter`] trait.
impl<EHF: EnvoyHttpFilter> HttpFilter<EHF> for Filter {
    fn on_request_headers(
        &mut self,
        envoy_filter: &mut EHF,
        _end_of_stream: bool,
    ) -> abi::envoy_dynamic_module_type_on_http_filter_request_headers_status {
        self.start_time = Some(Instant::now());
        self.route_name = Some(
            String::from_utf8(
                envoy_filter
                    .get_attribute_string(abi::envoy_dynamic_module_type_attribute_id::XdsRouteName)
                    .unwrap_or_default()
                    .as_slice()
                    .to_vec(),
            )
            .unwrap(),
        );
        abi::envoy_dynamic_module_type_on_http_filter_request_headers_status::Continue
    }

    fn on_response_headers(
        &mut self,
        envoy_filter: &mut EHF,
        end_of_stream: bool,
    ) -> abi::envoy_dynamic_module_type_on_http_filter_response_headers_status {
        if end_of_stream {
            self.record_latency(envoy_filter);
        }
        abi::envoy_dynamic_module_type_on_http_filter_response_headers_status::Continue
    }

    fn on_response_body(
        &mut self,
        envoy_filter: &mut EHF,
        end_of_stream: bool,
    ) -> abi::envoy_dynamic_module_type_on_http_filter_response_body_status {
        if end_of_stream {
            self.record_latency(envoy_filter);
        }
        abi::envoy_dynamic_module_type_on_http_filter_response_body_status::Continue
    }

    fn on_request_trailers(
        &mut self,
        envoy_filter: &mut EHF,
    ) -> abi::envoy_dynamic_module_type_on_http_filter_request_trailers_status {
        self.record_latency(envoy_filter);
        abi::envoy_dynamic_module_type_on_http_filter_request_trailers_status::Continue
    }
}
