# Race server image: docker build -t code-racer-server . && docker run -p 8080:8080 code-racer-server

FROM rust:1-slim-bookworm AS builder
WORKDIR /app
COPY Cargo.toml Cargo.lock ./
COPY crates ./crates
COPY src ./src
RUN cargo build --locked --release --package code-racer-server

FROM debian:bookworm-slim
RUN useradd --system --uid 10001 racer
COPY --from=builder /app/target/release/code-racer-server /usr/local/bin/code-racer-server
USER racer
EXPOSE 8080
ENTRYPOINT ["code-racer-server"]
CMD ["--host", "0.0.0.0", "--port", "8080"]
