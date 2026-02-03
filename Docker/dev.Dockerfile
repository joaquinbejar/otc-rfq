# =============================================================================
# OTC-RFQ Service - Development Dockerfile
# =============================================================================
# Development image with hot-reload support via cargo-watch
# Includes all development dependencies and debug symbols
# =============================================================================

FROM rust:1.93-alpine3.23

# Install development dependencies
RUN apk add --no-cache \
    musl-dev \
    pkgconfig \
    openssl-dev \
    openssl-libs-static \
    protobuf-dev \
    protoc \
    curl \
    git

# Install development tools
RUN cargo install cargo-watch --locked
RUN cargo install cargo-tarpaulin --locked || true

# Create non-root user for development
RUN addgroup -g 1000 dev \
    && adduser -u 1000 -G dev -s /bin/sh -D dev

# Create app directory
RUN mkdir -p /app && chown -R dev:dev /app

WORKDIR /app

# Switch to non-root user
USER dev

# Environment variables for development
ENV RUST_LOG=debug
ENV RUST_BACKTRACE=full
ENV CARGO_HOME=/home/dev/.cargo
ENV CARGO_TARGET_DIR=/app/target

# Expose ports
# HTTP API port
EXPOSE 8080
# gRPC port
EXPOSE 50051
# Metrics port
EXPOSE 9090

# Default command: watch for changes and rebuild
CMD ["cargo", "watch", "-x", "run"]

# Labels for metadata
LABEL org.opencontainers.image.title="OTC-RFQ Service (Development)"
LABEL org.opencontainers.image.description="OTC Request for Quote Trading Platform - Development Image"
