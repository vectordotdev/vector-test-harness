# vi: syntax=bash
# shellcheck shell=bash

error() {
  local MESSAGE="$1"
  echo -e "\033[0;31m$MESSAGE\033[0m"
}

fail_with_code() {
  local EXIT_CODE="$1"
  local MESSAGE="$2"
  error "$MESSAGE"
  exit "$EXIT_CODE"
}

fail() {
  local MESSAGE="$1"
  fail_with_code 1 "$MESSAGE"
}

fail_arg_invalid() {
  local MESSAGE="$1"
  error "$MESSAGE"
  usage
  exit 3
}

ensure_commad_available() {
  local COMMAND="$1"
  local HINT="${2:-""}"
  if ! [ -x "$(command -v "$COMMAND")" ]; then
    if [[ -n "$HINT" ]]; then
      HINT=" ($HINT)"
    fi

    echo "Error: \"$COMMAND\" is not installed.$HINT" >&2
    exit 1
  fi
}

# This is required for flag parsing, specifically the posix style: --flag
ensure_echanced_getopt_available() {
  ! getopt --test >/dev/null
  if [[ ${PIPESTATUS[0]} -ne 4 ]]; then
    error 'Error: enhanced getopt is not installed. (brew install gnu-getopt)'
    exit 1
  fi
}

join() {
  d="$1"
  shift
  arr=("$@")
  if [ "${#arr[@]}" -gt 1 ]; then
    last=${arr[${#arr[@]}-1]}
    unset arr[${#arr[@]}-1]
    printf -v str "%s$d" "${arr[@]}"
    printf "%s%s" "$str" "$last"
  else
    echo "${arr[0]}"
  fi
}


spin() {
  spinner="/|\\-/|\\-"
  while :
  do
    for i in `seq 0 7`
    do
      echo -n "${spinner:$i:1}" >&2
      echo -en "\010" >&2
      sleep 1
    done
  done
}
