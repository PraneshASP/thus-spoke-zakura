#!/bin/sh
set -eu

repo="zakura-core/thus-spoke-zakura"
version="${TSZ_VERSION:-latest}"
case "$(uname -s)-$(uname -m)" in
  Linux-x86_64) target="x86_64-unknown-linux-gnu" ;;
  Linux-aarch64|Linux-arm64) target="aarch64-unknown-linux-gnu" ;;
  Darwin-x86_64) target="x86_64-apple-darwin" ;;
  Darwin-arm64) target="aarch64-apple-darwin" ;;
  *) echo "Unsupported platform: $(uname -s) $(uname -m)" >&2; exit 1 ;;
esac

if [ "$version" = latest ]; then
  url="https://github.com/$repo/releases/latest/download/thus-spoke-zakura-$target.tar.gz"
else
  url="https://github.com/$repo/releases/download/$version/thus-spoke-zakura-$target.tar.gz"
fi
destination="${TSZ_INSTALL_DIR:-$HOME/.local/bin}"
mkdir -p "$destination"
archive="$(mktemp -t thus-spoke-zakura.XXXXXX)"
trap 'rm -f "$archive"' EXIT
curl --proto '=https' --tlsv1.2 -fsSL "$url" -o "$archive"
tar -xzf "$archive" -C "$destination" thus-spoke-zakura
chmod 755 "$destination/thus-spoke-zakura"
echo "Installed thus-spoke-zakura to $destination"
