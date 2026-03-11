#!/bin/sh
set -eu

echo "=== Claude Code 설치 ==="
echo ""

# Detect OS
OS=$(uname -s)
case "$OS" in
  Darwin)       os="macos" ;;
  Linux)
    if command -v apt >/dev/null 2>&1; then os="linux-apt"
    elif command -v dnf >/dev/null 2>&1; then os="linux-dnf"
    else os="linux-unknown"
    fi ;;
  MINGW*|MSYS*)
    echo "Windows에서는 PowerShell을 사용하세요:"
    echo ""
    echo "  irm baro-sync.com/install-claude.ps1 | iex"
    echo ""
    exit 1 ;;
  *)
    echo "Error: 지원하지 않는 OS입니다: $OS"
    exit 1 ;;
esac

echo "  OS: $os"
echo ""

# --- 1. Homebrew (macOS only) ---
if [ "$os" = "macos" ]; then
  if ! command -v brew >/dev/null 2>&1; then
    echo "[1/3] Homebrew 설치 중..."
    /bin/bash -c "$(curl -fsSL https://raw.githubusercontent.com/Homebrew/install/HEAD/install.sh)"
    # Apple Silicon PATH
    if [ -f /opt/homebrew/bin/brew ]; then
      eval "$(/opt/homebrew/bin/brew shellenv)"
    fi
    echo "  Homebrew: 설치 완료"
  else
    echo "[1/3] Homebrew: OK"
  fi
else
  echo "[1/3] Homebrew: 건너뜀 (Linux)"
fi

# --- 2. Node.js ---
echo ""
if ! command -v node >/dev/null 2>&1; then
  echo "[2/3] Node.js 설치 중..."
  case "$os" in
    macos)         brew install node ;;
    linux-apt)     sudo apt update -qq && sudo apt install -y nodejs npm ;;
    linux-dnf)     sudo dnf install -y nodejs npm ;;
    *)             echo "  Node.js를 수동으로 설치하세요: https://nodejs.org" && exit 1 ;;
  esac
  echo "  Node.js: 설치 완료"
else
  echo "[2/3] Node.js: OK ($(node --version))"
fi

# --- 3. Claude Code ---
echo ""
echo "[3/3] Claude Code 설치 중..."
npm install -g @anthropic-ai/claude-code
echo "  Claude Code: 설치 완료"

# --- Done ---
echo ""
echo "=== 설치 완료! ==="
echo ""
echo "터미널에서 'claude' 를 실행하여 로그인하세요."
if [ "$os" = "macos" ]; then
  echo ""
  echo "추천 터미널: iTerm2 (True Color 지원)"
  echo "  brew install --cask iterm2"
fi
echo ""
