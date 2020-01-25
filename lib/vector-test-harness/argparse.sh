# vi: syntax=bash
# shellcheck shell=bash

parse_arguments() {
  local OPTIONS="$1"; shift
  local LONGOPTS="$1"; shift

  local NAME="$1"; shift
  local ARGV=("$@")

  local PARSED
  ! PARSED="$(getopt --options="$OPTIONS" --longoptions="$LONGOPTS" --name "$NAME" -- "${ARGV[@]}")"
  if [[ ${PIPESTATUS[0]} -ne 0 ]]; then
    exit 2
  fi
  printf "%s\n" "$PARSED"
}
