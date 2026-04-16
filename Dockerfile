# Unified Dockerfile for all services in the workspace.
# Build with: docker build --build-arg SERVICE_NAME=user-service -f Dockerfile .

ARG SERVICE_NAME

# --- Planner ---
FROM lukemathwalker/cargo-chef:latest-rust-1 AS chef
WORKDIR /app

FROM chef AS planner
COPY . .
RUN cargo chef prepare --recipe-path recipe.json

# --- Builder ---
FROM chef AS builder
ARG SERVICE_NAME

# Install protoc for tonic-prost-build
RUN apt-get update && apt-get install -y protobuf-compiler && rm -rf /var/lib/apt/lists/*

COPY --from=planner /app/recipe.json recipe.json
RUN cargo chef cook --release --recipe-path recipe.json

COPY . .
RUN cargo build --release --bin ${SERVICE_NAME}

# --- Runtime ---
FROM gcr.io/distroless/cc-debian12:nonroot AS runtime
ARG SERVICE_NAME
COPY --from=builder /app/target/release/${SERVICE_NAME} /usr/local/bin/service
USER nonroot
ENTRYPOINT ["/usr/local/bin/service"]
