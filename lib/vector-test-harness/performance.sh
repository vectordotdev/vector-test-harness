# vi: syntax=bash
# shellcheck shell=bash

BUILD_ROOT_BASE="/tmp/vector-test-harness"

get_base_dir_for_sha() {
    local VECTOR_SHA="$1"
    echo "${BUILD_ROOT_BASE}/${VECTOR_SHA}"
}

get_src_dir_for_sha() {
   local VECTOR_SHA="$1"
   local BUILD_ROOT=$(get_base_dir_for_sha "${VECTOR_SHA}")
   echo "${BUILD_ROOT}/src"
}

get_bin_dir_for_sha() {
   local VECTOR_SHA="$1"
   local BUILD_ROOT=$(get_base_dir_for_sha "${VECTOR_SHA}")
   echo "${BUILD_ROOT}/bin"
}

get_bin_for_sha() {
  local VECTOR_SHA="$1"
  local BIN_DIR=$(get_bin_dir_for_sha "${VECTOR_SHA}")
  echo "${BIN_DIR}/vector"
}

get_results_dir() {
   echo "${BUILD_ROOT_BASE}/results"
}

prepare_dirs_for_sha() {
    local VECTOR_SHA="$1"
    local SRC_DIR=$(get_src_dir_for_sha "${VECTOR_SHA}")
    local BIN_DIR=$(get_bin_dir_for_sha "${VECTOR_SHA}")
    local RESULTS_DIR=$(get_results_dir)

    mkdir -p "${SRC_DIR}"
    mkdir -p "${BIN_DIR}"
    mkdir -p "${RESULTS_DIR}"
}

# Generates the base of a Hyperfine result file.
#
# This file is suitable for direct inclusion in a Github issue/PR comment, and is formatted via
# Markdown.
generate_hyperfine_results() {
  local RESULTS_DIR="$1"
  cat << EOF > "${RESULTS_DIR}/hyperfine.md"
## Hyperfine
These tests check the topline performance a given configuration for both the base and 
comparison versions.  This represents the total time taken to consume all input and 
process it, according to the given configuration.

The \`Relative\` column shows the relative speed differences between the commands.  The faster
command will have a relative value of 1.00, and the slower command(s) will be greater than 1.00.
For example, a value of 1.25 means the command was 25% slower than the fastest command.
EOF
}

# Gets the appropriate path for a Hyperfine result subfile.
get_hyperfine_result_subfile_path() {
  local RESULTS_DIR="$1"
  local RESULTS_NAME="$2"
  echo "${RESULTS_DIR}/hyperfine-${RESULTS_NAME}.md"
}

# Adds a Hyperfine Markdown result subfile to an overall Hyperfine result file.
#
# Expects that the subfile -- which is the output of a single Hyperfine run -- exists in a specific
# location.  Callers can use `get_hyperfine_result_subfile_path` to generate the path that will be
# looked up.
#
# Appends to the given Hyperfine result file in the results directory, which can be initialized by
# calling `generate_hyperfine_results`.
append_hyperfine_result() {
  local RESULTS_DIR="$1"
  local RESULTS_NAME="$2"
  local RESULTS_FILE="${RESULTS_DIR}/hyperfine.md"
  local RESULTS_SUBFILE=$(get_hyperfine_result_subfile_path "${RESULTS_DIR}" "${RESULTS_NAME}")

  echo "### ${RESULTS_NAME}" >> "${RESULTS_FILE}"
  cat "${RESULTS_SUBFILE}" >> "${RESULTS_FILE}"
}

build_vector_sha() {
  local VECTOR_SHA="$1"
  local FEATURES="${2:-""}"
  local SRC_DIR=$(get_src_dir_for_sha "${VECTOR_SHA}")
  local VECTOR_BIN=$(get_bin_for_sha "${VECTOR_SHA}")
  local REPOSITORY_URL="https://github.com/timberio/vector.git"
  local REPOSITORY_BASE_NAME=$(basename -- "${REPOSITORY_URL}")
  local REPOSITORY_NAME="${REPOSITORY_BASE_NAME%.*}"

  # Build out a valid --features if any were passed in.
  if [ ! -z "${FEATURES}"];  then
    FEATURES="--no-default-features --features ${FEATURES}"
  fi

  # Make sure all pertinent directories exist.
  prepare_dirs_for_sha "${VECTOR_SHA}"

  # Checkout the repository if we haven't already.
  if [ ! -d "${SRC_DIR}/${REPOSITORY_NAME}" ]; then
      pushd "${SRC_DIR}"
      git clone "${REPOSITORY_URL}"
      popd
  fi

  # If the binary doesn't already exist, then compile it.
  if [ ! -f "${VECTOR_BIN}" ]; then
    pushd "${SRC_DIR}/${REPOSITORY_NAME}"
    git fetch --all
    git checkout "${VECTOR_SHA}"
    RUSTFLAGS="-g" cargo build --release $FEATURES
    cp target/release/vector "${VECTOR_BIN}"
    popd
  fi
}