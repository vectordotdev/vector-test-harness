# vi: syntax=bash
# shellcheck shell=bash

VECTOR_TEST_HARNESS_LIB_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}" )" >/dev/null 2>&1 && pwd)"
VECTOR_TEST_HARNESS_ROOT="$VECTOR_TEST_HARNESS_LIB_ROOT/../.."
# shellcheck disable=SC2034
VECTOR_TEST_HARNESS_ANSIBLE_DIR="$VECTOR_TEST_HARNESS_ROOT/ansible"
# shellcheck disable=SC2034
VECTOR_TEST_HARNESS_UTILS_DIR="$VECTOR_TEST_HARNESS_LIB_ROOT/utils"

# shellcheck source=SCRIPTDIR/load_dotenv.sh
source "${VECTOR_TEST_HARNESS_LIB_ROOT}/load_dotenv.sh"

load_dotenv_if_allowed

# shellcheck source=SCRIPTDIR/misc.sh
source "${VECTOR_TEST_HARNESS_LIB_ROOT}/misc.sh"

# shellcheck source=SCRIPTDIR/usage.sh
source "${VECTOR_TEST_HARNESS_LIB_ROOT}/usage.sh"

# shellcheck source=SCRIPTDIR/athena.sh
source "${VECTOR_TEST_HARNESS_LIB_ROOT}/athena.sh"

# shellcheck source=SCRIPTDIR/visual.sh
source "${VECTOR_TEST_HARNESS_LIB_ROOT}/visual.sh"

# shellcheck source=SCRIPTDIR/visual.sh
source "${VECTOR_TEST_HARNESS_LIB_ROOT}/ansible.sh"

# shellcheck source=SCRIPTDIR/argparse.sh
source "${VECTOR_TEST_HARNESS_LIB_ROOT}/argparse.sh"

# shellcheck source=SCRIPTDIR/terraform.sh
source "${VECTOR_TEST_HARNESS_LIB_ROOT}/terraform.sh"

# shellcheck source=SCRIPTDIR/test_subjects.sh
source "${VECTOR_TEST_HARNESS_LIB_ROOT}/test_subjects.sh"

# shellcheck source=SCRIPTDIR/filter.sh
source "${VECTOR_TEST_HARNESS_LIB_ROOT}/filter.sh"
