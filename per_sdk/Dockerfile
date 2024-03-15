ARG RUST_VERSION=1.66.1
ARG PYTHON_VERSION=3.11
ARG POETRY_VERSION=1.6.1

FROM node:21-alpine3.18 AS npm_build

WORKDIR /src
COPY per_multicall per_multicall
WORKDIR /src/per_multicall
RUN npm install


FROM rust:${RUST_VERSION} AS contract_build
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
COPY --from=contract_build /src/per_multicall/out/ /src/per_multicall/out/

# Project initialization:
RUN poetry -C per_sdk install  --no-interaction --no-ansi

# Creating folders, and files for a project:
COPY per_sdk/ /src/per_sdk
