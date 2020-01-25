# vi: syntax=bash
# shellcheck shell=bash

load_env_file() {
  FILE="$1"
  if [[ -f "$FILE" ]]; then
    set -a
    # shellcheck disable=SC1090
    source "$FILE"
    set +a
  fi
}

load_dotenv() {
  load_env_file ".env"
  load_env_file ".env.local"
}

load_dotenv_if_allowed() {
  if [[ "${DO_NOT_LOAD_DOTENV:-"false"}" == "false" ]]; then
    load_dotenv
  fi
}
