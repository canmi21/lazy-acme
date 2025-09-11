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

FROM alpine:3.20
WORKDIR /app

# runtime deps: openssl + certs
# because lego denps on these, lego itself full static, ldd shows no dynamic link, but who know? it didn't work
RUN unset HTTP_PROXY HTTPS_PROXY ALL_PROXY http_proxy https_proxy all_proxy \
  && apk add --no-cache openssl ca-certificates

COPY --from=builder /app/lazy-acme ./lazy-acme
COPY --from=builder /app/lego/bin/lego /usr/bin/lego

CMD ["./lazy-acme"]