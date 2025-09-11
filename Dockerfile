FROM rustlang/rust:nightly-slim AS builder

ARG TARGETARCH
WORKDIR /app
COPY . .

RUN unset HTTP_PROXY HTTPS_PROXY ALL_PROXY http_proxy https_proxy all_proxy \
  && apt-get update && apt-get install -y musl-tools pkg-config libssl-dev curl wget bash jq tar \
  && bash ./build.sh "$TARGETARCH" \
  && case "$TARGETARCH" in \
        "amd64") rustup target add x86_64-unknown-linux-musl \
                && cargo build --release --target x86_64-unknown-linux-musl \
                && cp target/x86_64-unknown-linux-musl/release/lazy-acme /app/lazy-acme ;; \
        "arm64") rustup target add aarch64-unknown-linux-musl \
                && cargo build --release --target aarch64-unknown-linux-musl \
                && cp target/aarch64-unknown-linux-musl/release/lazy-acme /app/lazy-acme ;; \
      esac

FROM scratch
WORKDIR /app
COPY --from=builder /app/lazy-acme ./lazy-acme
COPY --from=builder /app/lego/bin/lego /usr/bin/lego

CMD ["./lazy-acme"]
