# shellcheck shell=bash
# Sourced into every recording stub, which sets CALLS.
#
# One call, one write(2): an O_APPEND write to a local file lands whole, so a
# background poller's stubs cannot split a line. bash's `echo` and `printf` do
# not give that: stdio cuts a long line into several writes (1024 bytes each on
# macOS, a run of blocks and a tail on Linux), and a poller's line can land
# between them. `dd` holds everything it reads until `obs` fills or its input
# ends, and `obs` is far above any line a stub logs.
record_call() { # $1 = the line
  printf '%s\n' "$1" | dd obs=16777216 2>/dev/null >>"$CALLS"
}
