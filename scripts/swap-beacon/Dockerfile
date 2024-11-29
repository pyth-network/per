# syntax=docker/dockerfile:1

# Comments are provided throughout this file to help you get started.
# If you need more help, visit the Dockerfile reference guide at
# https://docs.docker.com/go/dockerfile-reference/

# Want to help us make this template better? Share your feedback here: https://forms.gle/ybq9Krt8jtBL3iCk7

ARG NODE_VERSION=18.20.2

################################################################################
# Use node image for base image for all stages.
FROM node:${NODE_VERSION}-alpine as base

# Set working directory for all build stages.
WORKDIR /usr/src/app

# Install pnpm package manager.
ARG PNPM_VERSION=9.14.3
RUN npm install -g pnpm@${PNPM_VERSION}

################################################################################
# Create a stage for building the application.
FROM base as build

# Copy the rest of the source files into the image.
COPY pnpm-lock.yaml .
COPY pnpm-workspace.yaml .
COPY scripts/swap-beacon scripts/swap-beacon

RUN --mount=type=cache,id=pnpm,target=/pnpm/store pnpm install --frozen-lockfile

# Run the build script.
RUN pnpm --filter swap-beacon build
RUN pnpm deploy --filter swap-beacon --prod /prod/swap-beacon

################################################################################
# Create a new stage to run the application with minimal runtime dependencies
# where the necessary files are copied from the build stage.
FROM base as final

# Use production node environment by default.
ENV NODE_ENV production

# Run the application as a non-root user.
USER node

# Copy the production dependencies from the deps stage and also
# the built application from the build stage into the image.
COPY --from=build /prod/swap-beacon/node_modules ./node_modules
COPY --from=build /usr/src/app/scripts/swap-beacon/dist ./dist
