FROM rust:1.90-bookworm AS builder
WORKDIR /app
COPY . .
RUN cargo build --release -p zerant-api

FROM debian:bookworm-slim
RUN apt-get update && apt-get install -y --no-install-recommends ca-certificates && rm -rf /var/lib/apt/lists/*
COPY --from=builder /app/target/release/zerant-api /usr/local/bin/zerant-api
ENV PORT=8080
ENV ZERANT_BIND_ADDR=0.0.0.0
EXPOSE 8080
ENTRYPOINT ["zerant-api"]
