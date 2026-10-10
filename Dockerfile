FROM rust:1.98-slim AS builder

WORKDIR /app

# Install musl tools for static binary
RUN apt-get update && apt-get install -y musl-tools pkg-config perl make && rm -rf /var/lib/apt/lists/*
RUN rustup target add x86_64-unknown-linux-musl

# Cache dependencies
COPY Cargo.toml Cargo.lock ./
RUN mkdir src && echo "fn main() {}" > src/main.rs
RUN cargo build --release --target x86_64-unknown-linux-musl || true
RUN rm -rf src

# Build actual binary
COPY . .
ARG TGCP_VERSION=dev
ENV TGCP_VERSION=${TGCP_VERSION}
# Touch sources so cargo rebuilds over the cached placeholder binary
RUN touch src/main.rs && cargo build --release --target x86_64-unknown-linux-musl
RUN cp target/x86_64-unknown-linux-musl/release/tgcp /tgcp

FROM scratch
# CA bundle for TLS verification (rustls uses the system trust store)
COPY --from=builder /etc/ssl/certs/ca-certificates.crt /etc/ssl/certs/ca-certificates.crt
COPY --from=builder /tgcp /tgcp
ENTRYPOINT ["/tgcp"]
