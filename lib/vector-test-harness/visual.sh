# vi: syntax=bash
# shellcheck shell=bash

print_vector_logo() {
  cat <<END

                                   __   __  __
                                   \\ \\ / / / /
                                    \\ V / / /
                                     \\_/  \\/

                                   V E C T O R

END
}

print_divider() {
  cat <<END
--------------------------------------------------------------------------------
END
}

print_spacer() {
  echo
  echo
}

print_section_header() {
  local SECTION_NAME="$1"
  print_divider
  echo "$SECTION_NAME"
  print_divider
}
