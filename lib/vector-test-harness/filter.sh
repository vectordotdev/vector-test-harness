# vi: syntax=bash
# shellcheck shell=bash

glob_filter_array() {
  local GLOB_FILTER="$1"
  local ARRAY_TO_FILTER="$2"
  GLOB_FILTER_ARRAY_RESULT=()

  for ELEMENT in "${ARRAY_TO_FILTER[@]}"; do
    # shellcheck  disable=SC2053
    if [[ "$ELEMENT" == $GLOB_FILTER ]]; then
      GLOB_FILTER_ARRAY_RESULT+=("$ELEMENT")
    fi
  done
}

regex_filter_array() {
  local REGEX_FILTER="$1"
  local ARRAY_TO_FILTER="$2"
  REGEX_FILTER_ARRAY_RESULT=()

  for ELEMENT in "${ARRAY_TO_FILTER[@]}"; do
    if [[ "$ELEMENT" =~ $REGEX_FILTER ]]; then
      REGEX_FILTER_ARRAY_RESULT+=("$ELEMENT")
    fi
  done
}
