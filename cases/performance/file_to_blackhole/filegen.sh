#!/usr/bin/env bash

set -o errexit
set -o pipefail
set -o nounset
# set -o xtrace

__dir="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
BIN="${__dir}/bin"
BUILD="${__dir}/.build"
FILEGEN_SHA="9e13b40919109e1737ba5388ef9eeb07d0230608"
FILEGEN_BIN="${BIN}/file_gen-${FILEGEN_SHA}"
REPO="https://github.com/blt/file_gen.git"
REPO_DOT_GIT=$(basename -- "${REPO}")
REPO_NAME="${REPO_DOT_GIT%.*}"

mkdir -p "${BIN}"
mkdir -p "${BUILD}"

if [ ! -d "${BUILD}/${REPO_NAME}" ]; then
    pushd "${BUILD}"
    git clone "${REPO}"
    popd
fi

if [ ! -f "${FILEGEN_BIN}" ]; then
    pushd "${BUILD}/${REPO_NAME}"
    git fetch --all
    git checkout ${FILEGEN_SHA}
    RUSTFLAGS="-g" cargo build --no-default-features --release
    cp target/release/file_gen "${FILEGEN_BIN}"
    popd
fi

rm  logs/*.log || true
exec "${FILEGEN_BIN}" --config-path filegen.toml
