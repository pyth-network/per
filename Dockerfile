ARG RUST_VERSION=1.66.1

# Get the solidity dependencies using npm
FROM node:21-alpine3.18 AS npm_build
WORKDIR /src
COPY per_multicall per_multicall
WORKDIR /src/per_multicall
RUN npm install


FROM rust:${RUST_VERSION} AS build
# Set default toolchain
RUN rustup default nightly-2024-04-10

# Install dependencies
RUN curl -L https://foundry.paradigm.xyz | bash
ENV PATH="${PATH}:/root/.foundry/bin/"
RUN foundryup

# Add solidity dependencies
WORKDIR /src
COPY per_multicall per_multicall
COPY --from=npm_build /src/per_multicall/node_modules/ /src/per_multicall/node_modules/
WORKDIR /src/per_multicall
RUN forge install foundry-rs/forge-std@v1.8.0 --no-git --no-commit
RUN forge install OpenZeppelin/openzeppelin-contracts@v5.0.2 --no-git --no-commit
RUN forge install OpenZeppelin/openzeppelin-contracts-upgradeable@v4.9.6 --no-git --no-commit
RUN forge install Uniswap/permit2@0x000000000022D473030F116dDEE9F6B43aC78BA3 --no-git --no-commit
RUN forge install nomad-xyz/ExcessivelySafeCall@be417ab0c26233578b8d8f3a37b87bd1fcb4e286 --no-git --no-commit

# Build auction-server
WORKDIR /src
COPY auction-server auction-server
COPY gas-oracle gas-oracle
WORKDIR /src/auction-server
RUN --mount=type=cache,target=/root/.cargo/registry cargo build --release

# Build vault-simulator
WORKDIR /src
COPY vault-simulator vault-simulator
WORKDIR /src/vault-simulator
RUN --mount=type=cache,target=/root/.cargo/registry cargo build --release


FROM rust:${RUST_VERSION}
# Copy artifacts from other images
COPY --from=build /src/auction-server/target/release/auction-server /usr/local/bin/
COPY --from=build /src/vault-simulator/target/release/vault-simulator /usr/local/bin/
