# syntax=docker/dockerfile:1
# Build from repository root:
#   docker build -t easygo-http .

FROM --platform=$BUILDPLATFORM debian:bookworm-slim AS openssl102
RUN dpkg --add-architecture armhf \
 && echo 'deb [trusted=yes] http://archive.debian.org/debian stretch main' > /etc/apt/sources.list.d/stretch.list \
 && apt-get -o Acquire::Check-Valid-Until=false update \
 && cd /tmp && apt-get download libssl1.0.2:armhf \
 && dpkg-deb -x libssl1.0.2_*_armhf.deb /out

FROM --platform=$BUILDPLATFORM rust:1-bookworm AS builder
RUN apt-get update \
 && apt-get install -y --no-install-recommends g++-arm-linux-gnueabihf libc6-dev-armhf-cross \
 && rm -rf /var/lib/apt/lists/* \
 && rustup target add armv7-unknown-linux-gnueabihf
ENV CARGO_TARGET_ARMV7_UNKNOWN_LINUX_GNUEABIHF_LINKER=arm-linux-gnueabihf-gcc \
    CC_armv7_unknown_linux_gnueabihf=arm-linux-gnueabihf-gcc \
    CXX_armv7_unknown_linux_gnueabihf=arm-linux-gnueabihf-g++ \
    EASYGO_SDK_DIR=/sdk
COPY sdk/ /sdk/
WORKDIR /src
COPY Cargo.toml Cargo.lock* build.rs ./
COPY shim ./shim
COPY src ./src
RUN --mount=type=cache,target=/usr/local/cargo/registry \
    --mount=type=cache,target=/src/target \
    cargo build --release --target armv7-unknown-linux-gnueabihf \
 && cp target/armv7-unknown-linux-gnueabihf/release/easygo-http /easygo-http

FROM --platform=linux/arm/v7 debian:bookworm-slim
RUN apt-get update \
 && apt-get install -y --no-install-recommends libstdc++6 ca-certificates curl \
 && rm -rf /var/lib/apt/lists/*
COPY sdk/lib/ /opt/easygo/lib/
COPY --from=openssl102 /out/usr/lib/arm-linux-gnueabihf/libssl.so.1.0.2 \
                       /out/usr/lib/arm-linux-gnueabihf/libcrypto.so.1.0.2 /opt/easygo/lib/
RUN ln -sf libsqlite3.so /opt/easygo/lib/libsqlite3.so.0
COPY --from=builder /easygo-http /usr/local/bin/easygo-http
WORKDIR /var/lib/easygo
COPY sdk/rsa_public_key.pem ./
ENV LD_LIBRARY_PATH=/opt/easygo/lib \
    EASYGO_HTTP_ADDR=0.0.0.0:8080 \
    RUST_LOG=info
VOLUME ["/var/lib/easygo"]
EXPOSE 8080
HEALTHCHECK --interval=30s --timeout=3s CMD curl -fsS "http://${EASYGO_HTTP_ADDR}/healthz" || exit 1
CMD ["easygo-http"]
