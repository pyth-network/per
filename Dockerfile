# ARG RUST_VERSION=1.66.1

# Get the solidity dependencies using npm
# FROM node:21-alpine3.18 AS npm_build
# WORKDIR /src
# COPY contracts/evm contracts/evm
# WORKDIR /src/contracts/evm
# RUN npm install

# Build solana anchor
FROM solanalabs/solana:v1.18.18 AS solana_build
RUN apt-get update && apt-get install -y curl
RUN curl https://sh.rustup.rs -sSf > /tmp/rustup-init.sh \
    && chmod +x /tmp/rustup-init.sh \
    && sh /tmp/rustup-init.sh -y \
    && rm -rf /tmp/rustup-init.sh
ENV PATH "$PATH:~/.cargo/bin"
RUN cargo install --git https://github.com/coral-xyz/anchor --tag v0.30.1 anchor-cli --locked
WORKDIR /src
COPY contracts/svm contracts/svm
WORKDIR /src/contracts/svm
# RUN rustup default nightly-2024-02-04
RUN anchor build

# FROM rust:${RUST_VERSION} AS build

# # Set default toolchain
# RUN rustup default nightly-2024-04-10

# # Install dependencies
# RUN curl -L https://foundry.paradigm.xyz | bash
# ENV PATH="${PATH}:/root/.foundry/bin/"
# RUN foundryup

# # Add contracts
# WORKDIR /src
# COPY contracts contracts

# # Add solidity dependencies
# COPY --from=npm_build /src/contracts/evm/node_modules/ /src/contracts/evm/node_modules/
# WORKDIR /src/contracts/evm
# RUN forge install foundry-rs/forge-std@v1.8.0 --no-git --no-commit
# RUN forge install OpenZeppelin/openzeppelin-contracts@v5.0.2 --no-git --no-commit
# RUN forge install OpenZeppelin/openzeppelin-contracts-upgradeable@v4.9.6 --no-git --no-commit
# RUN forge install Uniswap/permit2@0x000000000022D473030F116dDEE9F6B43aC78BA3 --no-git --no-commit
# RUN forge install nomad-xyz/ExcessivelySafeCall@be417ab0c26233578b8d8f3a37b87bd1fcb4e286 --no-git --no-commit

# # Add solana dependencies
# COPY --from=solana_build /src/contracts/svm/target/ /src/contracts/svm/target/

# # Build auction-server
# WORKDIR /src
# COPY auction-server auction-server
# COPY gas-oracle gas-oracle
# WORKDIR /src/auction-server
# RUN --mount=type=cache,target=/root/.cargo/registry cargo build --release

# # Build vault-simulator
# WORKDIR /src
# COPY vault-simulator vault-simulator
# WORKDIR /src/vault-simulator
# RUN --mount=type=cache,target=/root/.cargo/registry cargo build --release


# FROM rust:${RUST_VERSION}
# # Copy artifacts from other images
# COPY --from=build /src/auction-server/target/release/auction-server /usr/local/bin/
# COPY --from=build /src/vault-simulator/target/release/vault-simulator /usr/local/bin/
