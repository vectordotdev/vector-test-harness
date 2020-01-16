#!/bin/bash

set -e

# saner programming env: these switches turn some bugs into errors
set -o errexit -o pipefail -o noclobber -o nounset

if ! [ -x "$(command -v aws)" ]; then
  echo 'Error: awscli is not installed. (sudo pip install awscli)' >&2
  exit 1
fi

if ! [ -x "$(command -v jq)" ]; then
  echo 'Error: jq is not installed. (brew install jq)' >&2
  exit 1
fi

my_dir="$(dirname "$0")"

execute_query () {
  local result=$(aws athena start-query-execution \
    --query-execution-context "Database=vector_tests" \
    --query-string "$1" \
    --result-configuration "OutputLocation=s3://vector-test-athena-results")
  result=$(echo $result | tr '\n' ' ')

  local execution_id=$(echo $result | jq -r ".QueryExecutionId")

  wait_for_query $execution_id

  echo $execution_id
}

wait_for_query() {
  local query_status=''

  until [ "$query_status" == "SUCCEEDED" ] || [ "$query_status" == "FAILED" ] || [ "$query_status" == "CANCELLED" ]
  do
    local temp=${spinstr#?}
    echo $temp
    sleep 0.25
    local result=$(aws athena get-query-execution --query-execution-id $1)
    result=$(echo $result | tr '\n' ' ')
    query_status=$(echo $result | jq -r ".QueryExecution.Status.State")
  done
}

get_results() {
  local result=$(aws athena get-query-results --query-execution-id $1)
  result=$(echo $result | tr '\n' ' ')
  echo $result | jq -r
}
