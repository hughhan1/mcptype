FROM --platform=$BUILDPLATFORM rust:1.99.1-slim-bookworm AS build
RUN apt-get update && apt-get install -y --no-install-recommends python3 python3-venv patchelf gcc-x86-64-linux-gnu gcc-aarch64-linux-gnu libc6-dev-amd64-cross libc6-dev-arm64-cross && rm -rf /var/lib/apt/lists/*
RUN python3 -m venv /buildenv && /buildenv/bin/pip install "maturin>=1.9,<2"
WORKDIR /src
COPY . .
ARG TARGETARCH
ENV CARGO_TARGET_X86_64_UNKNOWN_LINUX_GNU_LINKER=x86_64-linux-gnu-gcc CARGO_TARGET_AARCH64_UNKNOWN_LINUX_GNU_LINKER=aarch64-linux-gnu-gcc
ENV CC_x86_64_unknown_linux_gnu=x86_64-linux-gnu-gcc CC_aarch64_unknown_linux_gnu=aarch64-linux-gnu-gcc
RUN --mount=type=cache,target=/usr/local/cargo/registry --mount=type=cache,target=/src/target,id=mcptype-target-${TARGETARCH} \
    arch=$(if [ "$TARGETARCH" = amd64 ]; then echo x86_64; else echo aarch64; fi) && \
    target=${arch}-unknown-linux-gnu && rustup target add "$target" && \
    cargo build --locked --release --target "$target" -p mcptype -p mcptype-node && \
    PYO3_CROSS=1 /buildenv/bin/maturin build --locked --release --target "$target" --compatibility linux --out /out && \
    mkdir -p /out/native && cp target/$target/release/libmcptype_node.so /out/native/linux-$(if [ "$TARGETARCH" = amd64 ]; then echo x64; else echo arm64; fi).node && \
    tar -czf /out/mcptype-linux-${TARGETARCH}.tar.gz -C target/$target/release mcptype
FROM scratch
COPY --from=build /out/ /
