FROM rust:1-bookworm AS builder

WORKDIR /workspace
COPY . .
RUN cargo build -p api --release

FROM debian:bookworm-slim

RUN apt-get update \
  && apt-get install -y --no-install-recommends ca-certificates tzdata \
  && rm -rf /var/lib/apt/lists/*

WORKDIR /app
COPY --from=builder /workspace/target/release/api /usr/local/bin/cloudops-api

ENV API_HOST=0.0.0.0
ENV API_PORT=8080
ENV RUST_LOG=info

EXPOSE 8080

CMD ["cloudops-api"]
