#!/usr/bin/env bash
# 下載建置資料檔與準確率測試所需的上游資料，並固定在特定版本。
# 需要 git 與 git-lfs。
set -euo pipefail
cd "$(dirname "$0")/../data"

CHEWING_REV=1164cb4ed5fc498ae2f87b40e8b5a35a7afd1e8e
CC0_REV=b473f2a3ee52920dce26d86cc02117479e257c18

fetch() { # <目錄> <網址> <版本>
  if [ ! -d "$1/.git" ]; then
    git init -q "$1"
    git -C "$1" remote add origin "$2"
  fi
  git -C "$1" fetch -q --depth 1 origin "$3"
  git -C "$1" checkout -q FETCH_HEAD
}

fetch upstream https://codeberg.org/chewing/libchewing-data.git "$CHEWING_REV"
git -C upstream lfs pull
fetch cc0-sentences https://github.com/moztw/cc0-sentences.git "$CC0_REV"
echo "完成：data/upstream 與 data/cc0-sentences"
