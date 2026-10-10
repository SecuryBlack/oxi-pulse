#!/bin/sh
# Never starts the agent on a fresh install: endpoint must be set first.
# On upgrade, restarts it if it was running.
systemctl daemon-reload >/dev/null 2>&1 || true
if systemctl is-active --quiet oxipulse 2>/dev/null; then
  systemctl restart oxipulse || true
elif ! systemctl is-enabled --quiet oxipulse 2>/dev/null; then
  echo "OxiPulse installed. Set endpoint in /etc/oxipulse/config.toml, then run:"
  echo "  sudo systemctl enable --now oxipulse"
fi
