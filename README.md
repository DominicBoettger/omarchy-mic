# omarchy-mic

Microphone management and noise suppression for [Omarchy](https://github.com/basecamp/omarchy) Linux.

Uses [DeepFilterNet](https://github.com/Rikorose/DeepFilterNet) — a deep neural network for real-time noise suppression via PipeWire. Optionally configures [Shure MV7](https://www.shure.com/en-US/products/microphones/mv/mv7) hardware DSP over USB HID.

## Features

- **DeepFilterNet noise suppression** — better than RNNoise for keyboard clicks, background noise, and voice naturalness
- **Shure MV7 hardware DSP** — configure compressor, high-pass, presence filter, limiter, and gain directly on the mic via USB HID
- **MV7 auto-setup** — automatically selects MV7 and applies voice preset on plug-in (udev)
- **Waybar integration** — mic icon with noise toggle (click) and mic picker (right-click)
- **Instant mic switching** — select input mic via walker menu, no PipeWire restart needed
- **Toggle without restart** — enable/disable noise suppression by switching default source, no audio interruption

## Requirements

- Arch Linux with PipeWire + WirePlumber
- Rust toolchain (`rustup`)
- `walker` (for mic picker UI)
- `hidapi` (for MV7 USB HID, optional)

## Quick Install

```bash
git clone https://github.com/YOUR_USER/omarchy-mic.git ~/Development/omarchy-mic
cd ~/Development/omarchy-mic
./install.sh
```

The install script will:
1. Install dependencies (`noise-suppression-for-voice`, `hidapi`)
2. Download DeepFilterNet LADSPA plugin (~50MB, includes neural network model)
3. Build the `omarchy-mic` Rust binary
4. Install PipeWire filter-chain config
5. Install WirePlumber mic priority rules
6. Install udev rules for Shure MV7 hotplug (requires sudo)
7. Install systemd user service for MV7 auto-setup
8. Apply Framework Laptop 13 AMD audio fixes (if detected)

After install, restart PipeWire once:
```bash
systemctl --user restart pipewire pipewire-pulse
omarchy-mic noise on
```

## Usage

```
omarchy-mic status              Show mic, noise suppression, and MV7 status
omarchy-mic status --json       JSON output

omarchy-mic noise on            Enable noise suppression (set DeepFilterNet as default source)
omarchy-mic noise off           Disable noise suppression (set real mic as default source)
omarchy-mic toggle              Toggle noise suppression on/off

omarchy-mic select              Pick input mic via walker menu

omarchy-mic mv7 status          Show MV7 DSP settings (firmware, gain, compressor, etc.)
omarchy-mic mv7 voice           Apply voice preset (high-pass, presence, light compressor, limiter)
omarchy-mic mv7 compressor off  Set compressor (off/light/medium/heavy)
omarchy-mic mv7 high-pass on    Toggle high-pass filter
omarchy-mic mv7 presence on     Toggle presence filter
omarchy-mic mv7 limiter on      Toggle limiter
omarchy-mic mv7 gain 24.0       Set input gain in dB (0-36)
omarchy-mic mv7 identify        Blink MV7 LEDs

omarchy-mic waybar              JSON output for Waybar custom module
omarchy-mic setup               Auto-detect MV7, apply voice preset, select as input
```

## Waybar Integration

Add to `~/.config/waybar/config.jsonc`:

```jsonc
// In modules-right array:
"custom/mic",

// Module definition:
"custom/mic": {
    "exec": "omarchy-mic waybar",
    "return-type": "json",
    "interval": 5,
    "on-click": "omarchy-mic toggle",
    "on-click-right": "omarchy-mic select",
    "tooltip": true
},
```

Add to `~/.config/waybar/style.css`:

```css
#custom-mic {
    min-width: 12px;
    margin: 0 0 0 7.5px;
}

#custom-mic.inactive {
    color: #666666;
}
```

## How It Works

### Noise Suppression

DeepFilterNet runs as a PipeWire filter-chain module. It creates a virtual microphone (`deepfilter_source`) that captures from your physical mic and outputs noise-suppressed audio. The attenuation limit is set to 12 dB for natural voice with moderate noise reduction.

```
Physical Mic → DeepFilterNet (LADSPA) → deepfilter_source → Apps
```

Toggling noise suppression switches the default PipeWire source between `deepfilter_source` (filtered) and the physical mic (raw) — no PipeWire restart needed.

### Shure MV7 Hardware DSP

The MV7 has an onboard DSP accessible via USB HID (vendor `0x14ED`, product `0x1012`, interface 3). The tool sends text commands over 64-byte HID reports to configure:

- **High-pass filter** — cuts low-frequency rumble
- **Presence filter** — boosts voice clarity
- **Compressor** — evens out volume (off/light/medium/heavy)
- **Limiter** — prevents clipping
- **Input gain** — 0-36 dB

The `voice` preset enables high-pass + presence + light compressor + limiter.

### MV7 Auto-Setup

A udev rule triggers `omarchy-mic setup` when the MV7 is plugged in. This:
1. Detects the MV7 via PipeWire
2. Selects it as the DeepFilterNet input (via `pw-link`)
3. Applies the voice DSP preset via USB HID

When unplugged, PipeWire automatically falls back to the next available mic.

## File Locations

| File | Purpose |
|------|---------|
| `~/.local/share/omarchy/bin/omarchy-mic` | Binary |
| `~/.ladspa/libdeep_filter_ladspa.so` | DeepFilterNet LADSPA plugin |
| `~/.config/pipewire/pipewire.conf.d/99-noise-suppression.conf` | PipeWire filter config |
| `~/.config/wireplumber/wireplumber.conf.d/50-default-mic.conf` | Mic priority rules |
| `~/.config/omarchy-mic/noise-enabled` | Noise suppression state |
| `~/.config/omarchy-mic/selected-mic` | Selected physical mic |
| `~/.config/systemd/user/omarchy-mic-setup.service` | MV7 hotplug service |
| `/etc/udev/rules.d/99-shure-mv7.rules` | MV7 udev rules |

## Contributing to Omarchy

To contribute this as an Omarchy addon, it would follow the Omarchy distribution pattern:

1. **Package list**: Add `hidapi` to `install/omarchy-base.packages`
2. **Binary**: Ship pre-built binary or add build step to install
3. **Configs**: Add default configs to `config/pipewire/` and `config/wireplumber/`
4. **Refresh script**: Create `bin/omarchy-refresh-mic` to deploy configs
5. **Migration**: Add a migration script to install DeepFilterNet LADSPA plugin on update

## Uninstall

```bash
cd ~/Development/omarchy-mic
./uninstall.sh
systemctl --user restart pipewire pipewire-pulse
```

## Credits

- [DeepFilterNet](https://github.com/Rikorose/DeepFilterNet) by Hendrik Schroeter — deep neural network noise suppression
- [mv7config](https://github.com/matteodelabre/mv7config) by Matteo Delabre — Shure MV7 USB HID protocol reference
- [noise-suppression-for-voice](https://github.com/werman/noise-suppression-for-voice) — RNNoise LADSPA plugin (fallback)

## License

MIT License — see [LICENSE](LICENSE)
