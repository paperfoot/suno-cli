#!/usr/bin/env bash

# Read a Clerk __client cookie without exposing it in shell history or argv.
set -u

usage() {
    cat <<'EOF'
Usage: suno-auth-cookie [--check]

Interactively read the __client cookie from auth.suno.com and sign in to Suno.
Set SUNO_BIN to use a specific suno executable (default: suno on PATH).
EOF
}

case "${1:-}" in
    -h|--help)
        usage
        exit 0
        ;;
    --check|"")
        ;;
    *)
        usage >&2
        exit 2
        ;;
esac
if [[ $# -gt 1 ]]; then
    usage >&2
    exit 2
fi

suno_bin=${SUNO_BIN:-suno}
if ! command -v "$suno_bin" >/dev/null 2>&1; then
    printf 'Suno executable not found: %s\n' "$suno_bin" >&2
    exit 2
fi

if [[ "${1:-}" == --check ]]; then
    "$suno_bin" --version
    exit $?
fi

if [[ ! -t 0 ]]; then
    printf 'Run this command in a terminal to enter the cookie privately.\n' >&2
    exit 2
fi

printf 'Paste the Value of the __client cookie from auth.suno.com. Press Ctrl+C to cancel.\n'
cookie=''
trap 'unset cookie' EXIT
if ! IFS= read -r -s -p 'Cookie __client (hidden): ' cookie; then
    printf '\nCookie entry cancelled.\n' >&2
    exit 2
fi
printf '\n'
if [[ -z "$cookie" ]]; then
    printf 'The cookie is empty.\n' >&2
    exit 2
fi

printf '%s' "$cookie" | "$suno_bin" --no-browser auth --cookie-stdin
result=$?
unset cookie
if [[ $result -eq 0 ]]; then
    printf 'Signed in. Verify with: %s --no-browser credits\n' "$suno_bin"
fi
exit "$result"
