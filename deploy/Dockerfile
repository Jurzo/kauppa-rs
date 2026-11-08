# kauppa-rs container image (MB-0022 P1, P2; RR-0014).
#
# Build from the repository root (BuildKit, the default since Docker 23):
#
#   docker build -f deploy/Dockerfile -t kauppa-rs:local .
#   docker buildx build --platform linux/amd64 -f deploy/Dockerfile ...   # for the droplet
#
# The build context is filtered by the root .dockerignore (an allowlist).
# Base images are pinned by tag and digest; bump them together.

ARG RUST_IMAGE=rust:1.99.1-slim-trixie@sha256:f6c4a35d21dbb3c89d7a8d056c193e94fc09037f2b58dd858b09c39a47dd02d7
ARG RUNTIME_IMAGE=debian:trixie-20261104-slim@sha256:a72b4433de1cf7d6bae0c4cee74d0547f58601a643b3375f070c51600d63b6b2

# --- builder: release build of the kauppa-web binary -----------------------
FROM ${RUST_IMAGE} AS builder

WORKDIR /build

# rust-toolchain.toml is not in the build context, so the image's pinned
# toolchain builds the binary (no rustup download during the build).
COPY Cargo.toml Cargo.lock ./
COPY crates ./crates
# sqlx::migrate! embeds migrations/ at compile time.
COPY migrations ./migrations

RUN --mount=type=cache,target=/usr/local/cargo/registry,sharing=locked \
    --mount=type=cache,target=/usr/local/cargo/git,sharing=locked \
    --mount=type=cache,target=/build/target,sharing=locked \
    cargo build --release --locked -p kauppa-web \
    && install -D -m 0755 target/release/kauppa-web /out/kauppa-web

# --- runtime: binary + templates + assets, as a non-root user ---------------
FROM ${RUNTIME_IMAGE}

# The container user. The droplet's cloud-init creates the data directory
# with this uid (deploy/terraform/cloud-init.yaml.tftpl); keep them equal.
ARG APP_UID=10001

RUN groupadd --system --gid "${APP_UID}" kauppa \
    && useradd --system --uid "${APP_UID}" --gid kauppa \
        --home-dir /nonexistent --no-create-home --shell /usr/sbin/nologin kauppa \
    && install -d -o kauppa -g kauppa -m 0750 /data

# Templates load from ./templates and static files from ./assets, relative
# to the working directory (templates.rs, main.rs), until MB-0024 embeds
# them. Everything here is root-owned and read-only to the app.
WORKDIR /app
COPY templates ./templates
COPY assets ./assets
COPY migrations ./migrations
COPY deploy/docker-entrypoint.sh /usr/local/bin/docker-entrypoint.sh
COPY deploy/healthcheck.sh /usr/local/bin/healthcheck
COPY --from=builder /out/kauppa-web /usr/local/bin/kauppa-web

# Defaults for the container; Compose sets the rest (deploy/compose.yaml).
# KAUPPA_SESSION_SECRET has no default here: the entrypoint refuses to start
# without a real one (RR-0014 constraint 2).
ENV KAUPPA_BIND=0.0.0.0:8080 \
    KAUPPA_DATABASE_URL=sqlite:///data/kauppa.db \
    KAUPPA_LOG_LEVEL=info

USER ${APP_UID}:${APP_UID}
VOLUME ["/data"]
EXPOSE 8080

HEALTHCHECK --interval=10s --timeout=5s --start-period=30s --retries=3 \
    CMD ["/usr/local/bin/healthcheck"]

ENTRYPOINT ["/usr/local/bin/docker-entrypoint.sh"]

# Last, so a new revision doesn't invalidate the layers above.
ARG KAUPPA_REVISION=unknown
LABEL org.opencontainers.image.title="kauppa-rs" \
      org.opencontainers.image.source="https://github.com/Jurzo/kauppa-rs" \
      org.opencontainers.image.licenses="MIT" \
      org.opencontainers.image.revision="${KAUPPA_REVISION}"
