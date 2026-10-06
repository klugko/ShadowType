# Race server image: docker build -t code-racer-server . && docker run -p 8080:8080 code-racer-server

FROM rust:1-slim-bookworm AS builder
WORKDIR /app

# Dependencies first, built from the manifests against empty crates, so this layer stays
# cached until a manifest or Cargo.lock changes. The empty crates' artifacts are dropped so
# that the real sources are always compiled. The client (src/) is never copied: the root
# package only gets a placeholder main.rs to remain a valid workspace member.
COPY Cargo.toml Cargo.lock ./
COPY crates/engine/Cargo.toml crates/engine/
COPY crates/protocol/Cargo.toml crates/protocol/
COPY crates/server/Cargo.toml crates/server/
RUN mkdir src crates/engine/src crates/protocol/src crates/server/src \
 && echo 'fn main() {}' > src/main.rs \
 && touch crates/engine/src/lib.rs crates/protocol/src/lib.rs crates/server/src/lib.rs \
 && cargo build --locked --release --package code-racer-server \
 && cargo clean --release --package code-racer-engine --package code-racer-protocol --package code-racer-server

COPY crates ./crates
RUN cargo build --locked --release --package code-racer-server

FROM debian:bookworm-slim
RUN useradd --system --uid 10001 racer
COPY --from=builder /app/target/release/code-racer-server /usr/local/bin/code-racer-server
USER racer
EXPOSE 8080
ENTRYPOINT ["code-racer-server"]
CMD ["--host", "0.0.0.0", "--port", "8080"]
