#!/bin/sh
set -eu

repo="zcashlabs/thus-spoke-zakura"
requested_version="${TSZ_VERSION:-latest}"
case "$(uname -s)-$(uname -m)" in
  Linux-x86_64) target="x86_64-unknown-linux-gnu" ;;
  Linux-aarch64|Linux-arm64) target="aarch64-unknown-linux-gnu" ;;
  Darwin-x86_64) target="x86_64-apple-darwin" ;;
  Darwin-arm64) target="aarch64-apple-darwin" ;;
  *) echo "Unsupported platform: $(uname -s) $(uname -m)" >&2; exit 1 ;;
esac

asset="thus-spoke-zakura-$target.tar.gz"
if [ "$requested_version" = "latest" ]; then
  release_url="https://github.com/$repo/releases/latest/download"
else
  case "$requested_version" in
    v*) tag="$requested_version" ;;
    *) tag="v$requested_version" ;;
  esac
  release_url="https://github.com/$repo/releases/download/$tag"
fi

destination="${TSZ_INSTALL_DIR:-$HOME/.local/bin}"
mkdir -p "$destination"
stage="$(mktemp -d "$destination/.thus-spoke-zakura.XXXXXX")"
trap 'rm -rf "$stage"' EXIT HUP INT TERM

curl --proto '=https' --tlsv1.2 -fsSL "$release_url/$asset" -o "$stage/$asset"
curl --proto '=https' --tlsv1.2 -fsSL "$release_url/SHA256SUMS" -o "$stage/SHA256SUMS"

expected="$(awk -v asset="$asset" '
  { name=$2; sub(/^\*/, "", name); if (name == asset) print $1 }
' "$stage/SHA256SUMS")"
[ -n "$expected" ] || { echo "SHA256SUMS does not contain $asset" >&2; exit 1; }

if command -v sha256sum >/dev/null 2>&1; then
  actual="$(sha256sum "$stage/$asset" | awk '{print $1}')"
elif command -v shasum >/dev/null 2>&1; then
  actual="$(shasum -a 256 "$stage/$asset" | awk '{print $1}')"
else
  echo "A SHA-256 utility (sha256sum or shasum) is required" >&2
  exit 1
fi
[ "$actual" = "$expected" ] || { echo "Checksum verification failed for $asset" >&2; exit 1; }

members="$(tar -tzf "$stage/$asset")"
[ "$members" = "thus-spoke-zakura" ] || {
  echo "Release archive contains unexpected files" >&2
  exit 1
}
tar -xzf "$stage/$asset" -C "$stage" thus-spoke-zakura
chmod 755 "$stage/thus-spoke-zakura"

installed_version="$("$stage/thus-spoke-zakura" --version | awk '{print $2}')"
[ -n "$installed_version" ] || { echo "Could not read launcher version" >&2; exit 1; }
if [ "$requested_version" != "latest" ]; then
  expected_version="${requested_version#v}"
  [ "$installed_version" = "$expected_version" ] || {
    echo "Downloaded launcher is $installed_version, expected $expected_version" >&2
    exit 1
  }
fi

if [ "${TSZ_SKIP_IMAGE_PULL:-0}" != "1" ]; then
  "$stage/thus-spoke-zakura" pull
fi

mv -f "$stage/thus-spoke-zakura" "$destination/thus-spoke-zakura"
trap - EXIT HUP INT TERM
rm -rf "$stage"
echo "Installed thus-spoke-zakura $installed_version to $destination"
