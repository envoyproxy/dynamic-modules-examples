# Dynamic Modules Examples

> Envoy Version: [4a113b5118003682833ba612202eb68628861ac6]

This repository hosts examples of dynamic modules for [Envoy] to extend its functionality.
The high level documentation is available [here][High Level Doc]. In short, a dynamic module is a shared library
that can be loaded into Envoy at runtime to add custom functionality, for example, a new HTTP filter.

It is a new way to extend Envoy without the need to recompile it just like the existing mechanisms
like Lua filters, Wasm filters, or External Processors.

Currently, the only language supported is Rust, so this repository contains examples of dynamic modules written in Rust.
Future examples will be added in other languages once the support is available.

This repository serves as a reference for developers who want to create their own dynamic modules for Envoy including
how to setup the project, how to build it, and how to test it, etc.

## Development

### Rust Dynamic Module

To build and test the modules locally without Envoy, you can use `cargo` to build them just like any other Rust project:

```
cd rust
cargo build
cargo test
cargo clippy -- -D warnings
cargo fmt --all -- --check
```

### Build Envoy + Rust Dynamic Module Docker Image

To build the example modules and bundle them with Envoy, simply run

```
docker buildx build . -t envoy-with-dynamic-modules:latest [--platform linux/amd64,linux/arm64]
```

where `--platform` is optional and can be used to build for multiple platforms.

### Run Envoy + Rust Dynamic Module Docker Image

The example Envoy configuration yaml is in [`integration/envoy.yaml`](integration/envoy.yaml) which is also used
to run the integration tests. Assuming you built the Docker image with the tag `envoy-with-dynamic-modules:latest`, you can run Envoy with the following command:

```
docker run --network host -v $(pwd):/examples -w /examples/integration envoy-with-dynamic-modules:latest --config-path ./envoy.yaml
```

Then execute, for example, the following command to test the passthrough and access log filters:

```
curl localhost:1062/uuid
```


## Update Envoy Version

To update the Envoy version used in this repository, execute the following command:

```
CURRENT_VERSION="$(cat ENVOY_VERSION)"
NEW_VERSION=4a113b5118003682833ba612202eb68628861ac6 # Whatever the commit in envoyproxy/envoy repo.
grep -rlF "${CURRENT_VERSION}" . | xargs sed -i "s/${CURRENT_VERSION}/${NEW_VERSION}/g"
```

[4a113b5118003682833ba612202eb68628861ac6]: https://github.com/envoyproxy/envoy/tree/4a113b5118003682833ba612202eb68628861ac6
[Envoy]: https://github.com/envoyproxy/envoy
[High Level Doc]: https://www.envoyproxy.io/docs/envoy/latest/intro/arch_overview/advanced/dynamic_modules
