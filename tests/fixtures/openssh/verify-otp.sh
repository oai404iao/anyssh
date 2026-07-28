#!/bin/sh
set -eu

response="$(cat)"
expected="$(cat /run/anyssh-otp-token)"
test -n "$expected" && test "$response" = "$expected"
