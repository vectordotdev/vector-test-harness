#!/usr/bin/env bash
set -o errexit
set -o pipefail
set -o nounset

__self_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
source ${__self_dir}/../../../lib/vector-test-harness/performance.sh

VECTOR_BASE_SHA="$1"
VECTOR_COMP_SHA="$2"
PERF_SAMPLE_HZ=${PERF_SAMPLE_HZ:-"499"}

# Get all our directories and inputs sorted out.
CONFIGS="${__self_dir}/configs/*"
RESULTS_DIR=$(get_results_dir)
INPUT_FILE="${__self_dir}/baked-small.log.gz"

prepare_dirs_for_sha "${VECTOR_BASE_SHA}"
prepare_dirs_for_sha "${VECTOR_COMP_SHA}"

# Get and build both the base and comparison version of Vector.
build_vector_sha "${VECTOR_BASE_SHA}"
VECTOR_BASE_BIN=$(get_bin_for_sha "${VECTOR_BASE_SHA}")
build_vector_sha "${VECTOR_COMP_SHA}"
VECTOR_COMP_BIN=$(get_bin_for_sha "${VECTOR_COMP_SHA}")

# Head-to-head runs utilizing the different configurations via hyperfine. This gets us
# the topline numbers for which SHA is completing the tests fastest.
generate_hyperfine_results_template "${RESULTS_DIR}"

figlet "hyperfine"
for config in $CONFIGS
do
    base_config=$(basename "${config}")
    markdown_path=$(get_hyperfine_result_subfile_path "${RESULTS_DIR}" "${base_config}")

    hyperfine "zcat ${INPUT_FILE} | ${VECTOR_BASE_BIN} -qq --config ${config}" \
              "zcat ${INPUT_FILE} | ${VECTOR_COMP_BIN} -qq --config ${config}" \
              --command-name "${base_config} (base)" \
              --command-name "${base_config} (comparison)" \
              --export-markdown "${markdown_path}"

    append_hyperfine_result "${RESULTS_DIR}" "${base_config}"
done
echo "Finished with hyperfine."

# Now do the same thing, but run then binary under `perf` so we can generate flamegraphs.
# figlet "perf / flamegraph"
# for config in $CONFIGS
# do
#     NAME="$(basename "${config}" | cut -f 1 -d '.')"

#     BASE_PERF="${RESULTS_DIR}/perf-${VECTOR_BASE_SHA}-${NAME}.data"
#     BASE_FOLDED="${RESULTS_DIR}/stacks-${VECTOR_BASE_SHA}-${NAME}.folded"
#     BASE_SVG="${RESULTS_DIR}/${VECTOR_BASE_SHA}-${NAME}.svg"

#     COMP_PERF="${RESULTS_DIR}/perf-${VECTOR_COMP_SHA}-${NAME}.data"
#     COMP_FOLDED="${RESULTS_DIR}/stacks-${VECTOR_COMP_SHA}-${NAME}.folded"
#     COMP_SVG="${RESULTS_DIR}/${VECTOR_COMP_SHA}-${NAME}.svg"

#     #
#     # Perf / Flamegraphs
#     #
#     echo "Running perf baseline..."
#     zcat ${INPUT_FILE} | \
#         sudo perf record --call-graph dwarf -F "${PERF_SAMPLE_HZ}" "${VECTOR_BASE_BIN}" -qq --config "${config}" && \
#         sudo mv perf.data "${BASE_PERF}"

#     echo "Running perf comparison..."
#     zcat ${INPUT_FILE} | \
#         sudo perf record --call-graph dwarf -F "${PERF_SAMPLE_HZ}" "${VECTOR_COMP_BIN}" -qq --config "${config}" && \
#         sudo mv perf.data "${COMP_PERF}"

#     echo "Transforming raw perf results..."
#     sudo perf script --input="${BASE_PERF}" | inferno-collapse-perf > "${BASE_FOLDED}"
#     sudo perf script --input="${COMP_PERF}" | inferno-collapse-perf > "${COMP_FOLDED}"

#     echo "Generating baseline/comparison flamegraph SVGs..."
#     inferno-flamegraph < "${BASE_FOLDED}" > "${BASE_SVG}"
#     inferno-flamegraph < "${COMP_FOLDED}" > "${COMP_SVG}"

#     echo "Generating mirrored difference flamegraph SVGs..."
#     inferno-diff-folded "${BASE_FOLDED}" "${COMP_FOLDED}" | inferno-flamegraph > "${RESULTS_DIR}/diff-${VECTOR_BASE_SHA}-${VECTOR_COMP_SHA}.svg"
#     inferno-diff-folded "${COMP_FOLDED}" "${BASE_FOLDED}" | inferno-flamegraph --negate > "${RESULTS_DIR}/diff-${VECTOR_COMP_SHA}-${VECTOR_BASE_SHA}.svg"

#     echo "Finished with perf/flamegraph."
# done

# # Now run through massif to check out memory usage / growth.
# figlet "massif"
# for config in $CONFIGS
# do
#     NAME="$(basename "${config}" | cut -f 1 -d '.')"
#     BASE_MASSIF="${RESULTS_DIR}/massif.out.${NAME}-${VECTOR_BASE_SHA}"
#     COMP_MASSIF="${RESULTS_DIR}/massif.out.${NAME}-${VECTOR_COMP_SHA}"

#     #
#     # Valgrind
#     #
#     echo "Running massif baseline..."
#     zcat ${INPUT_FILE} | \
#         valgrind --quiet --tool=massif --massif-out-file="${BASE_MASSIF}" "${VECTOR_BASE_BIN}" -qq --config "${config}"
#     echo "Running massif comparison..."
#     zcat ${INPUT_FILE} | \
#         valgrind --quiet --tool=massif --massif-out-file="${COMP_MASSIF}" "${VECTOR_COMP_BIN}" -qq --config "${config}"
#     echo "Finished with massif."
# done
