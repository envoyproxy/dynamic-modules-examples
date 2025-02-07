FROM --platform=$BUILDPLATFORM rust:1.84 AS rust_fetcher

WORKDIR /app

# Cache dependencies by copying only the Cargo files and fetching them on the build platform.
COPY ./rust/Cargo.toml ./rust/Cargo.lock ./
RUN mkdir src && echo "" > src/lib.rs
RUN cargo fetch
RUN rm -rf src

FROM rust:1.84 AS rust_builder

# We need libclang to do the bindgen.
RUN apt update && apt install -y clang

# Copy the dependencies from the previous stage.
WORKDIR /app
COPY --from=rust_fetcher /usr/local/cargo/git /usr/local/cargo/git
COPY --from=rust_fetcher /usr/local/cargo/registry /usr/local/cargo/registry

# Then, copy the entire source code and build.
COPY ./rust .
RUN cargo build

# Finally, copy the built library to the final image.
FROM envoyproxy/envoy-dev:80c1ac2143a7a73932c9dff814d38fd6867fe691 AS envoy
ENV ENVOY_DYNAMIC_MODULES_SEARCH_PATH=/usr/local/lib
COPY --from=rust_builder /app/target/debug/librust_module.so /usr/local/lib/librust_module.so
