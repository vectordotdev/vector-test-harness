#!/usr/bin/env bash

set -o errexit
set -o pipefail
set -o nounset
# set -o xtrace

__dir="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
BUILD="${__dir}/.build"
OUT="${__dir}/out"
VECTOR_BASE_SHA="6c0df5614a10c4ef7d8c5e18d3bba289c6a7a0b5"
VECTOR_COMP_SHA="cb12351ba2a039db1ea962ab75f990b3d149f37e"

VECTOR_BASE_BIN="${__dir}/bin/vector-${VECTOR_BASE_SHA}"
VECTOR_COMP_BIN="${__dir}/bin/vector-${VECTOR_COMP_SHA}"

REPO="https://github.com/timberio/vector.git"
REPO_DOT_GIT=$(basename -- "${REPO}")
REPO_NAME="${REPO_DOT_GIT%.*}"

mkdir -p "${__dir}/bin"
mkdir -p "${BUILD}"
mkdir -p "${OUT}"

if [ ! -d "${BUILD}/${REPO_NAME}" ]; then
    pushd "${BUILD}"
    git clone "${REPO}"
    popd
fi

if [ ! -f "${VECTOR_BASE_BIN}" ]; then
    pushd "${BUILD}/${REPO_NAME}"
    git fetch --all
    git checkout ${VECTOR_BASE_SHA}
    RUSTFLAGS="-g" cargo build --release
    cp target/release/vector "${VECTOR_BASE_BIN}"
    popd
fi

if [ ! -f "${VECTOR_COMP_BIN}" ]; then
    pushd "${BUILD}/${REPO_NAME}"
    git fetch --all
    git checkout ${VECTOR_COMP_SHA}
    RUSTFLAGS="-g" cargo build --release
    cp target/release/vector "${VECTOR_COMP_BIN}"
    popd
fi

CONFIGS="${__dir}/configs/*"

figlet "hyperfine"
for config in $CONFIGS
do
    echo "PROCESSING: $(basename "${config}")"
    #
    # Topline stable numbers
    #
    hyperfine "zcat baked.log.gz | ${VECTOR_BASE_BIN} -qq --config ${config}" \
              "zcat baked.log.gz | ${VECTOR_COMP_BIN} -qq --config ${config}"
done

figlet "perf / flamegraph"
for config in $CONFIGS
do
    NAME="$(basename "${config}" | cut -f 1 -d '.')"

    BASE_PERF="${OUT}/perf-${VECTOR_BASE_SHA}-${NAME}.data"
    BASE_FOLDED="${OUT}/stacks-${VECTOR_BASE_SHA}-${NAME}.folded"
    BASE_SVG="${OUT}/${VECTOR_BASE_SHA}-${NAME}.svg"

    COMP_PERF="${OUT}/perf-${VECTOR_COMP_SHA}-${NAME}.data"
    COMP_FOLDED="${OUT}/stacks-${VECTOR_COMP_SHA}-${NAME}.folded"
    COMP_SVG="${OUT}/${VECTOR_COMP_SHA}-${NAME}.svg"

    #
    # Perf / Flamegraphs
    #
    zcat baked.log.gz | \
        perf record --call-graph dwarf "${VECTOR_BASE_BIN}" -qq --config "${config}" && \
        mv perf.data "${BASE_PERF}"

    zcat baked.log.gz | \
        perf record --call-graph dwarf "${VECTOR_COMP_BIN}" -qq --config "${config}" && \
        mv perf.data "${COMP_PERF}"

    perf script --input="${BASE_PERF}" | inferno-collapse-perf > "${BASE_FOLDED}"
    perf script --input="${COMP_PERF}" | inferno-collapse-perf > "${COMP_FOLDED}"

    inferno-flamegraph < "${BASE_FOLDED}" > "${BASE_SVG}"
    inferno-flamegraph < "${COMP_FOLDED}" > "${COMP_SVG}"

    inferno-diff-folded "${BASE_FOLDED}" "${COMP_FOLDED}" | inferno-flamegraph > "${OUT}/diff-${VECTOR_BASE_SHA}-${VECTOR_COMP_SHA}.svg"
    inferno-diff-folded "${COMP_FOLDED}" "${BASE_FOLDED}" | inferno-flamegraph --negate > "${OUT}/diff-${VECTOR_COMP_SHA}-${VECTOR_BASE_SHA}.svg"
done

figlet "massif"
for config in $CONFIGS
do
    NAME="$(basename "${config}" | cut -f 1 -d '.')"
    BASE_MASSIF="${OUT}/massif.out.${NAME}-${VECTOR_BASE_SHA}"
    COMP_MASSIF="${OUT}/massif.out.${NAME}-${VECTOR_COMP_SHA}"

    #
    # Valgrind
    #
    zcat baked.log.gz | \
        valgrind --tool=massif --massif-out-file="${BASE_MASSIF}" "${VECTOR_BASE_BIN}" -qq --config "${config}"
    zcat baked.log.gz | \
        valgrind --quiet --tool=massif --massif-out-file="${COMP_MASSIF}" "${VECTOR_COMP_BIN}" -qq --config "${config}"
done
