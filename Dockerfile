# This is the example Dockerfile for building the multi arch Envoy image with the Rust and Go dynamic module.

##### Build the Rust library #####

# Use https://github.com/rust-cross/cargo-zigbuild to cross-compile the Rust library for both x86_64 and aarch64 architectures.
# We need it because bindgen relies on the sysroot of the target architecture which makes it
# a bit hairy to cross-compile.
#
# If you don't need multi-arch support, you can use simply `FROM rust:x.y.z` and `cargo build` on the host architecture or,
# compile the shared library on each architecture separately and copy the shared library to the final image.
FROM --platform=$BUILDPLATFORM ghcr.io/rust-cross/cargo-zigbuild:0.19.8 AS rust_builder

WORKDIR /build

# bindgen requires libclang-dev.
RUN apt update && apt install -y clang

# Fetch the dependencies first to leverage Docker cache.
COPY ./rust/Cargo.toml ./rust/Cargo.lock ./
RUN mkdir src && echo "" > src/lib.rs
RUN cargo fetch
RUN rm -rf src

# Then copy the rest of the source code and build the library.
COPY ./rust .
RUN cargo zigbuild --target aarch64-unknown-linux-gnu
RUN cargo zigbuild --target x86_64-unknown-linux-gnu

RUN cp /build/target/aarch64-unknown-linux-gnu/debug/librust_module.so /build/arm64_librust_module.so
RUN cp /build/target/x86_64-unknown-linux-gnu/debug/librust_module.so /build/amd64_librust_module.so

##### Build the Go library #####

# We use zig to cross-compile the Go library for both x86_64 and aarch64 architectures.
FROM --platform=$BUILDPLATFORM golang:1.24.2 AS go_builder
# Install zig.
ARG ZIG_VERSION=0.14.0
RUN apt update && apt install -y curl xz-utils
RUN curl -L "https://ziglang.org/download/${ZIG_VERSION}/zig-linux-$(uname -m)-${ZIG_VERSION}.tar.xz" | tar -J -x -C /usr/local && \
    ln -s "/usr/local/zig-linux-$(uname -m)-${ZIG_VERSION}/zig" /usr/local/bin/zig
# Build the Go library.
RUN mkdir /build
COPY ./go /build
WORKDIR /build
RUN CC="zig cc -target aarch64-linux-gnu" CXX="zig c++ -target aarch64-linux-gnu" CGO_ENABLED=1 GOARCH=arm64 go build -buildmode=c-shared -o /build/arm64_libgo_module.so .
RUN CC="zig cc -target x86_64-linux-gnu" CXX="zig c++ -target x86_64-linux-gnu" CGO_ENABLED=1 GOARCH=amd64 go build -buildmode=c-shared -o /build/amd64_libgo_module.so .

##### Build the final image #####
FROM envoyproxy/envoy-dev:5b88f941da971de57f29286103c20770811ec67f AS envoy
ARG TARGETARCH
ENV ENVOY_DYNAMIC_MODULES_SEARCH_PATH=/usr/local/lib
COPY --from=rust_builder /build/${TARGETARCH}_librust_module.so /usr/local/lib/librust_module.so
COPY --from=go_builder /build/${TARGETARCH}_libgo_module.so /usr/local/lib/libgo_module.so
