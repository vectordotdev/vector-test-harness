# vi: syntax=bash
# shellcheck shell=bash

TEST_SUBJECT_NAMES=(
  filebeat
  fluentbit
  fluentd
  logstash
  splunk_heavy_forwarder
  splunk_universal_forwarder
  telegraf
  vector
)

TEST_SUBJECT_VERSION_filebeat="7.1.1"
TEST_SUBJECT_VERSION_fluentbit="1.1.0"
TEST_SUBJECT_VERSION_fluentd="3.3.0-1"
TEST_SUBJECT_VERSION_logstash="7.0.1"
TEST_SUBJECT_VERSION_splunk_heavy_forwarder="7.2.6-c0bf0f679ce9"
TEST_SUBJECT_VERSION_splunk_universal_forwarder="7.2.5.1-962d9a8e1586"
TEST_SUBJECT_VERSION_telegraf="1.11.0-1"
TEST_SUBJECT_VERSION_vector="0.5.0"

print_test_subject_versions() {
  local PREFIX="${1:-""}"
  local VERSION_VAR

  for SUBJECT in "${TEST_SUBJECT_NAMES[@]}"; do
    VERSION_VAR="TEST_SUBJECT_VERSION_${SUBJECT}"
    echo "${PREFIX}${SUBJECT}: ${!VERSION_VAR}"
  done
}

prepare_ansible_version_vars() {
  local VERSION_VAR

  ANSIBLE_VERSION_VARS=""

  for SUBJECT in "${TEST_SUBJECT_NAMES[@]}"; do
    VERSION_VAR="TEST_SUBJECT_VERSION_${SUBJECT}"
    ANSIBLE_VERSION_VARS="$ANSIBLE_VERSION_VARS ${SUBJECT}_version=${!VERSION_VAR}"
  done
}
