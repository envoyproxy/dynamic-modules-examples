use envoy_proxy_dynamic_modules_rust_sdk::*;
use serde::{Deserialize, Serialize};
use std::io::Write;
use std::sync::{mpsc, Arc, Mutex};
use std::thread;

/// This implements the [`envoy_proxy_dynamic_modules_rust_sdk::HttpFilterConfig`] trait.
///
/// The trait corresponds to a Envoy filter chain configuration.
///
/// This logger spawns a number of worker threads that write the log messages to files concurrently.
/// Each worker thread writes to a separate file. The file names are access_log_0.log, access_log_1.log, etc.
///
/// Each filter sends the log message to the worker threads via a channel. The worker threads write the log messages to the files.
///
/// The number of worker threads is configurable via the `num_workers` field in the filter config.
/// The directory to write the log files to is configurable via the `dirname` field in the filter config.
pub struct FilterConfig {
    tx: mpsc::Sender<String>,
}

/// This will be parsed from filter_config passed to the constructor coming from Envoy config.
#[derive(Serialize, Deserialize, Debug)]
struct FilterConfigData {
    // The dirname to write the log file to.
    dirname: String,
    // The number of workers to spawn.
    num_workers: usize,
}

impl FilterConfig {
    /// This is the constructor for the [`FilterConfig`].
    ///
    /// filter_config is the filter config from the Envoy config here:
    /// https://www.envoyproxy.io/docs/envoy/latest/api-v3/extensions/dynamic_modules/v3/dynamic_modules.proto#envoy-v3-api-msg-extensions-dynamic-modules-v3-dynamicmoduleconfig
    pub fn new(filter_config: &[u8]) -> Option<Self> {
        let filter_config: FilterConfigData = match serde_json::from_slice(filter_config) {
            Ok(cfg) => cfg,
            Err(err) => {
                eprintln!("Error parsing filter config: {}", err);
                return None;
            }
        };
        let (tx, rx) = mpsc::channel::<String>();
        let config = Self { tx };
        let rx = Arc::new(Mutex::new(rx));
        for i in 0..filter_config.num_workers {
            let rx = Arc::clone(&rx);
            let file_path = format!("{}/access_log_{}.log", filter_config.dirname, i);
            let mut file = match std::fs::File::create(file_path) {
                Ok(file) => file,
                Err(err) => {
                    eprintln!("Error creating log file: {}", err);
                    return None;
                }
            };
            thread::spawn(move || {
                loop {
                    let message = {
                        let rx_lock = rx.lock().unwrap();
                        rx_lock.recv()
                    };
                    match message {
                        Ok(msg) => match writeln!(file, "{}", msg) {
                            Ok(_) => {}
                            Err(err) => eprintln!("Error writing to log file: {}", err),
                        },
                        // When the channel is closed, exit the loop.
                        Err(_) => break,
                    }
                }
            });
        }
        Some(config)
    }
}

impl<EC: EnvoyHttpFilterConfig, EHF: EnvoyHttpFilter> HttpFilterConfig<EC, EHF> for FilterConfig {
    /// This is called for each new HTTP filter.
    fn new_http_filter(&mut self, _envoy: &mut EC) -> Box<dyn HttpFilter<EHF>> {
        let tx = self.tx.clone();
        Box::new(Filter {
            tx,
            request_headers: Vec::new(),
            response_headers: Vec::new(),
        })
    }
}

/// This implements the [`envoy_proxy_dynamic_modules_rust_sdk::HttpFilter`] trait.
pub struct Filter {
    tx: mpsc::Sender<String>,
    request_headers: Vec<String>,
    response_headers: Vec<String>,
}

/// This implements the [`envoy_proxy_dynamic_modules_rust_sdk::HttpFilter`] trait.
///
/// Default implementation of all methods is to return `Continue`.
impl<EHF: EnvoyHttpFilter> HttpFilter<EHF> for Filter {
    fn on_request_headers(
        &mut self,
        envoy_filter: &mut EHF,
        _end_of_stream: bool,
    ) -> abi::envoy_dynamic_module_type_on_http_filter_request_headers_status {
        for (key, value) in envoy_filter.get_request_headers() {
            let Some(key) = std::str::from_utf8(key.as_slice()).ok() else {
                continue;
            };
            let Some(value) = std::str::from_utf8(value.as_slice()).ok() else {
                continue;
            };
            self.request_headers.push(format!("{}: {}", key, value));
        }
        abi::envoy_dynamic_module_type_on_http_filter_request_headers_status::Continue
    }

    fn on_response_headers(
        &mut self,
        envoy_filter: &mut EHF,
        _end_of_stream: bool,
    ) -> abi::envoy_dynamic_module_type_on_http_filter_response_headers_status {
        for (key, value) in envoy_filter.get_response_headers() {
            let Some(key) = std::str::from_utf8(key.as_slice()).ok() else {
                continue;
            };
            let Some(value) = std::str::from_utf8(value.as_slice()).ok() else {
                continue;
            };
            self.response_headers.push(format!("{}: {}", key, value));
        }
        abi::envoy_dynamic_module_type_on_http_filter_response_headers_status::Continue
    }
}

#[derive(Serialize, Deserialize, Debug)]
struct LogMessage {
    request_headers: Vec<String>,
    response_headers: Vec<String>,
}

impl Drop for Filter {
    fn drop(&mut self) {
        let message = serde_json::to_string(&LogMessage {
            request_headers: self.request_headers.clone(),
            response_headers: self.response_headers.clone(),
        })
        .unwrap();
        let err = self.tx.send(message);
        if let Err(err) = err {
            eprintln!("Error sending log message: {}", err);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_filter_config() {
        let tmpdir = tempfile::tempdir().unwrap();
        let filter_config = format!(
            r#"{{"dirname": "{}", "num_workers": 1}}"#,
            tmpdir.path().display()
        )
        .into_bytes();
        let config = FilterConfig::new(&filter_config).unwrap();
        config.tx.clone().send("foo".to_string()).unwrap();

        // Sleep for a bit to let the worker thread write the log.
        std::thread::sleep(std::time::Duration::from_secs(1));
        let log_file = tmpdir.path().join("access_log_0.log");
        let log_contents = std::fs::read_to_string(log_file).unwrap();
        assert_eq!(log_contents, "foo\n");
        tmpdir.close().unwrap();
    }

    #[test]
    fn test_filter() {
        let (tx, rx) = mpsc::channel::<String>();
        let mut access_logger_filter = Filter {
            tx,
            request_headers: Vec::new(),
            response_headers: Vec::new(),
        };
        let mut envoy_filter = envoy_proxy_dynamic_modules_rust_sdk::MockEnvoyHttpFilter::new();
        envoy_filter
            .expect_get_request_headers()
            .returning(|| vec![(EnvoyBuffer::new("host"), EnvoyBuffer::new("example.com"))]);
        envoy_filter
            .expect_get_response_headers()
            .returning(|| vec![(EnvoyBuffer::new("content-length"), EnvoyBuffer::new("123"))]);
        access_logger_filter.on_request_headers(&mut envoy_filter, false);
        access_logger_filter.on_response_headers(&mut envoy_filter, false);

        // Check the headers are stored correctly.
        assert_eq!(
            access_logger_filter.request_headers,
            vec!["host: example.com"]
        );
        assert_eq!(
            access_logger_filter.response_headers,
            vec!["content-length: 123"]
        );

        // Drop the filter to trigger the log message.
        drop(access_logger_filter);

        // Check the log message is sent correctly.
        let log_message = rx.recv().unwrap();
        let log_message: LogMessage = serde_json::from_str(&log_message).unwrap();
        assert_eq!(log_message.request_headers, vec!["host: example.com"]);
        assert_eq!(log_message.response_headers, vec!["content-length: 123"]);
    }
}
