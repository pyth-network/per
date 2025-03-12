ARG RUST_VERSION=1.83.0

# Get the solidity dependencies using npm
FROM node:21-alpine3.18 AS npm_build
WORKDIR /src
COPY contracts/evm contracts/evm
WORKDIR /src/contracts/evm
RUN npm install

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

# Install dependencies
RUN curl -L https://foundry.paradigm.xyz | bash
ENV PATH="${PATH}:/root/.foundry/bin"
RUN foundryup

# Add contracts
WORKDIR /src
COPY contracts contracts

# Add solidity dependencies
COPY --from=npm_build /src/contracts/evm/node_modules/ /src/contracts/evm/node_modules/
WORKDIR /src/contracts/evm
RUN forge install foundry-rs/forge-std@v1.8.0 --no-git --no-commit
RUN forge install OpenZeppelin/openzeppelin-contracts@v5.0.2 --no-git --no-commit
RUN forge install OpenZeppelin/openzeppelin-contracts-upgradeable@v4.9.6 --no-git --no-commit
RUN forge install Uniswap/permit2@0x000000000022D473030F116dDEE9F6B43aC78BA3 --no-git --no-commit
RUN forge install nomad-xyz/ExcessivelySafeCall@be417ab0c26233578b8d8f3a37b87bd1fcb4e286 --no-git --no-commit

# Build auction-server
WORKDIR /src

COPY . .
RUN --mount=type=cache,target=/root/.cargo/registry cargo build -p auction-server -p vault-simulator --release

FROM rust:${RUST_VERSION}
# Copy artifacts from other images
COPY --from=build /src/target/release/auction-server /usr/local/bin/
COPY --from=build /src/target/release/vault-simulator /usr/local/bin/
