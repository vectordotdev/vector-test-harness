#!/usr/bin/env bash

set -o errexit
set -o pipefail
set -o nounset
# set -o xtrace

__dir="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
BIN="${__dir}/bin"
BUILD="${__dir}/.build"
VECTOR_SHA="6193e68e5b3e50b7e5469d6a8699e25d07bcf57e"
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
    RUSTFLAGS="-g" cargo build --no-default-features --features "sources-stdin,sources-internal_metrics,transforms-json_parser,transforms-regex_parser,transforms-add_fields,transforms-coercer,transforms-remap,transforms-reduce,transforms-route,sinks-prometheus,sinks-blackhole" --release
    cp target/release/vector "${VECTOR_BIN}"
    popd
fi

perf stat --repeat=10 bash -c "zcat baked.log.gz | ${VECTOR_BIN} -qq --config vector-multihop.toml"

perf stat --repeat=10 bash -c "zcat baked.log.gz | ${VECTOR_BIN} -qq --config vector-nohop.toml"
