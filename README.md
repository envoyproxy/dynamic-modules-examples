# Dynamic Modules Examples

> Envoy Version: v1.37.0
>
> Since dynamic modules are tied with a specific Envoy version, this repository is based on the specific commit of Envoy.
> For examples for a specific Envoy version, please check out `release/v<version>` branches:
> * [`release/v1.34`](https://github.com/envoyproxy/dynamic-modules-examples/tree/release/v1.34)
> * [`release/v1.35`](https://github.com/envoyproxy/dynamic-modules-examples/tree/release/v1.35)
> * [`release/v1.36`](https://github.com/envoyproxy/dynamic-modules-examples/tree/release/v1.36)

This repository hosts examples of dynamic modules for [Envoy] to extend its functionality.
The high level documentation is available [here][High Level Doc]. In short, a dynamic module is a shared library
that can be loaded into Envoy at runtime to add custom functionality, for example, a new HTTP filter.

It is a new way to extend Envoy without the need to recompile it just like the existing mechanisms
like Lua filters, Wasm filters, or External Processors.

As of writing, the only official language supported by Envoy is Rust. However, the dynamic module's interface is defined in a plain
C header file, so technically you can implement a dynamic module in any language that can build shared libraries, such as C, C++, Go, Zig, etc.
Currently, this repository hosts two language implementations of dynamic modules: Rust and Go.
* [`rust`](rust): using the official Rust dynamic module SDK.
* [`go`](go): using the experimental Go dynamic module SDK implemented here. WARNING: This is not an official SDK and is not
  supported by Envoy main respository. See [issue#25](https://github.com/envoyproxy/dynamic-modules-examples/issues/25) for more details.

This repository serves as a reference for developers who want to create their own dynamic modules for Envoy including
how to setup the project, how to build it, and how to test it, etc.

The tracking issue for dynamic modules in general is [here](https://github.com/envoyproxy/envoy/issues/38392) where you can find more information about the current status and future plans as well as feature requests.

## Development

```
# Run all unit tests
make test
# Build all dynamic modules
make build
# Run integration tests with Envoy
make integration-test
```

[Envoy]: https://github.com/envoyproxy/envoy
[High Level Doc]: https://www.envoyproxy.io/docs/envoy/latest/intro/arch_overview/advanced/dynamic_modules
