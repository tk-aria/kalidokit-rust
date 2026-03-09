#!/bin/sh

set -e

if [ -n "${DEBUG}" ]; then
  set -x
fi

# デフォルト設定
DEFAULT_INSTALL_PATH="/usr/local/bin"
REPO="tk-aria/kalidokit-rust"
BINARY_NAME="kalidokit-rust"

# 最新バージョンを取得
_latest_version() {
  curl -sSLf "https://api.github.com/repos/${REPO}/releases/latest" | \
    grep '"tag_name":' | \
    sed -E 's/.*"([^"]+)".*/\1/'
}

# OS検出
_detect_os() {
  os="$(uname -s)"
  case "$os" in
    Linux) echo "linux" ;;
    Darwin) echo "darwin" ;;
    CYGWIN*|MINGW*|MSYS*) echo "windows" ;;
    *) echo "Unsupported operating system: $os" 1>&2; return 1 ;;
  esac
  unset os
}

# アーキテクチャ検出
_detect_arch() {
  arch="$(uname -m)"
  case "$arch" in
    amd64|x86_64) echo "x86_64" ;;
    arm64|aarch64) echo "aarch64" ;;
    *) echo "Unsupported processor architecture: $arch" 1>&2; return 1 ;;
  esac
  unset arch
}

# ターゲットトリプル生成
_get_target() {
  local os="$1"
  local arch="$2"

  case "$os" in
    linux)   echo "${arch}-unknown-linux-gnu" ;;
    darwin)  echo "aarch64-apple-darwin" ;;  # macOS は aarch64 のみ (Intel は Rosetta 2)
    windows) echo "${arch}-pc-windows-msvc" ;;
  esac
}

# アーカイブ拡張子
_get_ext() {
  local os="$1"
  case "$os" in
    windows) echo "zip" ;;
    *) echo "tar.gz" ;;
  esac
}

# バイナリ名を決定
_get_binary_name() {
  local os="$1"
  case "$os" in
    windows) echo "${BINARY_NAME}.exe" ;;
    *) echo "${BINARY_NAME}" ;;
  esac
}

# ダウンロードURL生成
_download_url() {
  local version="$1"
  local target="$2"
  local ext="$3"

  echo "https://github.com/${REPO}/releases/download/${version}/${BINARY_NAME}-${version}-${target}.${ext}"
}

# インストール実行
main() {
  # バージョン決定
  if [ -z "${KALIDOKIT_VERSION}" ]; then
    echo "Getting latest version..."
    KALIDOKIT_VERSION=$(_latest_version)
    if [ -z "${KALIDOKIT_VERSION}" ]; then
      echo "Failed to get latest version" 1>&2
      return 1
    fi
  fi

  # インストールパス決定
  install_path="${KALIDOKIT_INSTALL_PATH:-$DEFAULT_INSTALL_PATH}"

  # プラットフォーム検出
  detected_os="$(_detect_os)"
  detected_arch="$(_detect_arch)"
  target="$(_get_target "$detected_os" "$detected_arch")"
  ext="$(_get_ext "$detected_os")"
  binary="$(_get_binary_name "$detected_os")"

  # ダウンロードURL生成
  download_url="$(_download_url "$KALIDOKIT_VERSION" "$target" "$ext")"

  echo "Installing ${BINARY_NAME} ${KALIDOKIT_VERSION} for ${detected_os}/${detected_arch} (${target})..."
  echo "Download URL: $download_url"

  # インストールディレクトリ作成
  if [ ! -d "$install_path" ]; then
    echo "Creating install directory: $install_path"
    mkdir -p -- "$install_path"
  fi

  # 一時ディレクトリ作成
  tmp_dir=$(mktemp -d)
  trap 'rm -rf "$tmp_dir"' EXIT

  # アーカイブダウンロード
  echo "Downloading ${BINARY_NAME} archive..."
  if ! curl -sSLf "$download_url" -o "$tmp_dir/archive.${ext}"; then
    echo "Failed to download archive from: $download_url" 1>&2
    echo "Please check if the version ${KALIDOKIT_VERSION} exists and supports ${target}" 1>&2
    return 1
  fi

  # アーカイブ展開
  echo "Extracting archive..."
  case "$ext" in
    tar.gz)
      if ! tar -xzf "$tmp_dir/archive.tar.gz" -C "$tmp_dir"; then
        echo "Failed to extract archive" 1>&2
        return 1
      fi
      ;;
    zip)
      if ! unzip -q "$tmp_dir/archive.zip" -d "$tmp_dir"; then
        echo "Failed to extract archive" 1>&2
        return 1
      fi
      ;;
  esac

  # バイナリを探す (アーカイブ内のディレクトリ構造に対応)
  archive_dir="${BINARY_NAME}-${KALIDOKIT_VERSION}-${target}"
  if [ -f "$tmp_dir/${archive_dir}/${binary}" ]; then
    binary_path="$tmp_dir/${archive_dir}/${binary}"
  elif [ -f "$tmp_dir/${binary}" ]; then
    binary_path="$tmp_dir/${binary}"
  else
    echo "Binary not found in archive" 1>&2
    echo "Expected: ${archive_dir}/${binary}" 1>&2
    return 1
  fi

  # バイナリ配置
  echo "Installing ${BINARY_NAME} to $install_path/$binary"
  if ! cp "$binary_path" "$install_path/$binary"; then
    echo "Failed to install binary. Check permissions for $install_path" 1>&2
    echo "Try: KALIDOKIT_INSTALL_PATH=~/.local/bin sh install.sh" 1>&2
    echo "  or: sudo sh install.sh" 1>&2
    return 1
  fi

  chmod 755 -- "$install_path/$binary"

  # assets ディレクトリがあればコピー
  if [ -d "$tmp_dir/${archive_dir}/assets" ]; then
    assets_dest="$(dirname "$install_path")/../share/${BINARY_NAME}/assets"
    mkdir -p "$assets_dest" 2>/dev/null || true
    if cp -r "$tmp_dir/${archive_dir}/assets" "$assets_dest/../" 2>/dev/null; then
      echo "Assets installed to: $assets_dest"
    fi
  fi

  echo ""
  echo "kalidokit-rust ${KALIDOKIT_VERSION} has been successfully installed!"
  echo ""
  echo "Binary: $install_path/$binary"
  echo ""
  echo "To get started, run:"
  echo "  ${BINARY_NAME} --help"
  echo ""
  echo "For more information, visit: https://github.com/${REPO}"
}

main "$@"
