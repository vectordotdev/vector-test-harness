# vi: syntax=bash
# shellcheck shell=bash

# WARNING!
#
# Do not rely on version information from this file when runing tests.
# These versions are intended to be used only during the AMI build.
#
# This file is not loaded by default during lib initialization.

PREINSTALLED_TEST_SUBJECT_VERSION_filebeat="8.14.3"
PREINSTALLED_TEST_SUBJECT_VERSION_fluentbit="3.2.7"
PREINSTALLED_TEST_SUBJECT_VERSION_fluentd="4.5.2-1"
PREINSTALLED_TEST_SUBJECT_VERSION_logstash="8.17.3"
PREINSTALLED_TEST_SUBJECT_VERSION_splunk_heavy_forwarder="9.2.2-d76edf6f0a15"
PREINSTALLED_TEST_SUBJECT_VERSION_splunk_universal_forwarder="9.2.2-d76edf6f0a15"
PREINSTALLED_TEST_SUBJECT_VERSION_telegraf="1.33.2-1"

print_preinstalled_test_subject_versions() {
  local PREFIX="${1:-""}"
  local VERSION_VAR

  for SUBJECT in "${TEST_SUBJECT_NAMES[@]}"; do
    VERSION_VAR="PREINSTALLED_TEST_SUBJECT_VERSION_${SUBJECT}"
    if [[ -n "${!VERSION_VAR:-""}" ]]; then
      echo "${PREFIX}${SUBJECT}: ${!VERSION_VAR}"
    fi
  done
}

prepare_preinstalled_test_subject_versions_ansible_vars() {
  local VERSION_VAR

  PREINSTALLED_TEST_SUBJECT_VERSIONS_ANSIBLE_VARS=""

  for SUBJECT in "${TEST_SUBJECT_NAMES[@]}"; do
    VERSION_VAR="PREINSTALLED_TEST_SUBJECT_VERSION_${SUBJECT}"
    if [[ -n "${!VERSION_VAR:-""}" ]]; then
      PREINSTALLED_TEST_SUBJECT_VERSIONS_ANSIBLE_VARS="$PREINSTALLED_TEST_SUBJECT_VERSIONS_ANSIBLE_VARS preinstalled_${SUBJECT}_version=${!VERSION_VAR}"
    fi
  done
}
