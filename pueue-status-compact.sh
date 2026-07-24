#!/usr/bin/env bash
set -euo pipefail

debug=0
if [[ "${1-}" == "--debug" ]]; then
	debug=1
	shift
elif [[ $# -gt 0 ]]; then
	echo "Usage: $0 [--debug]" >&2
	exit 2
fi

# Remove lines that are only decorative table borders.
# Prefer a Unicode-aware filter, then fall back to ASCII-only borders.
unicode_filter='{
	line = $0
	gsub(/[[:space:]\+\-|=:_#\*\.┌┐└┘├┤┬┴┼─│╭╮╰╯╞╡╪═║╔╗╚╝╠╣╦╩╬]/, "", line)
	if (line == "") {
		next
	}
	print
}'

ascii_filter='{
	line = $0
	gsub(/[[:space:]\+\-|=:_#\*\.]/, "", line)
	if (line == "") {
		next
	}
	print
}'

if printf '%s\n' '┌─┐' | awk "$unicode_filter" >/dev/null 2>&1; then
	if [[ $debug -eq 1 ]]; then
		echo "[debug] filter=unicode" >&2
	fi
	pueue status | awk "$unicode_filter"
else
	if [[ $debug -eq 1 ]]; then
		echo "[debug] filter=ascii" >&2
	fi
	pueue status | awk "$ascii_filter"
fi
