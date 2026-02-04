fn main() {
    let mut config = prost_build::Config::new();
    config.type_attribute(".", "#[derive(serde::Deserialize, serde::Serialize)]");
    config.field_attribute(".", "#[serde(default)]");
    config
        .compile_protos(
            &["src/dns_gateway/dns_gateway.proto"],
            &["src/dns_gateway/"],
        )
        .unwrap();
}
