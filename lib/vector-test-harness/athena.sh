# vi: syntax=bash
# shellcheck shell=bash

ensure_commad_available "aws" "sudo pip install awscli"
ensure_commad_available "jq" "brew install jq"

athena_execute_query() {
  local RESULT
  RESULT="$(aws athena start-query-execution \
    --query-execution-context "Database=vector_tests" \
    --query-string "$1" \
    --result-configuration "OutputLocation=s3://vector-test-athena-results")"
  RESULT="$(echo "$RESULT" | tr '\n' ' ')"

  local EXECUTION_ID
  EXECUTION_ID="$(echo "$RESULT" | jq -r ".QueryExecutionId")"

  athena_wait_for_query "$EXECUTION_ID" >/dev/null

  echo "$EXECUTION_ID"
}

athena_wait_for_query() {
  local EXECUTION_ID="$1"
  local QUERY_STATUS=""

  until [ "$QUERY_STATUS" == "SUCCEEDED" ] ||
    [ "$QUERY_STATUS" == "FAILED" ] ||
    [ "$QUERY_STATUS" == "CANCELLED" ]; do

    printf "." >&2
    sleep 0.25

    local RESULT
    RESULT="$(aws athena get-query-execution --query-execution-id "$EXECUTION_ID")"
    QUERY_STATUS="$(echo "$RESULT" | jq -r ".QueryExecution.Status.State")"
  done
  printf "\n" >&2
}

athena_get_results() {
  local EXECUTION_ID="$1"
  local RESULT
  RESULT="$(aws athena get-query-results --query-execution-id "$EXECUTION_ID")"
  echo "$RESULT" | jq -r
}
