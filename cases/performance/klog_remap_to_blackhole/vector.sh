#!/usr/bin/env bash

set -o errexit
set -o pipefail
set -o nounset
set -o xtrace

__dir="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
BIN="${__dir}/bin"
BUILD="${__dir}/.build"
VECTOR_BASE_SHA="f1ea629fb1d751bbb073597e6fe17887cd6a0098"
VECTOR_COMP_SHA="043af0866d054b74d4cd69d3641db36c8a34d8d9"

VECTOR_BASE_BIN="${__dir}/bin/vector-${VECTOR_BASE_SHA}"
VECTOR_COMP_BIN="${__dir}/bin/vector-${VECTOR_COMP_SHA}"

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

if [ ! -f "${VECTOR_COMP_BIN}" ]; then
    pushd "${BUILD}/${REPO_NAME}"
    git fetch --all
    git checkout ${VECTOR_COMP_SHA}
    # RUSTFLAGS="-g" cargo build --no-default-features --features "sources-stdin,sources-internal_metrics,transforms-json_parser,transforms-regex_parser,transforms-add_fields,transforms-coercer,transforms-remap,transforms-reduce,transforms-route,sinks-prometheus,sinks-blackhole" --release
    RUSTFLAGS="-g" cargo build --release
    cp target/release/vector "${VECTOR_COMP_BIN}"
    popd
fi

perf stat --repeat=10 bash -c "zcat baked.log.gz | ${VECTOR_BASE_BIN} -qq --config vector-nohop.toml"
perf stat --repeat=10 bash -c "zcat baked.log.gz | ${VECTOR_COMP_BIN} -qq --config vector-nohop.toml"

zcat baked.log.gz | perf record --call-graph dwarf "${VECTOR_BASE_BIN}" -qq --config vector-nohop.toml && mv perf.data perf-base-nohop.data
zcat baked.log.gz | perf record --call-graph dwarf "${VECTOR_COMP_BIN}" -qq --config vector-nohop.toml && mv perf.data perf-comp-nohop.data

perf script --input=perf-base-nohop.data | inferno-collapse-perf > stacks-base-nohop.folded
perf script --input=perf-comp-nohop.data | inferno-collapse-perf > stacks-comp-nohop.folded

inferno-flamegraph < stacks-base-nohop.folded > flamegraph-base-nohop.svg
inferno-flamegraph < stacks-comp-nohop.folded > flamegraph-comp-nohop.svg

inferno-diff-folded stacks-base-nohop.folded stacks-comp-nohop.folded | inferno-flamegraph > diff.svg
