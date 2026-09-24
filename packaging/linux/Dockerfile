# Build environment for the Ubuntu .deb (GTK4 + libadwaita + WebKitGTK 6.0).
# Ubuntu 24.04 LTS is the first LTS shipping webkitgtk-6.0.
FROM ubuntu:24.04
ENV DEBIAN_FRONTEND=noninteractive
RUN apt-get update && apt-get install -y --no-install-recommends \
    build-essential curl ca-certificates pkg-config \
    libgtk-4-dev libadwaita-1-dev libwebkitgtk-6.0-dev libssl-dev \
    xvfb dbus-x11 xauth \
    && rm -rf /var/lib/apt/lists/*
ENV RUSTUP_HOME=/usr/local/rustup CARGO_HOME=/usr/local/cargo PATH=/usr/local/cargo/bin:$PATH
RUN curl -sSf https://sh.rustup.rs | sh -s -- -y --profile minimal --default-toolchain stable \
    && cargo install cargo-deb --locked
WORKDIR /src
