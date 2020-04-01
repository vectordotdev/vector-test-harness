# vi: syntax=bash
# shellcheck shell=bash

prepare_terraform_params() {
  local VERBOSE="${1:-"false"}"

  TERRAFORM_INIT_EXTRA_ARGS=()
  TERRAFORM_COMMON_EXTRA_ARGS=()

  if [[ -n "${TF_PLUGIN_DIR:-""}" ]]; then
    TERRAFORM_INIT_EXTRA_ARGS+=(
      -plugin-dir "$TF_PLUGIN_DIR"
    )

    if [[ "$VERBOSE" != "false" ]]; then
      echo "Terraform plugins dir: $TF_PLUGIN_DIR"
    fi
  fi

  if [[ "${TF_INTERACTIVE:-""}" == "true" ]]; then
    TERRAFORM_COMMON_EXTRA_ARGS+=(
      "-input=true"
    )
    if [[ "$VERBOSE" != "false" ]]; then
      echo "Terraform will accept interactive input"
    fi
  else
    TERRAFORM_COMMON_EXTRA_ARGS+=(
      "-input=false"
    )
  fi
}
