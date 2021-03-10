#!/usr/bin/env bash

set -o errexit
set -o pipefail
set -o nounset
# set -o xtrace

__dir="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
BIN="${__dir}/bin"
BUILD="${__dir}/.build"
VECTOR_SHA="0327d46a9d213b5390e11d86b920224bdf91ed38"
VECTOR_BIN="${__dir}/bin/vector_gen-${VECTOR_SHA}"
REPO="https://github.com/timberio/vector.git"
REPO_DOT_GIT=$(basename -- "${REPO}")
REPO_NAME="${REPO_DOT_GIT%.*}"

mkdir -p "${__dir}/bin"
mkdir -p "${BUILD}"

if [ ! -d "${BUILD}/${REPO_NAME}" ]; then
    pushd "${BUILD}"
    git clone "${REPO}"
    popd
fi

if [ ! -f "${VECTOR_BIN}" ]; then
    pushd ~/projects/com/datadog/vector
    git checkout ${VECTOR_SHA}
    RUSTFLAGS="-g" cargo build --no-default-features --features "sources-file,sources-internal_metrics,sinks-prometheus,sinks-blackhole" --release
    cp target/release/vector "${VECTOR_BIN}"
    popd
fi

exec flamegraph "${VECTOR_BIN}" --config vector.toml
