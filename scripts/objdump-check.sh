#!/bin/sh
# Disassembles a raw x86-64 code file with GNU objdump, Intel syntax, on stdout.
#
# usage: GLOSS_RUNNER="orb -m gloss-amd64" scripts/objdump-check.sh <file>
#
# GLOSS_RUNNER is the command prefix that reaches a Linux machine with binutils (empty or
# unset runs objdump here). tests/objdump.rs sets it from the same runner the run tests
# use. The file must be visible on that machine (put it under target/, not in /var/folders).
#
# GLOSS_RUNNER is split on spaces (ruling M2-R11), so a runner whose path contains a space
# is not supported.
#
# One-time setup of the OrbStack machine (done on the owner's machine, 2026-11-23):
#   orb -m gloss-amd64 sudo apk add binutils
set -eu
[ $# -eq 1 ] || { echo "usage: $0 <file>" >&2; exit 2; }
# shellcheck disable=SC2086 # the runner is a command prefix of several words
exec ${GLOSS_RUNNER:-} objdump -D -b binary -m i386:x86-64 -M intel "$1"
