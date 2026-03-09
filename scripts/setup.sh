#!/bin/sh

set -e

if [ -n "${DEBUG}" ]; then
  set -x
fi

# デフォルト設定
DEFAULT_INSTALL_PATH="/usr/local/bin"
REPO="tk-aria/kalidokit-rust"
BINARY_NAME="kalidokit-rust"

# ---------- ユーティリティ ----------

_latest_version() {
  curl -sSLf "https://api.github.com/repos/${REPO}/releases/latest" | \
    grep '"tag_name":' | \
    sed -E 's/.*"([^"]+)".*/\1/'
}

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

_detect_arch() {
  arch="$(uname -m)"
  case "$arch" in
    amd64|x86_64) echo "x86_64" ;;
    arm64|aarch64) echo "aarch64" ;;
    *) echo "Unsupported processor architecture: $arch" 1>&2; return 1 ;;
  esac
  unset arch
}

_get_target() {
  _os="$1"
  _arch="$2"
  case "$_os" in
    linux)   echo "${_arch}-unknown-linux-gnu" ;;
    darwin)  echo "aarch64-apple-darwin" ;;
    windows) echo "${_arch}-pc-windows-msvc" ;;
  esac
}

_get_ext() {
  _os="$1"
  case "$_os" in
    windows) echo "zip" ;;
    *) echo "tar.gz" ;;
  esac
}

_get_binary_file() {
  _os="$1"
  case "$_os" in
    windows) echo "${BINARY_NAME}.exe" ;;
    *) echo "${BINARY_NAME}" ;;
  esac
}

_download_url() {
  _version="$1"; _target="$2"; _ext="$3"
  echo "https://github.com/${REPO}/releases/download/${_version}/${BINARY_NAME}-${_version}-${_target}.${_ext}"
}

_resolve_install_path() {
  echo "${KALIDOKIT_INSTALL_PATH:-$DEFAULT_INSTALL_PATH}"
}

# ---------- install サブコマンド ----------

cmd_install() {
  # バージョン決定
  if [ -z "${KALIDOKIT_VERSION}" ]; then
    echo "Getting latest version..."
    KALIDOKIT_VERSION=$(_latest_version)
    if [ -z "${KALIDOKIT_VERSION}" ]; then
      echo "Failed to get latest version" 1>&2
      return 1
    fi
  fi

  install_path="$(_resolve_install_path)"
  detected_os="$(_detect_os)"
  detected_arch="$(_detect_arch)"
  target="$(_get_target "$detected_os" "$detected_arch")"
  ext="$(_get_ext "$detected_os")"
  binary="$(_get_binary_file "$detected_os")"
  download_url="$(_download_url "$KALIDOKIT_VERSION" "$target" "$ext")"

  echo "Installing ${BINARY_NAME} ${KALIDOKIT_VERSION} for ${detected_os}/${detected_arch} (${target})..."
  echo "Download URL: $download_url"

  # インストールディレクトリ作成
  if [ ! -d "$install_path" ]; then
    echo "Creating install directory: $install_path"
    mkdir -p -- "$install_path"
  fi

  # 一時ディレクトリ
  tmp_dir=$(mktemp -d)
  trap 'rm -rf "$tmp_dir"' EXIT

  # ダウンロード
  echo "Downloading..."
  if ! curl -sSLf "$download_url" -o "$tmp_dir/archive.${ext}"; then
    echo "Failed to download from: $download_url" 1>&2
    echo "Check if version ${KALIDOKIT_VERSION} exists for ${target}" 1>&2
    return 1
  fi

  # 展開
  echo "Extracting..."
  case "$ext" in
    tar.gz) tar -xzf "$tmp_dir/archive.tar.gz" -C "$tmp_dir" ;;
    zip)    unzip -q "$tmp_dir/archive.zip" -d "$tmp_dir" ;;
  esac

  # バイナリ検索
  archive_dir="${BINARY_NAME}-${KALIDOKIT_VERSION}-${target}"
  if [ -f "$tmp_dir/${archive_dir}/${binary}" ]; then
    binary_path="$tmp_dir/${archive_dir}/${binary}"
  elif [ -f "$tmp_dir/${binary}" ]; then
    binary_path="$tmp_dir/${binary}"
  else
    echo "Binary not found in archive. Expected: ${archive_dir}/${binary}" 1>&2
    return 1
  fi

  # 配置
  if ! cp "$binary_path" "$install_path/$binary"; then
    echo "Failed to copy binary to $install_path" 1>&2
    echo "Try: KALIDOKIT_INSTALL_PATH=~/.local/bin sh setup.sh install" 1>&2
    echo "  or: sudo sh setup.sh install" 1>&2
    return 1
  fi
  chmod 755 -- "$install_path/$binary"

  # assets
  if [ -d "$tmp_dir/${archive_dir}/assets" ]; then
    assets_dest="$(dirname "$install_path")/../share/${BINARY_NAME}"
    if mkdir -p "$assets_dest" 2>/dev/null; then
      cp -r "$tmp_dir/${archive_dir}/assets" "$assets_dest/" 2>/dev/null && \
        echo "Assets installed to: $assets_dest/assets"
    fi
  fi

  echo ""
  echo "${BINARY_NAME} ${KALIDOKIT_VERSION} installed successfully!"
  echo "  Binary: $install_path/$binary"
  echo ""
  echo "Run '${BINARY_NAME} --help' to get started."
}

# ---------- uninstall サブコマンド ----------

cmd_uninstall() {
  install_path="$(_resolve_install_path)"
  detected_os="$(_detect_os)"
  binary="$(_get_binary_file "$detected_os")"
  binary_full="$install_path/$binary"

  if [ ! -f "$binary_full" ]; then
    echo "${BINARY_NAME} is not installed at $binary_full" 1>&2
    echo "If installed elsewhere, set KALIDOKIT_INSTALL_PATH" 1>&2
    return 1
  fi

  echo "Removing ${BINARY_NAME} from $binary_full ..."
  if ! rm -f "$binary_full"; then
    echo "Failed to remove $binary_full. Try: sudo sh setup.sh uninstall" 1>&2
    return 1
  fi

  # assets 削除
  assets_dir="$(dirname "$install_path")/../share/${BINARY_NAME}"
  if [ -d "$assets_dir" ]; then
    echo "Removing assets from $assets_dir ..."
    rm -rf "$assets_dir"
  fi

  echo ""
  echo "${BINARY_NAME} has been uninstalled."
}

# ---------- usage ----------

usage() {
  cat <<EOF
Usage: setup.sh <command> [options]

Commands:
  install     Download and install ${BINARY_NAME}
  uninstall   Remove ${BINARY_NAME}

Environment variables:
  KALIDOKIT_VERSION       Version to install (default: latest)
  KALIDOKIT_INSTALL_PATH  Install directory (default: /usr/local/bin)
  DEBUG                   Enable verbose output

Examples:
  # Install latest
  curl -sSLf https://raw.githubusercontent.com/tk-aria/kalidokit-rust/main/scripts/setup.sh | sh -s install

  # Install specific version
  curl -sSLf https://raw.githubusercontent.com/tk-aria/kalidokit-rust/main/scripts/setup.sh | KALIDOKIT_VERSION=v0.1.0 sh -s install

  # Install to custom path
  curl -sSLf https://raw.githubusercontent.com/tk-aria/kalidokit-rust/main/scripts/setup.sh | KALIDOKIT_INSTALL_PATH=~/.local/bin sh -s install

  # Uninstall
  curl -sSLf https://raw.githubusercontent.com/tk-aria/kalidokit-rust/main/scripts/setup.sh | sh -s uninstall
EOF
}

# ---------- エントリポイント ----------

main() {
  command="${1:-}"

  case "$command" in
    install)   cmd_install ;;
    uninstall) cmd_uninstall ;;
    -h|--help|help) usage ;;
    "")
      # サブコマンドなし → デフォルトで install (後方互換)
      cmd_install
      ;;
    *)
      echo "Unknown command: $command" 1>&2
      echo "" 1>&2
      usage 1>&2
      return 1
      ;;
  esac
}

main "$@"
