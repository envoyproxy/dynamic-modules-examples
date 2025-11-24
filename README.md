# Dynamic Modules Examples

> Envoy Version: [dc2d3098ae5641555f15c71d5bb5ce0060a8015c] v1.36.2
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

### Rust Dynamic Module

To build and test the modules locally without Envoy, you can use `cargo` to build them just like any other Rust project:

```
cd rust
cargo build
cargo test
cargo clippy -- -D warnings
cargo fmt --all -- --check
```

### Go Dynamic Module
To build and test the modules locally without Envoy, you can use `go` to build them just like any other Go project:

```
cd go
go test ./... -v
go build -buildmode=c-shared -o libgo_module.so .
go tool golangci-lint run
find . -type f -name '*.go' | xargs go tool gofumpt -l -w
```

### Build Envoy + Example Dynamic Module Docker Image

To build the example modules and bundle them with Envoy, simply run

```
docker buildx build . -t envoy-with-dynamic-modules:latest [--platform linux/amd64,linux/arm64]
```

where `--platform` is optional and can be used to build for multiple platforms.

### Run Envoy + Example Dynamic Module Docker Image

The example Envoy configuration yaml is in [`integration/envoy.yaml`](integration/envoy.yaml) which is also used
to run the integration tests. Assuming you built the Docker image with the tag `envoy-with-dynamic-modules:latest`, you can run Envoy with the following command:

```
docker run --network host -v $(pwd):/examples -w /examples/integration envoy-with-dynamic-modules:latest --config-path ./envoy.yaml
```

Then execute, for example, the following command to test the passthrough and access log filters:

```
curl localhost:1062/uuid
```

### Run integration tests with the built example Envoy + Dynamic Module Docker Image.

The integration tests are in the `integration` directory. Assuming you built the Docker image with the tag `envoy-with-dynamic-modules:latest`, you can run the integration tests with the following command:
```
cd integration
go test . -v -count=1
```

If you want to explicitly specify the docker image, use `ENVOY_IMAGE` environment variable:
```
ENVOY_IMAGE=foo-bar-image:latest go test . -v -count=1
```

[dc2d3098ae5641555f15c71d5bb5ce0060a8015c]: https://github.com/envoyproxy/envoy/tree/dc2d3098ae5641555f15c71d5bb5ce0060a8015c
[Envoy]: https://github.com/envoyproxy/envoy
[High Level Doc]: https://www.envoyproxy.io/docs/envoy/latest/intro/arch_overview/advanced/dynamic_modules
