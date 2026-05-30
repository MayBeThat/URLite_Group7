# ── Stage 1: Build ──────────────────────────────────────────────────────────
FROM rust:1.88-slim AS builder

RUN apt-get update && apt-get install -y \
    pkg-config \
    libssl-dev \
    libsqlite3-dev \
    && rm -rf /var/lib/apt/lists/*

WORKDIR /app

COPY Cargo.toml Cargo.lock ./
RUN mkdir src && echo "fn main() {}" > src/main.rs
RUN cargo build --release
RUN rm -rf src

COPY src ./src
COPY migrations ./migrations

# Build chính xác binary "server"
RUN cargo build --release --bin server

# ── Stage 2: Runtime ──────────────────────────────────────────────────────────
FROM debian:bookworm-slim

RUN apt-get update && apt-get install -y \
    ca-certificates \
    libssl3 \
    libsqlite3-0 \
    && rm -rf /var/lib/apt/lists/*

WORKDIR /app

# Copy trực tiếp từ đúng đường dẫn build
COPY --from=builder /app/target/release/server ./server
COPY --from=builder /app/migrations ./migrations

RUN mkdir -p /app/data && touch /app/data/urls.db
# Thêm dòng này để cấp quyền cho user chạy ứng dụng (nếu cần)
RUN chmod -R 777 /app/data
EXPOSE 8080

# Chạy trực tiếp binary
CMD ["./server"]