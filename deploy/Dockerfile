# syntax=docker/dockerfile:1
# Image de production : node (SPA) -> rust musl (binaire statique) -> scratch (AD-1, AD-8).
# Contexte de build = racine du dépôt : `docker compose -f deploy/docker-compose.yml build`.

FROM node:26.12.1-alpine AS web
WORKDIR /src/web
COPY web/package.json web/package-lock.json ./
RUN --mount=type=cache,target=/root/.npm npm ci --no-audit --no-fund
COPY web/ ./
RUN npm run build

FROM rust:1.99.1-alpine AS build
RUN apk add --no-cache musl-dev
WORKDIR /src
# rust-toolchain.toml n'est pas copié : l'image fournit déjà 1.99.1, sans télécharger clippy ni rustfmt.
COPY Cargo.toml Cargo.lock ./
COPY crates crates
COPY reference reference
COPY --from=web /src/web/dist web/dist
ARG APP_BUILD=dev
ENV APP_BUILD=${APP_BUILD}
RUN --mount=type=cache,target=/usr/local/cargo/registry \
    --mount=type=cache,target=/src/target \
    cargo build --release --locked -p server \
    && cp target/release/server /server
# /data vide, avec ses deux sous-dossiers ; recopié dans le volume nommé à sa création.
RUN mkdir -p /out/data/reference /out/data/backups

FROM scratch
COPY --from=build /server /server
COPY --from=build --chown=10001:10001 /out/data /data
USER 10001:10001
EXPOSE 8080
ENTRYPOINT ["/server"]
CMD ["serve"]
