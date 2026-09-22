FROM ghcr.io/cross-rs/x86_64-unknown-linux-musl:main

RUN apt-get update \
    && apt-get install -y --no-install-recommends \
        pkg-config \
        libfontconfig1-dev \
        fontconfig \
    && rm -rf /var/lib/apt/lists/*

ENV PKG_CONFIG_ALLOW_CROSS=1