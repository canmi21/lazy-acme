#!/usr/bin/env bash
set -euo pipefail

JSON=$(curl -sS https://api.github.com/repos/go-acme/lego/releases/latest)
TAG=$(printf '%s' "$JSON" | jq -r .tag_name)
if [ -z "$TAG" ] || [ "$TAG" = "null" ]; then
  printf 'failed to get latest tag\n' >&2
  exit 1
fi

declare -A URL
for ARCH in amd64 arm64; do
  URL[$ARCH]=$(printf '%s' "$JSON" \
    | jq -r --arg arch "$ARCH" '.assets[] | select(.name | test("linux_" + $arch)) | .browser_download_url' \
    | head -n1)
done

for ARCH in amd64 arm64; do
  if [ -z "${URL[$ARCH]}" ]; then
    V1="lego_${TAG}_linux_${ARCH}.tar.gz"
    V2="lego_${TAG#v}_linux_${ARCH}.tar.gz"
    URL[$ARCH]="https://github.com/go-acme/lego/releases/download/${TAG}/${V1}"
  fi
done

rm -rf lego
mkdir -p lego/bin
for ARCH in amd64 arm64; do
  OUT="lego/lego_${ARCH}.tar.gz"
  TMPDIR="lego/tmp_${ARCH}"
  mkdir -p "$TMPDIR"
  wget -q -O "$OUT" "${URL[$ARCH]}"
  tar -xzf "$OUT" -C "$TMPDIR"
  BIN=$(find "$TMPDIR" -type f -name 'lego' -print -quit)
  if [ -z "$BIN" ]; then
    BIN=$(find "$TMPDIR" -type f -perm /111 -print -quit)
  fi
  if [ -z "$BIN" ]; then
    BIN=$(find "$TMPDIR" -type f -print -quit)
  fi
  if [ -z "$BIN" ]; then
    printf 'failed to extract lego binary for %s\n' "$ARCH" >&2
    exit 1
  fi
  mkdir -p "lego/bin/${ARCH}"
  mv "$BIN" "lego/bin/${ARCH}/lego"
  chmod +x "lego/bin/${ARCH}/lego"
  rm -rf "$TMPDIR" "$OUT"
done

TARGET="${1:-}"
if [ -z "$TARGET" ]; then
  case "$(uname -m)" in
    x86_64) TARGET=amd64 ;;
    aarch64) TARGET=arm64 ;;
  esac
fi

if [ -n "$TARGET" ]; then
  if [ -f "lego/bin/${TARGET}/lego" ]; then
    cp "lego/bin/${TARGET}/lego" lego/bin/lego
    chmod +x lego/bin/lego
  else
    printf 'no lego binary for target: %s\n' "$TARGET" >&2
    exit 1
  fi
fi

printf 'lego release %s downloaded. selected: %s\n' "$TAG" "${TARGET:-none}"
