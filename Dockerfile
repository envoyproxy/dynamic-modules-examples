FROM --platform=$TARGETPLATFORM rust:1.84 AS rust_builder

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
FROM --platform=$TARGETPLATFORM envoyproxy/envoy-dev:80c1ac2143a7a73932c9dff814d38fd6867fe691 AS envoy
ENV ENVOY_DYNAMIC_MODULES_SEARCH_PATH=/usr/local/lib
COPY --from=rust_builder /app/target/debug/librust_module.so /usr/local/lib/librust_module.so
