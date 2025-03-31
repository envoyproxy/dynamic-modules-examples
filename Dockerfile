# This is the example Dockerfile for building the multi arch Envoy image with the Rust dynamic module.

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

# Finally, copy the built library to the final image.
FROM envoyproxy/envoy-dev:a27d2c31627e59f096f7c8cdc84488649158b000 AS envoy
ARG TARGETARCH
ENV ENVOY_DYNAMIC_MODULES_SEARCH_PATH=/usr/local/lib
COPY --from=rust_builder /build/${TARGETARCH}_librust_module.so /usr/local/lib/librust_module.so
