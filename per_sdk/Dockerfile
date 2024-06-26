ARG RUST_VERSION=1.66.1
ARG PYTHON_VERSION=3.11
ARG POETRY_VERSION=1.6.1

FROM node:21-alpine3.18 AS npm_build

WORKDIR /src
COPY contracts contracts
WORKDIR /src/contracts
RUN npm install


FROM rust:${RUST_VERSION} AS contract_build
# Set default toolchain
RUN rustup default nightly-2023-04-10

# Install dependencies
RUN curl -L https://foundry.paradigm.xyz | bash
ENV PATH="${PATH}:/root/.foundry/bin/"
RUN foundryup

# Add solidity dependencies
WORKDIR /src
COPY contracts contracts
COPY --from=npm_build /src/contracts/node_modules/ /src/contracts/node_modules/
WORKDIR /src/contracts
RUN forge install foundry-rs/forge-std@v1.8.0 --no-git --no-commit
RUN forge install OpenZeppelin/openzeppelin-contracts@v5.0.2 --no-git --no-commit
RUN forge install OpenZeppelin/openzeppelin-contracts-upgradeable@v4.9.6 --no-git --no-commit
RUN forge install Uniswap/permit2@0x000000000022D473030F116dDEE9F6B43aC78BA3 --no-git --no-commit
RUN forge install nomad-xyz/ExcessivelySafeCall@be417ab0c26233578b8d8f3a37b87bd1fcb4e286 --no-git --no-commit
RUN forge build --via-ir


FROM python:$PYTHON_VERSION


ARG APP_NAME
ARG APP_PATH
ARG POETRY_VERSION

ENV \
    PYTHONDONTWRITEBYTECODE=1 \
    PYTHONUNBUFFERED=1 \
    PYTHONFAULTHANDLER=1
ENV \
    POETRY_VERSION=$POETRY_VERSION \
    POETRY_HOME="/opt/poetry" \
    POETRY_VIRTUALENVS_IN_PROJECT=true \
    POETRY_NO_INTERACTION=1

# Install Poetry - respects $POETRY_VERSION & $POETRY_HOME
RUN curl -sSL https://install.python-poetry.org | python
ENV PATH="$POETRY_HOME/bin:$PATH"

# Copy only requirements to cache them in docker layer
WORKDIR /src
COPY per_sdk/poetry.lock per_sdk/pyproject.toml /src/per_sdk/
COPY --from=contract_build /src/contracts/out/ /src/contracts/out/

# Project initialization:
RUN poetry -C per_sdk install  --no-interaction --no-ansi

# Creating folders, and files for a project:
COPY per_sdk/ /src/per_sdk
