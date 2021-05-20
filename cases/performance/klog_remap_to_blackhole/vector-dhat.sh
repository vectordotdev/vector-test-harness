#!/usr/bin/env bash

set -o errexit
set -o pipefail
set -o nounset
set -o xtrace

__dir="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
BIN="${__dir}/bin"
BUILD="${__dir}/.build"
VECTOR_BASE_SHA="f1ea629fb1d751bbb073597e6fe17887cd6a0098"
VECTOR_BASE_BIN="${__dir}/bin/vector-${VECTOR_BASE_SHA}"

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

if [ ! -f "${VECTOR_BASE_BIN}" ]; then
    pushd "${BUILD}/${REPO_NAME}"
    git fetch --all
    git checkout ${VECTOR_BASE_SHA}
    RUSTFLAGS="-g" cargo build --no-default-features --features "sources-stdin,sources-internal_metrics,transforms-json_parser,transforms-regex_parser,transforms-add_fields,transforms-coercer,transforms-remap,transforms-reduce,transforms-route,sinks-prometheus,sinks-blackhole" --release
    cp target/release/vector "${VECTOR_BASE_BIN}"
    popd
fi

zcat baked.log.gz | valgrind --tool=dhat "${VECTOR_BASE_BIN}" -qq --config vector-nohop.toml
