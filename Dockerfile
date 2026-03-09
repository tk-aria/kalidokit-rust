FROM rust:1.85-bookworm AS builder
WORKDIR /app
COPY . .
RUN apt-get update && apt-get install -y cmake pkg-config libx11-dev libxkbcommon-dev nasm libclang-dev && \
    cargo build --release

FROM debian:bookworm-slim
RUN apt-get update && apt-get install -y libvulkan1 libx11-6 libxkbcommon0 && rm -rf /var/lib/apt/lists/*
COPY --from=builder /app/target/release/kalidokit-rust /usr/local/bin/
COPY --from=builder /app/assets /app/assets
WORKDIR /app
CMD ["kalidokit-rust"]
