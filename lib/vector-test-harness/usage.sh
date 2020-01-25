# vi: syntax=bash
# shellcheck shell=bash

usage() {
  # shellcheck disable=SC2154
  echo "$USAGE_TEXT" 1>&2;
}

register_usage() {
  USAGE_TEXT="$(cat)"
}
