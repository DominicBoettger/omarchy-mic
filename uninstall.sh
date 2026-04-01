#!/bin/bash
set -euo pipefail

OMARCHY_BIN="${HOME}/.local/share/omarchy/bin"
LADSPA_DIR="${HOME}/.ladspa"
PIPEWIRE_CONF_DIR="${HOME}/.config/pipewire/pipewire.conf.d"
WIREPLUMBER_CONF_DIR="${HOME}/.config/wireplumber/wireplumber.conf.d"
SYSTEMD_USER_DIR="${HOME}/.config/systemd/user"
STATE_DIR="${HOME}/.config/omarchy-mic"

echo "Uninstalling omarchy-mic..."

rm -f "${OMARCHY_BIN}/omarchy-mic"
rm -f "${PIPEWIRE_CONF_DIR}/99-noise-suppression.conf"
rm -f "${WIREPLUMBER_CONF_DIR}/50-default-mic.conf"
rm -f "${SYSTEMD_USER_DIR}/omarchy-mic-setup.service"
rm -rf "${STATE_DIR}"
rm -f "${LADSPA_DIR}/libdeep_filter_ladspa.so"

systemctl --user daemon-reload

echo "Removing udev rules (requires sudo)..."
sudo rm -f /etc/udev/rules.d/99-shure-mv7.rules
sudo udevadm control --reload-rules

echo "Done. Restart PipeWire: systemctl --user restart pipewire pipewire-pulse"
