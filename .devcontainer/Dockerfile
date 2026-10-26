# RustRover / Dev Container：Linux 内交叉编译 armhf + qemu-arm 运行（M1 宿主机可用）
FROM --platform=$BUILDPLATFORM debian:bookworm-slim AS openssl102
RUN dpkg --add-architecture armhf \
 && echo 'deb [trusted=yes] http://archive.debian.org/debian stretch main' > /etc/apt/sources.list.d/stretch.list \
 && apt-get -o Acquire::Check-Valid-Until=false update \
 && cd /tmp && apt-get download libssl1.0.2:armhf \
 && dpkg-deb -x libssl1.0.2_*_armhf.deb /out

FROM rust:1-bookworm
RUN apt-get update \
 && apt-get install -y --no-install-recommends \
      g++-arm-linux-gnueabihf libc6-dev-armhf-cross \
      qemu-user-static \
      ca-certificates curl git \
 && rm -rf /var/lib/apt/lists/* \
 && rustup target add armv7-unknown-linux-gnueabihf

RUN mkdir -p /opt/easygo/extra-lib \
 && true

COPY --from=openssl102 /out/usr/lib/arm-linux-gnueabihf/libssl.so.1.0.2 \
                       /out/usr/lib/arm-linux-gnueabihf/libcrypto.so.1.0.2 /opt/easygo/extra-lib/

COPY .devcontainer/easygo-run-armhf.sh /usr/local/bin/easygo-run-armhf
RUN chmod +x /usr/local/bin/easygo-run-armhf

ENV PATH="/usr/local/cargo/bin:${PATH}" \
    EASYGO_SDK_DIR=/workspace/sdk \
    CARGO_BUILD_TARGET=armv7-unknown-linux-gnueabihf \
    CARGO_TARGET_ARMV7_UNKNOWN_LINUX_GNUEABIHF_LINKER=arm-linux-gnueabihf-gcc \
    CC_armv7_unknown_linux_gnueabihf=arm-linux-gnueabihf-gcc \
    CXX_armv7_unknown_linux_gnueabihf=arm-linux-gnueabihf-g++ \
    CARGO_TARGET_ARMV7_UNKNOWN_LINUX_GNUEABIHF_RUNNER=easygo-run-armhf \
    EASYGO_HTTP_ADDR=0.0.0.0:8093 \
    RUST_LOG=info

WORKDIR /workspace
