# syntax=docker/dockerfile:1
#
# The Kinige sync server (spike S4c-2, ADR-0020). Build from the repository root:
#   docker build -f deploy/Dockerfile -t kinige-server .
# Both base images are pinned by digest; bump them by hand together with a review (the tag in the comment is what the digest was taken from).

# rust:1.94.0-bookworm. The workspace's rust-toolchain.toml is not copied (Dockerfile.dockerignore): the image's own 1.94.0 is the pinned
# toolchain, and rustup would otherwise try to fetch the wasm target and components over the network.
FROM rust:1.94.0-bookworm@sha256:365468470075493dc4583f47387001854321c5a8583ea9604b297e67f01c5a4f AS build
WORKDIR /src
COPY Cargo.toml Cargo.lock ./
COPY crates ./crates
COPY server ./server
# A member of the workspace: its manifest must be there for cargo to read the workspace. It is not built.
COPY src-tauri ./src-tauri
RUN cargo build --release --locked -p kinige-server

# debian:bookworm-slim. ca-certificates is for an S3 store over HTTPS. Nothing else: no shell tools beyond Debian's base, no curl.
FROM debian:bookworm-slim@sha256:734a72b3d6142a71993b4bc580ca01d8ca687f77a2c422d96a961d3376b179bc
RUN apt-get update \
 && apt-get install -y --no-install-recommends ca-certificates \
 && rm -rf /var/lib/apt/lists/* \
 && useradd --system --uid 10001 --user-group --home-dir /nonexistent --shell /usr/sbin/nologin kinige \
 && mkdir -p /data/blobs /backups \
 && chown -R kinige:kinige /data /backups
COPY --from=build /src/target/release/kinige-server /usr/local/bin/kinige-server

# Not root. The server listens on 8080 behind a reverse proxy (deploy/docker-compose.yml) and stops on SIGTERM, which is what
# `docker stop` sends: it finishes the requests in flight and exits 0.
USER 10001:10001
ENV KINIGE_LISTEN=0.0.0.0:8080 \
    KINIGE_BLOB_DIR=/data/blobs
VOLUME ["/data", "/backups"]
EXPOSE 8080
STOPSIGNAL SIGTERM
ENTRYPOINT ["/usr/local/bin/kinige-server"]
CMD ["serve"]
