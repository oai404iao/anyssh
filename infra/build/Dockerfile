# syntax=docker/dockerfile:1.7@sha256:a57df69d0ea827fb7266491f2813635de6f17269be881f696fbfdf2d83dda33e

ARG RUST_IMAGE=rust:1.93.1-bookworm@sha256:7c4ae649a84014c467d79319bbf17ce2632ae8b8be123ac2fb2ea5be46823f31
ARG NODE_IMAGE=node:24.13.0-bookworm-slim@sha256:4660b1ca8b28d6d1906fd644abe34b2ed81d15434d26d845ef0aced307cf4b6f

FROM ${RUST_IMAGE} AS rust-toolchain
FROM ${NODE_IMAGE} AS base

COPY --from=rust-toolchain /usr/local/cargo /usr/local/cargo
COPY --from=rust-toolchain /usr/local/rustup /usr/local/rustup

ENV CARGO_HOME=/usr/local/cargo
ENV RUSTUP_HOME=/usr/local/rustup
ENV PATH=/usr/local/cargo/bin:${PATH}

RUN apt-get update \
    && DEBIAN_FRONTEND=noninteractive apt-get install --yes --no-install-recommends \
      bash \
      build-essential \
      ca-certificates \
      cmake \
      curl \
      file \
      git \
      libssl-dev \
      ninja-build \
      perl \
      pkg-config \
      python3 \
      unzip \
      xz-utils \
      zip \
    && npm install --global pnpm@11.7.0 \
    && rustup component add clippy rustfmt \
    && rustc --version \
    && cargo --version \
    && node --version \
    && pnpm --version \
    && chmod -R a+rX "$RUSTUP_HOME" "$CARGO_HOME" \
    && rm -rf /var/lib/apt/lists/*

WORKDIR /workspace

FROM base AS linux

RUN apt-get update \
    && DEBIAN_FRONTEND=noninteractive apt-get install --yes --no-install-recommends \
      libayatana-appindicator3-dev \
      librsvg2-dev \
      libwebkit2gtk-4.1-dev \
      patchelf \
    && rm -rf /var/lib/apt/lists/*

FROM base AS android

ARG ANDROID_CMDLINE_TOOLS_VERSION=13114758
ARG ANDROID_CMDLINE_TOOLS_SHA256=7ec965280a073311c339e571cd5de778b9975026cfcbe79f2b1cdcb1e15317ee
ARG ANDROID_SDK_VERSION=36
ARG ANDROID_BUILD_TOOLS_VERSION=35.0.0
ARG ANDROID_NDK_VERSION=29.0.13846066

ENV ANDROID_HOME=/opt/android-sdk
ENV ANDROID_SDK_ROOT=/opt/android-sdk
ENV JAVA_HOME=/usr/lib/jvm/java-17-openjdk-amd64
ENV NDK_HOME=/opt/android-sdk/ndk/29.0.13846066
ENV PATH=/opt/android-sdk/platform-tools:/opt/android-sdk/cmdline-tools/bin:${PATH}

RUN apt-get update \
    && DEBIAN_FRONTEND=noninteractive apt-get install --yes --no-install-recommends \
      openjdk-17-jdk-headless \
    && rm -rf /var/lib/apt/lists/* \
    && mkdir -p "$ANDROID_HOME" \
    && curl --fail --location --silent --show-error \
      "https://dl.google.com/android/repository/commandlinetools-linux-${ANDROID_CMDLINE_TOOLS_VERSION}_latest.zip" \
      --output /tmp/android-command-line-tools.zip \
    && echo "${ANDROID_CMDLINE_TOOLS_SHA256}  /tmp/android-command-line-tools.zip" \
      | sha256sum --check --strict \
    && unzip -q /tmp/android-command-line-tools.zip -d "$ANDROID_HOME" \
    && rm /tmp/android-command-line-tools.zip

RUN yes | sdkmanager --sdk_root="$ANDROID_HOME" --licenses >/dev/null

RUN sdkmanager \
      --sdk_root="$ANDROID_HOME" \
      "platform-tools" \
      "platforms;android-${ANDROID_SDK_VERSION}" \
      "build-tools;${ANDROID_BUILD_TOOLS_VERSION}" \
      "ndk;${ANDROID_NDK_VERSION}" \
    && rustup target add aarch64-linux-android \
    && chmod -R a+rX "$ANDROID_HOME" "$RUSTUP_HOME" "$CARGO_HOME"
