use envoy_proxy_dynamic_modules_rust_sdk::*;

mod http_access_logger;
mod http_header_mutation;
mod http_passthrough;
mod http_random_auth;
mod http_zero_copy_regex_waf;

declare_init_functions!(init, new_http_filter_config_fn);

/// This implements the [`envoy_proxy_dynamic_modules_rust_sdk::ProgramInitFunction`].
///
/// This is called exactly once when the module is loaded. It can be used to
/// initialize global state as well as check the runtime environment to ensure that
/// the module is running in a supported environment.
///
/// Returning `false` will cause Envoy to reject the config hence the
/// filter will not be loaded.
fn init() -> bool {
    true
}

/// This implements the [`envoy_proxy_dynamic_modules_rust_sdk::NewHttpFilterConfigFunction`].
///
/// This is the entrypoint every time a new HTTP filter is created via the DynamicModuleFilter config.
///
/// Each argument matches the corresponding argument in the Envoy config here:
/// https://www.envoyproxy.io/docs/envoy/latest/api-v3/extensions/dynamic_modules/v3/dynamic_modules.proto#envoy-v3-api-msg-extensions-dynamic-modules-v3-dynamicmoduleconfig
///
/// Returns None if the filter name or config is determined to be invalid by each filter's `new` function.
fn new_http_filter_config_fn<EC: EnvoyHttpFilterConfig, EHF: EnvoyHttpFilter>(
    _envoy_filter_config: &mut EC,
    filter_name: &str,
    filter_config: &[u8],
) -> Option<Box<dyn HttpFilterConfig<EHF>>> {
    let filter_config = std::str::from_utf8(filter_config).unwrap();
    match filter_name {
        "passthrough" => Some(Box::new(http_passthrough::FilterConfig::new(filter_config))),
        "access_logger" => http_access_logger::FilterConfig::new(filter_config)
            .map(|config| Box::new(config) as Box<dyn HttpFilterConfig<EHF>>),
        "random_auth" => Some(Box::new(http_random_auth::FilterConfig::new(filter_config))),
        "zero_copy_regex_waf" => http_zero_copy_regex_waf::FilterConfig::new(filter_config)
            .map(|config| Box::new(config) as Box<dyn HttpFilterConfig<EHF>>),
        "header_mutation" => http_header_mutation::FilterConfig::new(filter_config)
            .map(|config| Box::new(config) as Box<dyn HttpFilterConfig<EHF>>),
        _ => panic!("Unknown filter name: {filter_name}"),
    }
}
