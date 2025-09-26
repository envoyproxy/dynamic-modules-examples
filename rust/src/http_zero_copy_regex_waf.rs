use envoy_proxy_dynamic_modules_rust_sdk::*;
use matchers::Pattern;

/// This implements the [`envoy_proxy_dynamic_modules_rust_sdk::HttpFilterConfig`] trait.
///
/// The trait corresponds to a Envoy filter chain configuration.
///
/// This filter checks the request body against a regex and returns a 403 response if it matches without
/// copying the body from the Envoy buffers.
pub struct FilterConfig {
    re: Pattern,
}

impl FilterConfig {
    /// This is the constructor for the [`FilterConfig`].
    ///
    /// filter_config is the filter config from the Envoy config here:
    /// https://www.envoyproxy.io/docs/envoy/latest/api-v3/extensions/dynamic_modules/v3/dynamic_modules.proto#envoy-v3-api-msg-extensions-dynamic-modules-v3-dynamicmoduleconfig
    pub fn new(filter_config: &str) -> Option<Self> {
        let re = match Pattern::new(filter_config) {
            Ok(cfg) => cfg,
            Err(err) => {
                eprintln!("Error parsing filter config: {err}");
                return None;
            }
        };
        Some(Self { re })
    }
}

impl<EHF: EnvoyHttpFilter> HttpFilterConfig<EHF> for FilterConfig {
    /// This is called for each new HTTP filter.
    fn new_http_filter(&mut self, _envoy: &mut EHF) -> Box<dyn HttpFilter<EHF>> {
        Box::new(Filter {
            re: self.re.clone(),
        })
    }
}

/// This implements the [`envoy_proxy_dynamic_modules_rust_sdk::HttpFilter`] trait.
///
/// This checks the request body against a regex and returns a 403 response if it matches.
pub struct Filter {
    /// The regex to match against the request body.
    re: Pattern,
}

/// This implements the [`envoy_proxy_dynamic_modules_rust_sdk::HttpFilter`] trait.
impl<EHF: EnvoyHttpFilter> HttpFilter<EHF> for Filter {
    fn on_request_body(
        &mut self,
        envoy_filter: &mut EHF,
        end_of_stream: bool,
    ) -> abi::envoy_dynamic_module_type_on_http_filter_request_body_status {
        // Until we have the entire body, we buffer all chunks.
        if !end_of_stream {
            return abi::envoy_dynamic_module_type_on_http_filter_request_body_status::StopIterationAndBuffer;
        }

        // Get the body from the Envoy filter. The returned data is a vector of
        // [`EnvoyMutBuffer`]s. Each [`EnvoyMutBuffer`] is a mutable buffer that can be
        // used to read the body data.
        let data = envoy_filter
            .get_request_body()
            .expect("Failed to get request body");
        let mut body_reader = BodyReader::new(data);
        let matched = self
            .re
            .read_matches(&mut body_reader)
            .expect("Failed to do regex match");
        if matched {
            // If the regex matches, we send a 403 response.
            envoy_filter.send_response(403, vec![], Some(b"Access forbidden"));
            return abi::envoy_dynamic_module_type_on_http_filter_request_body_status::StopIterationNoBuffer;
        }
        abi::envoy_dynamic_module_type_on_http_filter_request_body_status::Continue
    }
}

/// This implements the [`std::io::Read`] trait for the Envoy request body.
///
/// This allows us to read the body data from the Envoy buffers as a single stream and
/// pass it to the regex matcher without copying the data.
struct BodyReader<'a> {
    data: Vec<EnvoyMutBuffer<'a>>,
    vec_idx: usize,
    buf_idx: usize,
}

impl<'a> BodyReader<'a> {
    fn new(data: Vec<EnvoyMutBuffer<'a>>) -> Self {
        Self {
            data,
            vec_idx: 0,
            buf_idx: 0,
        }
    }
}

impl std::io::Read for BodyReader<'_> {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        if self.vec_idx >= self.data.len() {
            return Ok(0);
        }
        let mut n = 0;
        while n < buf.len() && self.vec_idx < self.data.len() {
            let slice = self.data[self.vec_idx].as_slice();
            let remaining = slice.len() - self.buf_idx;
            let to_copy = std::cmp::min(remaining, buf.len() - n);
            buf[n..n + to_copy].copy_from_slice(&slice[self.buf_idx..self.buf_idx + to_copy]);
            n += to_copy;
            self.buf_idx += to_copy;
            if self.buf_idx >= slice.len() {
                self.vec_idx += 1;
                self.buf_idx = 0;
            }
        }
        Ok(n)
    }
}

#[cfg(test)]
mod tests {
    use std::f64::consts::E;

    use envoy_proxy_dynamic_modules_rust_sdk::abi::envoy_dynamic_module_type_metrics_result;

    use super::*;

    #[test]
    /// This demonstrates how to write a test without Envoy using a mock provided by the SDK.
    fn test_filter() {
        let mut filter_config = FilterConfig::new("Hello [Ww].+").unwrap();
        let mut envoy_filter = MockEnvoyHttpFilter::new();
        let mut filter: Box<dyn HttpFilter<MockEnvoyHttpFilter>> =
            filter_config.new_http_filter(&mut envoy_filter);

        // Not end of stream, so we should buffer the request body.
        assert_eq!(filter.on_request_body(&mut envoy_filter, false),  abi::envoy_dynamic_module_type_on_http_filter_request_body_status::StopIterationAndBuffer);

        // End of stream and matching regex, so we should send a 403 response.
        envoy_filter
            .expect_get_request_body()
            .returning(|| {
                static mut HELLO: [u8; 6] = *b"Hello ";
                static mut WORLD: [u8; 6] = *b"World!";
                Some(vec![
                    EnvoyMutBuffer::new(unsafe { &mut HELLO }),
                    EnvoyMutBuffer::new(unsafe { &mut WORLD }),
                ])
            })
            .times(1);
        envoy_filter
            .expect_send_response()
            .withf(|status, _, _| *status == 403)
            .returning(|_, _, _| {})
            .times(1);
        assert_eq!(filter.on_request_body(&mut envoy_filter, true), abi::envoy_dynamic_module_type_on_http_filter_request_body_status::StopIterationNoBuffer);

        // End of stream and not matching regex, so we should continue.
        envoy_filter
            .expect_get_request_body()
            .returning(|| {
                static mut GOOD: [u8; 5] = *b"Good ";
                static mut MORNING: [u8; 8] = *b"Morning!";
                Some(vec![
                    EnvoyMutBuffer::new(unsafe { &mut GOOD }),
                    EnvoyMutBuffer::new(unsafe { &mut MORNING }),
                ])
            })
            .times(1);

        envoy_filter.expect_send_response().never();

        assert_eq!(
            filter.on_request_body(&mut envoy_filter, true),
            abi::envoy_dynamic_module_type_on_http_filter_request_body_status::Continue
        );
    }
}
