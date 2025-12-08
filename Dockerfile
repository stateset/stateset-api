# Build stage
ARG RUST_VERSION=1.88
FROM rust:${RUST_VERSION} AS builder

ARG API_BIN=stateset-api
ARG MIGRATION_BIN=migration
ARG SEED_BIN=seed-data
ENV CARGO_TERM_COLOR=always

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
COPY simple_api/Cargo.toml ./simple_api/

# Create dummy files to build dependencies
RUN mkdir -p src migrations/src proto benches src/bin/only_standalone simple_api/src
RUN echo "fn main() {}" > src/main.rs
RUN echo "pub fn run() {}" > migrations/src/lib.rs
RUN echo "fn main() {}" > src/bin/only_standalone/main.rs
RUN echo "fn main() {}" > simple_api/src/main.rs
RUN echo "fn main() {}" > src/bin/migration.rs
RUN echo "fn main() {}" > benches/api_benchmarks.rs

# Copy build.rs for the build script
COPY build.rs ./

# Copy proto files if they exist
COPY include/ ./include/

# Build dependencies (this layer will be cached)
RUN cargo build --locked --release --bin ${API_BIN} --bin ${MIGRATION_BIN}
RUN rm -rf src migrations/src

# Copy the actual source code
COPY . .

# Build the application
RUN cargo build --locked --release --bin ${API_BIN} --bin ${MIGRATION_BIN} --bin ${SEED_BIN}

# Runtime stage
FROM debian:bookworm-slim

ARG API_BIN=stateset-api
ARG MIGRATION_BIN=migration
ARG SEED_BIN=seed-data

# Install runtime dependencies
RUN apt-get update && apt-get install -y \
    ca-certificates \
    libssl3 \
    tini \
    && rm -rf /var/lib/apt/lists/*

# Create a non-root user
RUN useradd -m -u 1001 -s /bin/bash appuser

WORKDIR /app

# Copy the binary from builder
COPY --from=builder /usr/src/app/target/release/${API_BIN} /app/${API_BIN}
COPY --from=builder /usr/src/app/target/release/${MIGRATION_BIN} /app/${MIGRATION_BIN}
COPY --from=builder /usr/src/app/target/release/${SEED_BIN} /app/${SEED_BIN}

# Copy migrations if needed at runtime
COPY --from=builder /usr/src/app/migrations /app/migrations

# Copy config files
COPY --from=builder /usr/src/app/config /app/config

# Copy docker entrypoint
COPY docker/entrypoint.sh /app/docker-entrypoint.sh
RUN chmod +x /app/docker-entrypoint.sh

# Copy any static files or assets if needed
# COPY --from=builder /usr/src/app/static /app/static

# Change ownership to appuser
RUN chown -R appuser:appuser /app

USER appuser

# Expose the port your app runs on
EXPOSE 8080

# Set environment variables
ENV RUST_LOG=info \
    RUST_BACKTRACE=1 \
    APP__HOST=0.0.0.0 \
    APP__PORT=8080 \
    APP__ENVIRONMENT=production \
    RUN_ENV=production \
    APP_ENV=production \
    RUN_MIGRATIONS_ON_START=false

# Run the binary via entrypoint
ENTRYPOINT ["/usr/bin/tini", "--", "/app/docker-entrypoint.sh"]
CMD ["/app/stateset-api"]
