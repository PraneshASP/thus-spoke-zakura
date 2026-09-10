FROM node:24-alpine AS web
WORKDIR /src/web
COPY web/package.json web/package-lock.json* ./
RUN npm ci
COPY web/ ./
RUN npm run build

FROM rust:1.98-bookworm AS rust
WORKDIR /src
COPY Cargo.toml Cargo.lock* rust-toolchain.toml ./
COPY crates/ crates/
RUN --mount=type=cache,target=/usr/local/cargo/registry \
    --mount=type=cache,target=/usr/local/cargo/git \
    --mount=type=cache,target=/src/target \
    cargo build --release --locked -p tsz-server && \
    cp /src/target/release/tsz-server /tmp/tsz-server

FROM debian:bookworm-slim
RUN apt-get update && apt-get install -y --no-install-recommends ca-certificates && rm -rf /var/lib/apt/lists/*
COPY --from=rust /tmp/tsz-server /usr/local/bin/tsz-server
COPY --from=web /src/web/dist /opt/tsz/web
ENV TSZ_WEB_DIR=/opt/tsz/web
ENTRYPOINT ["tsz-server"]
