# Build stage
FROM rust:1.75 as builder

WORKDIR /usr/src/app

# Copy manifests
COPY Cargo.toml ./

# Copy source code
COPY src ./src

# Build release binary
RUN cargo build --release

# Runtime stage
FROM debian:bookworm-slim

# Install runtime dependencies
RUN apt-get update && \
    apt-get install -y ca-certificates && \
    rm -rf /var/lib/apt/lists/*

# Create non-root user
RUN useradd -m -u 1000 agentic

# Copy binary from builder
COPY --from=builder /usr/src/app/target/release/agentic-commerce-server /usr/local/bin/

# Set ownership
RUN chown -R agentic:agentic /usr/local/bin/agentic-commerce-server

# Switch to non-root user
USER agentic

# Expose port
EXPOSE 8080

# Health check
HEALTHCHECK --interval=30s --timeout=3s --start-period=5s --retries=3 \
    CMD curl -f http://localhost:8080/health || exit 1

# Run the binary
CMD ["agentic-commerce-server"] 