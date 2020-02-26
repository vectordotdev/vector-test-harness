# vi: syntax=bash
# shellcheck shell=bash

ANSIBLE_CONFIG_DEFAULT_OVERRIDE="$VECTOR_TEST_HARNESS_ANSIBLE_DIR/ansible.cfg"

prepare_ansible_config() {
  local VERBOSE="${1:-"false"}"

  if [[ -z "${ANSIBLE_CONFIG:-""}" ]]; then
    ANSIBLE_CONFIG="$ANSIBLE_CONFIG_DEFAULT_OVERRIDE"

    if [[ "$VERBOSE" != "false" ]]; then
      echo "ANSIBLE_CONFIG not set, defaulting to $ANSIBLE_CONFIG"
    fi
  else
    if [[ "$VERBOSE" != "false" ]]; then
      echo "ANSIBLE_CONFIG set to $ANSIBLE_CONFIG"
    fi
  fi

  export ANSIBLE_CONFIG
}

prepare_ansible_extra_args_array() {
  local VERBOSE="${1:-"false"}"

  ANSIBLE_EXTRA_ARGS_ARRAY=()

  local ANSIBLE_EXTRA_ARGS_STRING="${ANSIBLE_EXTRA_ARGS:-""}"
  if [[ -n "$ANSIBLE_EXTRA_ARGS_STRING" ]]; then
    declare -a "ANSIBLE_EXTRA_ARGS_ARRAY_LOCAL=($ANSIBLE_EXTRA_ARGS_STRING)"
    ANSIBLE_EXTRA_ARGS_ARRAY=("${ANSIBLE_EXTRA_ARGS_ARRAY_LOCAL[@]}")

    if [[ "$VERBOSE" != "false" ]]; then
      echo "Extra ansible arguments: ${ANSIBLE_EXTRA_ARGS_ARRAY[*]}"
    fi
  fi
}

prepare_ansible_version_vars() {
  local COMMA_SEPARATED_LIST="$1"

  ANSIBLE_VERSION_VARS=""

  while read -d, -r PAIR; do
    IFS='=' read -r SUBJECT VERSION <<<"$PAIR"
    ANSIBLE_VERSION_VARS="$ANSIBLE_VERSION_VARS ${SUBJECT}_version=$VERSION"
  done <<<"$COMMA_SEPARATED_LIST,"
}
