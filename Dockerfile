FROM --platform=$BUILDPLATFORM rust:1.84 AS rust_builder

# We need libclang to do the bindgen.
RUN apt update && apt install -y clang

WORKDIR /app

# Cache dependencies by copying only the Cargo files and fetching them.
COPY ./rust/Cargo.toml ./rust/Cargo.lock ./
RUN mkdir src && echo "" > src/lib.rs
RUN cargo fetch
RUN rm -rf src

# Then, copy the entire source code and build.
COPY ./rust .
RUN cargo build

# Finally, copy the built library to the final image.
FROM --platform=$BUILDPLATFORM  envoyproxy/envoy@sha256:9ca0dcc84ec582b7ece0ccf6c24af76268d017c87376f69a0dc4a1a0ab55b4c4 AS envoy
ENV ENVOY_DYNAMIC_MODULES_SEARCH_PATH=/usr/local/lib
COPY --from=rust_builder /app/target/debug/librust_module.so /usr/local/lib/librust_module.so
