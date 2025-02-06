# Dynamic Modules Examples

> Envoy Version: [78efd97]

This repository hosts examples of dynamic modules for [Envoy] to extend its functionality.
The high level documentation is available [here][High Level Doc]. In short, a dynamic module is a shared library
that can be loaded into Envoy at runtime to add custom functionality, for example, a new HTTP filter.

It is a new way to extend Envoy without the need to recompile it just like the existing mechanisms
like Lua filters, Wasm filters, or External Processors.

Currently, the only language supported is Rust, so this repository contains examples of dynamic modules written in Rust.
Future examples will be added in other languages once the support is available.

## Build


To build and test the modules locally without Envoy, you can use `cargo` to build them.

```
cargo build --manifest-path rust/Cargo.toml
cargo test --manifest-path rust/Cargo.toml
```

To build the example modules and bundle them with Envoy, simply run

```
docker buildx build . -t envoy-with-dynamic-modules:latest [--platform linux/amd64,linux/arm64]
```

where `--platform` is optional and can be used to build for multiple platforms.

[78efd97]: https://github.com/envoyproxy/envoy/tree/78efd97
[Envoy]: https://github.com/envoyproxy/envoy
[High Level Doc]: https://www.envoyproxy.io/docs/envoy/latest/intro/arch_overview/advanced/dynamic_modules
