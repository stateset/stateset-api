# Build stage
FROM rust:1.88 AS builder

# Install protobuf compiler for proto files
RUN apt-get update && apt-get install -y \
    protobuf-compiler \
    && rm -rf /var/lib/apt/lists/*

# Create app directory
WORKDIR /usr/src/app

# Copy Cargo files first for better caching
COPY Cargo.toml Cargo.lock ./
COPY migrations/Cargo.toml ./migrations/
COPY src/bin/only_standalone/Cargo.toml ./src/bin/only_standalone/

# Create dummy files to build dependencies
RUN mkdir -p src migrations/src proto src/bin/only_standalone
RUN echo "fn main() {}" > src/main.rs
RUN echo "pub fn run() {}" > migrations/src/lib.rs
RUN echo "fn main() {}" > src/bin/only_standalone/main.rs

# Copy build.rs for the build script
COPY build.rs ./

# Copy proto files if they exist
COPY include/ ./include/

# Build dependencies (this layer will be cached)
RUN cargo build --release --bin stateset-api --bin migration
RUN rm -rf src migrations/src

# Copy the actual source code
COPY . .

# Build the application
RUN cargo build --release --bin stateset-api --bin migration

# Runtime stage
FROM debian:bookworm-slim

# Install runtime dependencies
RUN apt-get update && apt-get install -y \
    ca-certificates \
    libssl3 \
    && rm -rf /var/lib/apt/lists/*

# Create a non-root user
RUN useradd -m -u 1001 -s /bin/bash appuser

WORKDIR /app

# Copy the binary from builder
COPY --from=builder /usr/src/app/target/release/stateset-api /app/stateset-api
COPY --from=builder /usr/src/app/target/release/migration /app/migration

# Copy migrations if needed at runtime
COPY --from=builder /usr/src/app/migrations /app/migrations

# Copy config files
COPY --from=builder /usr/src/app/config /app/config

# Copy any static files or assets if needed
# COPY --from=builder /usr/src/app/static /app/static

# Change ownership to appuser
RUN chown -R appuser:appuser /app

USER appuser

# Expose the port your app runs on
EXPOSE 3000

# Set environment variables
ENV RUST_LOG=info
ENV RUST_BACKTRACE=1

# Run the binary
CMD ["./stateset-api"]
