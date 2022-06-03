# syntax=docker/dockerfile:1.3

FROM rust:1.61.0-bullseye AS builder

WORKDIR /usr/src/app

COPY Cargo.lock Cargo.toml ./
COPY src ./src
RUN --mount=type=cache,target=/usr/local/cargo/git \
    --mount=type=cache,target=/usr/local/cargo/registry \
    --mount=type=cache,sharing=private,target=/usr/src/app/target \
    cargo install --locked --path . --root .


# hadolint ignore=DL3006
FROM gcr.io/distroless/cc-debian11

WORKDIR /app

COPY --from=builder /usr/src/app/bin/* /usr/local/bin/

HEALTHCHECK CMD ["/usr/local/bin/healthcheck", "8080"]

USER nonroot
EXPOSE 8080
CMD ["/usr/local/bin/static_config_api"]
