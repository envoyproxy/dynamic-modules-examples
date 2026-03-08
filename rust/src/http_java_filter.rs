//! Java HTTP filter — embeds a JVM to execute user-provided Java filter logic.
//!
//! Users write their filter in Java by implementing the
//! `io.envoyproxy.dynamicmodules.EnvoyHttpFilter` interface, compile it to a JAR,
//! and point to it from the Envoy config.
//!
//! # Envoy filter config (JSON)
//!
//! ```json
//! {
//!   "jar_path": "/path/to/your-filter.jar",
//!   "class_name": "io.envoyproxy.dynamicmodules.ExampleFilter"
//! }
//! ```
//!
//! # Java interface contract
//!
//! ```java
//! public class MyFilter implements EnvoyHttpFilter {
//!     public HeaderMutation onRequestHeaders(String[] names, String[] values) {
//!         HeaderMutation m = new HeaderMutation();
//!         m.addHeaders = new String[]{"x-java-filter", "active"};
//!         return m;
//!     }
//!     public HeaderMutation onResponseHeaders(String[] names, String[] values) {
//!         return null; // no changes
//!     }
//! }
//! ```
//!
//! # Build requirements
//!
//! Java (JDK 11+) must be installed both at compile time (for the `jni` invocation
//! feature) and at runtime (Envoy needs `libjvm.so` in `LD_LIBRARY_PATH`).
//!
//! To compile the Java sources and build the example JAR:
//!
//! ```sh
//! make -C java
//! ```

use envoy_proxy_dynamic_modules_rust_sdk::*;
use jni::{
    objects::{GlobalRef, JObject, JObjectArray, JString, JValue},
    sys::jsize,
    InitArgsBuilder, JNIVersion, JavaVM,
};
use serde::Deserialize;
use std::sync::OnceLock;

// ---------------------------------------------------------------------------
// Global JVM — there can only be one per process.
// ---------------------------------------------------------------------------

static JVM: OnceLock<JavaVM> = OnceLock::new();

fn jvm() -> Option<&'static JavaVM> {
    JVM.get()
}

// ---------------------------------------------------------------------------
// Config (parsed from Envoy filter JSON config)
// ---------------------------------------------------------------------------

#[derive(Deserialize, Debug)]
struct JavaFilterConfigData {
    /// Colon-separated classpath entries (JAR files, directories).
    /// The first `java_filter` config to be loaded wins for JVM initialisation.
    jar_path: String,
    /// Fully-qualified Java class name, e.g. `"io.envoyproxy.dynamicmodules.ExampleFilter"`.
    class_name: String,
}

// ---------------------------------------------------------------------------
// FilterConfig — created once per filter chain config block
// ---------------------------------------------------------------------------

/// Holds a global reference to the Java filter *instance* that is shared
/// across all requests for this filter chain.
pub struct FilterConfig {
    filter_instance: GlobalRef,
}

impl FilterConfig {
    pub fn new(filter_config: &str) -> Option<Self> {
        let data: JavaFilterConfigData = match serde_json::from_str(filter_config) {
            Ok(d) => d,
            Err(e) => {
                eprintln!("[java_filter] Failed to parse config: {e}");
                return None;
            }
        };

        // Initialise the JVM exactly once.  The classpath is fixed at init time;
        // if multiple java_filter configs are loaded, all needed JARs must be
        // in the *first* config's jar_path (use ':' as separator on Linux).
        let jvm = JVM.get_or_init(|| {
            let classpath_opt = format!("-Djava.class.path={}", data.jar_path);
            let args = InitArgsBuilder::new()
                .version(JNIVersion::V8)
                .option(&classpath_opt)
                .build()
                .expect("[java_filter] Failed to build JVM args");
            JavaVM::new(args).expect("[java_filter] Failed to create JVM")
        });

        let mut env = match jvm.attach_current_thread_permanently() {
            Ok(e) => e,
            Err(e) => {
                eprintln!("[java_filter] Failed to attach thread: {e}");
                return None;
            }
        };

        let class_jni = data.class_name.replace('.', "/");
        let filter_class = match env.find_class(&class_jni) {
            Ok(c) => c,
            Err(e) => {
                eprintln!(
                    "[java_filter] Class not found '{}': {e}",
                    data.class_name
                );
                return None;
            }
        };

        let filter_obj = match env.new_object(&filter_class, "()V", &[]) {
            Ok(o) => o,
            Err(e) => {
                eprintln!(
                    "[java_filter] Failed to instantiate '{}': {e}",
                    data.class_name
                );
                return None;
            }
        };

        let filter_instance = env
            .new_global_ref(filter_obj)
            .expect("[java_filter] Failed to create global ref");

        Some(FilterConfig { filter_instance })
    }
}

impl<EHF: EnvoyHttpFilter> HttpFilterConfig<EHF> for FilterConfig {
    fn new_http_filter(&self, _envoy: &mut EHF) -> Box<dyn HttpFilter<EHF>> {
        Box::new(Filter {
            filter_instance: self.filter_instance.clone(),
        })
    }
}

// ---------------------------------------------------------------------------
// Filter — per-request state
// ---------------------------------------------------------------------------

pub struct Filter {
    filter_instance: GlobalRef,
}

impl<EHF: EnvoyHttpFilter> HttpFilter<EHF> for Filter {
    fn on_request_headers(
        &mut self,
        envoy_filter: &mut EHF,
        _end_of_stream: bool,
    ) -> abi::envoy_dynamic_module_type_on_http_filter_request_headers_status {
        let headers = collect_headers(envoy_filter.get_request_headers());
        let mutation = call_java_on_headers(&self.filter_instance, "onRequestHeaders", &headers);
        for (k, v) in &mutation.add {
            envoy_filter.set_request_header(k, v.as_bytes());
        }
        for k in &mutation.remove {
            envoy_filter.remove_request_header(k);
        }
        if mutation.stop_iteration {
            abi::envoy_dynamic_module_type_on_http_filter_request_headers_status::StopIteration
        } else {
            abi::envoy_dynamic_module_type_on_http_filter_request_headers_status::Continue
        }
    }

    fn on_response_headers(
        &mut self,
        envoy_filter: &mut EHF,
        _end_of_stream: bool,
    ) -> abi::envoy_dynamic_module_type_on_http_filter_response_headers_status {
        let headers = collect_headers(envoy_filter.get_response_headers());
        let mutation = call_java_on_headers(&self.filter_instance, "onResponseHeaders", &headers);
        for (k, v) in &mutation.add {
            envoy_filter.set_response_header(k, v.as_bytes());
        }
        for k in &mutation.remove {
            envoy_filter.remove_response_header(k);
        }
        if mutation.stop_iteration {
            abi::envoy_dynamic_module_type_on_http_filter_response_headers_status::StopIteration
        } else {
            abi::envoy_dynamic_module_type_on_http_filter_response_headers_status::Continue
        }
    }
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Mutations returned by the Java filter (owned Rust data, no JNI lifetime).
#[derive(Default)]
struct JavaMutation {
    stop_iteration: bool,
    /// Headers to add: Vec of (name, value) pairs.
    add: Vec<(String, String)>,
    /// Header names to remove.
    remove: Vec<String>,
}

/// Collect `(EnvoyBuffer, EnvoyBuffer)` pairs into owned `(String, String)`.
fn collect_headers(raw: Vec<(EnvoyBuffer, EnvoyBuffer)>) -> Vec<(String, String)> {
    raw.into_iter()
        .filter_map(|(k, v)| {
            let k = std::str::from_utf8(k.as_slice()).ok()?.to_owned();
            let v = std::str::from_utf8(v.as_slice()).ok()?.to_owned();
            Some((k, v))
        })
        .collect()
}

/// Calls `<methodName>(String[] names, String[] values) → HeaderMutation` on
/// the Java filter instance and returns the parsed mutations as owned Rust data.
fn call_java_on_headers(
    filter_instance: &GlobalRef,
    method_name: &str,
    headers: &[(String, String)],
) -> JavaMutation {
    let Some(jvm) = jvm() else {
        eprintln!("[java_filter] JVM not initialised");
        return JavaMutation::default();
    };

    // attach_current_thread_permanently is idempotent — safe to call every time.
    let mut env = match jvm.attach_current_thread_permanently() {
        Ok(e) => e,
        Err(e) => {
            eprintln!("[java_filter] Failed to attach thread: {e}");
            return JavaMutation::default();
        }
    };

    // Build parallel String[] arrays for names and values.
    let len = headers.len() as jsize;
    let string_class = match env.find_class("java/lang/String") {
        Ok(c) => c,
        Err(e) => {
            eprintln!("[java_filter] Cannot find String class: {e}");
            return JavaMutation::default();
        }
    };

    let names_arr = match env.new_object_array(len, &string_class, JObject::null()) {
        Ok(a) => a,
        Err(e) => {
            eprintln!("[java_filter] Cannot create names array: {e}");
            return JavaMutation::default();
        }
    };
    let values_arr = match env.new_object_array(len, &string_class, JObject::null()) {
        Ok(a) => a,
        Err(e) => {
            eprintln!("[java_filter] Cannot create values array: {e}");
            return JavaMutation::default();
        }
    };

    for (i, (name, value)) in headers.iter().enumerate() {
        let js_name = match env.new_string(name) {
            Ok(s) => s,
            Err(_) => continue,
        };
        let js_value = match env.new_string(value) {
            Ok(s) => s,
            Err(_) => continue,
        };
        let _ = env.set_object_array_element(&names_arr, i as jsize, js_name);
        let _ = env.set_object_array_element(&values_arr, i as jsize, js_value);
    }

    // Call onRequestHeaders / onResponseHeaders.
    let sig = "([Ljava/lang/String;[Ljava/lang/String;)Lio/envoyproxy/dynamicmodules/HeaderMutation;";
    let result = match env.call_method(
        filter_instance,
        method_name,
        sig,
        &[JValue::Object(&names_arr), JValue::Object(&values_arr)],
    ) {
        Ok(r) => r,
        Err(e) => {
            eprintln!("[java_filter] JNI call to {method_name} failed: {e}");
            let _ = env.exception_clear();
            return JavaMutation::default();
        }
    };

    let mutation_obj = match result.l() {
        Ok(obj) if !obj.is_null() => obj,
        _ => return JavaMutation::default(), // null → no mutations, continue
    };

    let mut out = JavaMutation::default();

    // --- stopIteration (boolean field) ---
    out.stop_iteration = env
        .get_field(&mutation_obj, "stopIteration", "Z")
        .ok()
        .and_then(|v| v.z().ok())
        .unwrap_or(false);

    // --- addHeaders: String[] of alternating [name, value, …] ---
    if let Ok(field) = env.get_field(&mutation_obj, "addHeaders", "[Ljava/lang/String;") {
        if let Ok(obj) = field.l() {
            if !obj.is_null() {
                let arr = JObjectArray::from(obj);
                let arr_len = env.get_array_length(&arr).unwrap_or(0);
                let mut i = 0;
                while i + 1 < arr_len {
                    if let (Ok(name_obj), Ok(val_obj)) = (
                        env.get_object_array_element(&arr, i),
                        env.get_object_array_element(&arr, i + 1),
                    ) {
                        let name: String = env
                            .get_string(&JString::from(name_obj))
                            .map(|s| s.into())
                            .unwrap_or_default();
                        let value: String = env
                            .get_string(&JString::from(val_obj))
                            .map(|s| s.into())
                            .unwrap_or_default();
                        if !name.is_empty() {
                            out.add.push((name, value));
                        }
                    }
                    i += 2;
                }
            }
        }
    }

    // --- removeHeaders: String[] ---
    if let Ok(field) = env.get_field(&mutation_obj, "removeHeaders", "[Ljava/lang/String;") {
        if let Ok(obj) = field.l() {
            if !obj.is_null() {
                let arr = JObjectArray::from(obj);
                let arr_len = env.get_array_length(&arr).unwrap_or(0);
                for i in 0..arr_len {
                    if let Ok(name_obj) = env.get_object_array_element(&arr, i) {
                        let name: String = env
                            .get_string(&JString::from(name_obj))
                            .map(|s| s.into())
                            .unwrap_or_default();
                        if !name.is_empty() {
                            out.remove.push(name);
                        }
                    }
                }
            }
        }
    }

    out
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    // Unit tests for the Java filter require a JVM and the example JAR to be
    // present.  Run `make -C java` first, then `cargo test -- java_filter`.

    #[test]
    fn test_collect_headers() {
        let raw = vec![
            (EnvoyBuffer::new("host"), EnvoyBuffer::new("example.com")),
            (EnvoyBuffer::new(":path"), EnvoyBuffer::new("/api")),
        ];
        let headers = collect_headers(raw);
        assert_eq!(headers.len(), 2);
        assert_eq!(headers[0], ("host".to_string(), "example.com".to_string()));
        assert_eq!(headers[1], (":path".to_string(), "/api".to_string()));
    }

    #[test]
    fn test_collect_headers_with_mock() {
        let mut envoy_filter = envoy_proxy_dynamic_modules_rust_sdk::MockEnvoyHttpFilter::new();
        envoy_filter
            .expect_get_request_headers()
            .returning(|| vec![(EnvoyBuffer::new("host"), EnvoyBuffer::new("example.com"))]);

        let headers = collect_headers(envoy_filter.get_request_headers());
        assert_eq!(headers.len(), 1);
        assert_eq!(headers[0], ("host".to_string(), "example.com".to_string()));
    }
}
