FROM node:22-bookworm-slim
RUN apt-get update && apt-get install -y --no-install-recommends python3 python3-venv ca-certificates && rm -rf /var/lib/apt/lists/*
WORKDIR /consumer
COPY dist /artifacts
ARG TARGETARCH
RUN mkdir /cli && tar -xzf /artifacts/mcptype-linux-${TARGETARCH}.tar.gz -C /cli && \
    printf '{"private":true,"type":"module"}\n' > package.json && \
    npm install /artifacts/mcptype-0.0.1.tgz typescript@5.9.3 @modelcontextprotocol/server@2.3.1 @modelcontextprotocol/node@2.1.1 zod@4.6.5 && \
    python3 -m venv .venv && \
    arch=$(if [ "$TARGETARCH" = amd64 ]; then echo x86_64; else echo aarch64; fi) && \
    .venv/bin/pip install --only-binary=:all: /artifacts/mcptype-0.0.1-cp311-abi3-linux_${arch}.whl[test]
COPY tests tests
COPY packaging/test-remote.mjs packaging/test-remote.mjs
ENV MCPTYPE_PACKAGE_TEST=1 MCPTYPE_BINARY=/cli/mcptype
RUN ! command -v cargo && ! command -v rustc && ! command -v mcptype && \
    node --test tests/*.test.mjs && .venv/bin/python -m pytest -q tests/test_client.py
CMD ["node", "packaging/test-remote.mjs"]
