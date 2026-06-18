#!/bin/sh
set -e

if [ -n "${AUTHSTACK_BOOTSTRAP_EMAIL:-}" ]; then
  if /app/authstack bootstrap-admin; then
    echo "bootstrap: created first instance admin"
  else
    code=$?
    if [ "$code" -eq 1 ]; then
      echo "bootstrap: instance admin already exists"
    else
      exit "$code"
    fi
  fi
fi

exec /app/authstack "$@"
