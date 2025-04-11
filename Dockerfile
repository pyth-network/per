ARG RUST_VERSION=1.85.1

FROM rust:${RUST_VERSION} AS build

# Set default toolchain
RUN rustup default nightly-2024-04-10
RUN cargo install --git https://github.com/coral-xyz/anchor --tag v0.31.0 anchor-cli --locked

# Install protobuf (modify version as needed)
ARG PROTOC_VERSION=28.3
RUN curl -OL https://github.com/protocolbuffers/protobuf/releases/download/v${PROTOC_VERSION}/protoc-${PROTOC_VERSION}-linux-x86_64.zip && \
    unzip protoc-${PROTOC_VERSION}-linux-x86_64.zip -d /usr/local && \
    rm protoc-${PROTOC_VERSION}-linux-x86_64.zip

# Add /usr/local/bin to PATH if not already present
ENV PATH="/usr/local/bin:$PATH"

# Build auction-server
WORKDIR /src

COPY . .
RUN --mount=type=cache,target=/root/.cargo/registry cargo build -p auction-server --release

FROM rust:${RUST_VERSION}
# Copy artifacts from other images
COPY --from=build /src/target/release/auction-server /usr/local/bin/
