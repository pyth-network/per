ARG RUST_VERSION=1.66.1

# Get the solidity dependencies using npm
FROM node:21-alpine3.18 AS npm_build
WORKDIR /src
COPY per_multicall per_multicall
WORKDIR /src/per_multicall
RUN npm install


FROM rust:${RUST_VERSION} AS build
# Set default toolchain
RUN rustup default nightly-2023-07-23

# Install dependencies
RUN curl -L https://foundry.paradigm.xyz | bash
ENV PATH="${PATH}:/root/.foundry/bin/"
RUN foundryup

# Add solidity dependencies
WORKDIR /src
COPY per_multicall per_multicall
COPY --from=npm_build /src/per_multicall/node_modules/ /src/per_multicall/node_modules/
WORKDIR /src/per_multicall
RUN forge install foundry-rs/forge-std --no-git --no-commit
RUN forge install OpenZeppelin/openzeppelin-contracts --no-git --no-commit
RUN forge install OpenZeppelin/openzeppelin-contracts-upgradeable@v4.8.1 --no-git --no-commit

# Build auction-server
WORKDIR /src
COPY auction-server auction-server
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
