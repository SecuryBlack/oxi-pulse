#!/bin/sh
# Stop only on removal, not on upgrade (deb: "remove"; rpm: $1 = 0).
if [ "$1" = "remove" ] || [ "$1" = "0" ]; then
  systemctl disable --now oxipulse >/dev/null 2>&1 || true
fi
