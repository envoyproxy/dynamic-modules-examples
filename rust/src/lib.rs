use envoy_proxy_dynamic_modules_rust_sdk::*;

mod passthrough;
use passthrough::*;

declare_init_functions!(init, new_http_filter_config_fn);

/// This implements the [`envoy_proxy_dynamic_modules_rust_sdk::ProgramInitFunction`].
///
/// This is called exactly once when the module is loaded.
fn init() -> bool {
    true
}

/// This implements the [`envoy_proxy_dynamic_modules_rust_sdk::NewHttpFilterConfigFunction`].
///
/// This is the entrypoint every time a new HTTP filter is created via the DynamicModuleFilter config.
///
/// Each argument matches the corresponding argument in the Envoy config here:
/// https://www.envoyproxy.io/docs/envoy/latest/api-v3/extensions/dynamic_modules/v3/dynamic_modules.proto#envoy-v3-api-msg-extensions-dynamic-modules-v3-dynamicmoduleconfig
fn new_http_filter_config_fn<EC: EnvoyHttpFilterConfig, EHF: EnvoyHttpFilter>(
    _envoy_filter_config: &mut EC,
    filter_name: &str,
    _filter_config: &str,
) -> Option<Box<dyn HttpFilterConfig<EC, EHF>>> {
    match filter_name {
        "passthrough" => Some(Box::new(PassthroughHttpFilterConfig {})),
        _ => panic!("Unknown filter name: {}", filter_name),
    }
}
