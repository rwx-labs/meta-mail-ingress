ARG RUST_VERSION=1.78.0

FROM rust:${RUST_VERSION} as builder

WORKDIR /app
COPY . /app

RUN cargo build --release

FROM debian:bookworm

RUN apt update && apt install -y exiftran libssl3 exiftool ca-certificates

USER nobody

WORKDIR /app
COPY --from=builder /app/target/release/meta-mail-ingress /app/meta-mail-ingress
COPY config.toml /app/config.toml

ENTRYPOINT ["/app/meta-mail-ingress"]
