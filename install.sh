#!/bin/bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
OMARCHY_BIN="${HOME}/.local/share/omarchy/bin"
LADSPA_DIR="${HOME}/.ladspa"
PIPEWIRE_CONF_DIR="${HOME}/.config/pipewire/pipewire.conf.d"
WIREPLUMBER_CONF_DIR="${HOME}/.config/wireplumber/wireplumber.conf.d"
UDEV_RULES_DIR="/etc/udev/rules.d"
SYSTEMD_USER_DIR="${HOME}/.config/systemd/user"
STATE_DIR="${HOME}/.config/omarchy-mic"
OMARCHY_MIC_VERSION="0.1.0"
ARCH="$(uname -m)"
case "${ARCH}" in
    x86_64)  OMARCHY_MIC_ARCH="x86_64" ;;
    aarch64) OMARCHY_MIC_ARCH="aarch64" ;;
    *) echo "Error: unsupported architecture ${ARCH}" >&2; exit 1 ;;
esac
OMARCHY_MIC_URL="https://github.com/DominicBoettger/omarchy-mic/releases/download/v${OMARCHY_MIC_VERSION}/omarchy-mic-${OMARCHY_MIC_ARCH}-linux.tar.gz"
DEEPFILTER_VERSION="0.5.6"
DEEPFILTER_URL="https://github.com/Rikorose/DeepFilterNet/releases/download/v${DEEPFILTER_VERSION}/libdeep_filter_ladspa-${DEEPFILTER_VERSION}-x86_64-unknown-linux-gnu.so"

echo "╔══════════════════════════════════════╗"
echo "║       omarchy-mic installer          ║"
echo "╚══════════════════════════════════════╝"
echo

# 1. Install system dependencies
echo "▸ Installing dependencies..."
omarchy-pkg-add noise-suppression-for-voice hidapi 2>/dev/null || true

# 2. Download DeepFilterNet LADSPA plugin
if [ ! -f "${LADSPA_DIR}/libdeep_filter_ladspa.so" ]; then
    echo "▸ Downloading DeepFilterNet v${DEEPFILTER_VERSION}..."
    mkdir -p "${LADSPA_DIR}"
    curl -L "${DEEPFILTER_URL}" -o "${LADSPA_DIR}/libdeep_filter_ladspa.so"
else
    echo "▸ DeepFilterNet already installed"
fi

# 3. Download and install omarchy-mic binary
echo "▸ Downloading omarchy-mic v${OMARCHY_MIC_VERSION}..."
mkdir -p "${OMARCHY_BIN}"
curl -L "${OMARCHY_MIC_URL}" | tar xz -C "${OMARCHY_BIN}"

# 5. Install PipeWire filter config
echo "▸ Installing PipeWire DeepFilterNet config..."
mkdir -p "${PIPEWIRE_CONF_DIR}"
cat > "${PIPEWIRE_CONF_DIR}/99-noise-suppression.conf" << EOF
context.modules = [
    {   name = libpipewire-module-filter-chain
        args = {
            node.description = "Noise Cancelling Source"
            media.name        = "Noise Cancelling Source"
            filter.graph = {
                nodes = [
                    {
                        type   = ladspa
                        name   = deepfilter
                        plugin = ${LADSPA_DIR}/libdeep_filter_ladspa.so
                        label  = deep_filter_mono
                        control = {
                            "Attenuation Limit (dB)" 12
                        }
                    }
                ]
            }
            audio.rate = 48000
            audio.position = [MONO]
            capture.props = {
                node.name      = "capture.deepfilter_source"
                node.passive   = true
            }
            playback.props = {
                node.name    = "deepfilter_source"
                media.class  = Audio/Source
            }
        }
    }
]
EOF

# 6. Install WirePlumber config (prioritize Shure MV7)
echo "▸ Installing WirePlumber mic priority config..."
mkdir -p "${WIREPLUMBER_CONF_DIR}"
cat > "${WIREPLUMBER_CONF_DIR}/50-default-mic.conf" << 'EOF'
monitor.alsa.rules = [
  {
    matches = [
      {
        node.name = "alsa_input.usb-Shure_Inc_Shure_MV7-00.mono-fallback"
      }
    ]
    actions = {
      update-props = {
        priority.session = 2500
        priority.driver  = 2500
      }
    }
  }
]
EOF

# 7. Install udev rules (needs sudo)
echo "▸ Installing udev rules (requires sudo)..."
sudo cp "${SCRIPT_DIR}/99-shure-mv7.rules" "${UDEV_RULES_DIR}/"
sudo udevadm control --reload-rules
sudo udevadm trigger

# 8. Install systemd user service for MV7 hotplug
echo "▸ Installing systemd user service..."
mkdir -p "${SYSTEMD_USER_DIR}"
cat > "${SYSTEMD_USER_DIR}/omarchy-mic-setup.service" << EOF
[Unit]
Description=Auto-configure microphone (MV7 hotplug)

[Service]
Type=oneshot
ExecStartPre=/usr/bin/sleep 2
ExecStart=${OMARCHY_BIN}/omarchy-mic setup
EOF
systemctl --user daemon-reload

# 9. Install audio power-save fix for Framework 13 AMD
if grep -q "Framework" /sys/devices/virtual/dmi/id/board_vendor 2>/dev/null; then
    echo "▸ Framework laptop detected — disabling audio power-save..."
    echo "options snd_hda_intel power_save=0 power_save_controller=N" | sudo tee /etc/modprobe.d/audio-no-powersave.conf > /dev/null
fi

# 10. Install Waybar module config
echo "▸ Waybar module config:"
echo "  Add to your ~/.config/waybar/config.jsonc modules-right:"
echo '    "custom/mic"'
echo
echo "  Add this module definition:"
cat << 'EOF'
  "custom/mic": {
    "exec": "omarchy-mic waybar",
    "return-type": "json",
    "interval": 5,
    "on-click": "omarchy-mic toggle",
    "on-click-right": "omarchy-mic select",
    "tooltip": true
  },
EOF

# 11. Initialize state
mkdir -p "${STATE_DIR}"
echo "true" > "${STATE_DIR}/noise-enabled"

echo
echo "════════════════════════════════════════"
echo "  Installation complete!"
echo
echo "  Restart PipeWire to activate:"
echo "    systemctl --user restart pipewire pipewire-pulse"
echo
echo "  Then set the filter as default:"
echo "    omarchy-mic noise on"
echo
echo "  Commands:"
echo "    omarchy-mic status     — show mic & filter status"
echo "    omarchy-mic toggle     — toggle noise suppression"
echo "    omarchy-mic select     — pick input mic (walker)"
echo "    omarchy-mic mv7 voice  — apply MV7 voice preset"
echo "    omarchy-mic mv7 status — show MV7 DSP settings"
echo "════════════════════════════════════════"
