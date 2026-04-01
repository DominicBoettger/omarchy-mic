mod mv7;
mod noise;
mod pipewire;

use clap::{Parser, Subcommand};
use serde_json::json;

#[derive(Parser)]
#[command(name = "omarchy-mic", about = "Microphone management for Omarchy")]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Show current microphone and noise suppression status
    Status {
        /// Output as JSON (for Waybar)
        #[arg(long)]
        json: bool,
    },
    /// Manage noise suppression
    Noise {
        #[command(subcommand)]
        action: NoiseAction,
    },
    /// Configure Shure MV7 DSP settings
    Mv7 {
        #[command(subcommand)]
        action: Mv7Action,
    },
    /// Waybar module output (JSON)
    Waybar,
    /// Toggle noise suppression on/off
    Toggle,
    /// Select input mic via walker menu
    Select,
    /// Auto-setup: detect MV7, apply voice preset, select as input
    Setup,
}

#[derive(Subcommand)]
enum NoiseAction {
    /// Enable rnnoise noise suppression
    On,
    /// Disable noise suppression
    Off,
}

#[derive(Subcommand)]
enum Mv7Action {
    /// Show MV7 DSP status
    Status,
    /// Configure MV7 for voice (enables high-pass, presence, light compressor)
    Voice,
    /// Set compressor (off/light/medium/heavy)
    Compressor {
        level: String,
    },
    /// Toggle high-pass filter (on/off)
    HighPass {
        state: String,
    },
    /// Toggle presence filter (on/off)
    Presence {
        state: String,
    },
    /// Toggle limiter (on/off)
    Limiter {
        state: String,
    },
    /// Set input gain in dB (0-36)
    Gain {
        db: f32,
    },
    /// Blink the MV7 LEDs to identify it
    Identify,
}

fn parse_on_off(s: &str) -> bool {
    match s.to_lowercase().as_str() {
        "on" | "true" | "1" | "yes" => true,
        "off" | "false" | "0" | "no" => false,
        _ => {
            eprintln!("Expected on/off, got '{s}'");
            std::process::exit(1);
        }
    }
}

fn main() {
    let cli = Cli::parse();

    match cli.command {
        Commands::Status { json: as_json } => cmd_status(as_json),
        Commands::Noise { action } => cmd_noise(action),
        Commands::Mv7 { action } => cmd_mv7(action),
        Commands::Waybar => cmd_waybar(),
        Commands::Toggle => cmd_toggle(),
        Commands::Select => cmd_select(),
        Commands::Setup => cmd_setup(),
    }
}

fn cmd_status(as_json: bool) {
    let sources = pipewire::list_sources();
    let default_source = sources.iter().find(|s| s.is_default);
    let noise_enabled = noise::is_enabled();
    let noise_active = noise::is_active();
    let mv7_available = mv7::Mv7::is_available();

    if as_json {
        let obj = json!({
            "default_source": default_source.map(|s| &s.name),
            "noise_suppression": {
                "enabled": noise_enabled,
                "active": noise_active,
                "plugin_installed": noise::is_plugin_installed(),
            },
            "mv7": {
                "available": mv7_available,
            },
            "sources": sources.iter().map(|s| json!({
                "id": s.id,
                "name": s.name,
                "default": s.is_default,
            })).collect::<Vec<_>>(),
        });
        println!("{}", serde_json::to_string_pretty(&obj).unwrap());
    } else {
        println!("Microphone Status");
        println!("─────────────────");

        if let Some(src) = default_source {
            println!("Default source: {} (ID {})", src.name, src.id);
        } else {
            println!("Default source: none");
        }

        println!();
        println!("Noise suppression: {}", if noise_active {
            "active"
        } else if noise_enabled {
            "enabled (not active)"
        } else {
            "disabled"
        });

        if noise_active || noise_enabled {
            let input_name = pipewire::get_rnnoise_input()
                .or_else(|| noise::get_selected_mic().and_then(|n| pipewire::resolve_display_name(&n)))
                .unwrap_or_else(|| "auto (default)".to_string());
            println!("Input mic:         {input_name}");
        }

        if !noise::is_plugin_installed() {
            println!("  (plugin not installed — run: omarchy-pkg-add noise-suppression-for-voice)");
        }

        println!("Shure MV7: {}", if mv7_available { "connected" } else { "not found" });

        println!();
        println!("Sources:");
        for src in &sources {
            let marker = if src.is_default { " *" } else { "  " };
            println!("{} {} (ID {})", marker, src.name, src.id);
        }
    }
}

fn cmd_noise(action: NoiseAction) {
    match action {
        NoiseAction::On => {
            match noise::enable() {
                Ok(()) => println!("Noise suppression enabled"),
                Err(e) => {
                    eprintln!("Error: {e}");
                    std::process::exit(1);
                }
            }
        }
        NoiseAction::Off => {
            match noise::disable() {
                Ok(()) => println!("Noise suppression disabled"),
                Err(e) => {
                    eprintln!("Error: {e}");
                    std::process::exit(1);
                }
            }
        }
    }
}

fn cmd_toggle() {
    if noise::is_enabled() {
        match noise::disable() {
            Ok(()) => println!("Noise suppression disabled"),
            Err(e) => {
                eprintln!("Error: {e}");
                std::process::exit(1);
            }
        }
    } else {
        match noise::enable() {
            Ok(()) => println!("Noise suppression enabled"),
            Err(e) => {
                eprintln!("Error: {e}");
                std::process::exit(1);
            }
        }
    }
}

fn cmd_mv7(action: Mv7Action) {
    match action {
        Mv7Action::Status => {
            let mic = match mv7::Mv7::open() {
                Ok(m) => m,
                Err(e) => {
                    eprintln!("Error: {e}");
                    std::process::exit(1);
                }
            };
            let state = mic.get_state();
            println!("Shure MV7 Status");
            println!("────────────────");
            if let Some(fw) = &state.firmware_version {
                println!("Firmware:     {fw}");
            }
            if let Some(sn) = &state.serial_number {
                println!("Serial:       {sn}");
            }
            if let Some(mode) = &state.mode {
                println!("DSP Mode:     {mode}");
            }
            if let Some(gain) = state.input_gain {
                println!("Input Gain:   {gain:.1} dB");
            }
            if let Some(muted) = state.input_mute {
                println!("Input Mute:   {}", if muted { "on" } else { "off" });
            }
            if let Some(comp) = &state.compressor {
                println!("Compressor:   {comp}");
            }
            if let Some(lim) = state.limiter {
                println!("Limiter:      {}", if lim { "on" } else { "off" });
            }
            if let Some(hp) = state.high_pass_filter {
                println!("High-pass:    {}", if hp { "on" } else { "off" });
            }
            if let Some(pres) = state.presence_filter {
                println!("Presence:     {}", if pres { "on" } else { "off" });
            }
            if let Some(vol) = state.monitor_volume {
                println!("Monitor Vol:  {vol:.1} dB");
            }
            if let Some(muted) = state.monitor_mute {
                println!("Monitor Mute: {}", if muted { "on" } else { "off" });
            }
        }
        Mv7Action::Voice => {
            let mic = match mv7::Mv7::open() {
                Ok(m) => m,
                Err(e) => {
                    eprintln!("Error: {e}");
                    std::process::exit(1);
                }
            };
            mic.set_dsp_mode(mv7::DspMode::Manual).ok();
            std::thread::sleep(std::time::Duration::from_millis(500));
            mic.set_high_pass_filter(true).ok();
            mic.set_presence_filter(true).ok();
            mic.set_compressor(mv7::Compressor::Light).ok();
            mic.set_limiter(true).ok();
            println!("MV7 configured for voice: high-pass on, presence on, compressor light, limiter on");
        }
        Mv7Action::Compressor { level } => {
            let comp = match mv7::Compressor::from_str(&level) {
                Some(c) => c,
                None => {
                    eprintln!("Invalid compressor level: {level} (use off/light/medium/heavy)");
                    std::process::exit(1);
                }
            };
            let mic = open_mv7();
            mic.set_compressor(comp).unwrap_or_else(|e| {
                eprintln!("Error: {e}");
                std::process::exit(1);
            });
            println!("Compressor set to {comp}");
        }
        Mv7Action::HighPass { state } => {
            let on = parse_on_off(&state);
            let mic = open_mv7();
            mic.set_high_pass_filter(on).unwrap_or_else(|e| {
                eprintln!("Error: {e}");
                std::process::exit(1);
            });
            println!("High-pass filter {}", if on { "enabled" } else { "disabled" });
        }
        Mv7Action::Presence { state } => {
            let on = parse_on_off(&state);
            let mic = open_mv7();
            mic.set_presence_filter(on).unwrap_or_else(|e| {
                eprintln!("Error: {e}");
                std::process::exit(1);
            });
            println!("Presence filter {}", if on { "enabled" } else { "disabled" });
        }
        Mv7Action::Limiter { state } => {
            let on = parse_on_off(&state);
            let mic = open_mv7();
            mic.set_limiter(on).unwrap_or_else(|e| {
                eprintln!("Error: {e}");
                std::process::exit(1);
            });
            println!("Limiter {}", if on { "enabled" } else { "disabled" });
        }
        Mv7Action::Gain { db } => {
            let mic = open_mv7();
            mic.set_input_gain(db).unwrap_or_else(|e| {
                eprintln!("Error: {e}");
                std::process::exit(1);
            });
            println!("Input gain set to {db:.1} dB");
        }
        Mv7Action::Identify => {
            let mic = open_mv7();
            mic.identify().unwrap_or_else(|e| {
                eprintln!("Error: {e}");
                std::process::exit(1);
            });
            println!("MV7 LEDs blinking");
        }
    }
}

fn open_mv7() -> mv7::Mv7 {
    mv7::Mv7::open().unwrap_or_else(|e| {
        eprintln!("Error: {e}");
        std::process::exit(1);
    })
}

fn cmd_waybar() {
    let noise_active = noise::is_active();
    let noise_enabled = noise::is_enabled();
    let mv7_available = mv7::Mv7::is_available();

    // Check mute state
    let muted = std::process::Command::new("wpctl")
        .args(["get-volume", "@DEFAULT_SOURCE@"])
        .output()
        .ok()
        .map(|o| String::from_utf8_lossy(&o.stdout).contains("MUTED"))
        .unwrap_or(false);

    // Show the actual physical mic being used
    let input_mic = if noise_active || noise_enabled {
        pipewire::get_rnnoise_input()
            .or_else(|| {
                noise::get_selected_mic()
                    .and_then(|n| pipewire::resolve_display_name(&n))
            })
            .unwrap_or_else(|| "default".to_string())
    } else {
        pipewire::get_default_source_name()
            .unwrap_or_else(|| "No mic".to_string())
    };

    let icon = if muted {
        "\u{f131}" // mic-slash (muted)
    } else if noise_enabled {
        "\u{f130}" // mic (noise suppression on)
    } else {
        "\u{f130}" // mic (raw)
    };

    let noise_status = if muted {
        "muted"
    } else if noise_enabled {
        "on"
    } else {
        "off"
    };

    let tooltip = format!(
        "Mic: {input_mic}\nNoise suppression: {noise_status}\nMV7: {}",
        if mv7_available { "connected" } else { "disconnected" },
    );

    let class = if muted { "muted" } else if noise_enabled { "active" } else { "inactive" };

    let obj = json!({
        "text": icon,
        "tooltip": tooltip,
        "class": class,
    });

    println!("{}", serde_json::to_string(&obj).unwrap());
}

fn cmd_setup() {
    let mv7_node = "alsa_input.usb-Shure_Inc_Shure_MV7-00.mono-fallback";

    // Check if MV7 is connected by looking at available sources
    let sources = pipewire::list_sources();
    let mv7_source = sources.iter().find(|s| s.node_name.as_deref() == Some(mv7_node));

    if mv7_source.is_some() {
        println!("MV7 detected — configuring...");

        // Select MV7 as input (routes to rnnoise if active)
        if let Err(e) = noise::set_input(mv7_node) {
            eprintln!("Warning: failed to set input: {e}");
        }

        // Apply voice DSP preset
        match mv7::Mv7::open() {
            Ok(mic) => {
                mic.set_dsp_mode(mv7::DspMode::Manual).ok();
                std::thread::sleep(std::time::Duration::from_millis(500));
                mic.set_high_pass_filter(true).ok();
                mic.set_presence_filter(true).ok();
                mic.set_compressor(mv7::Compressor::Light).ok();
                mic.set_limiter(true).ok();
                println!("MV7 voice preset applied");
            }
            Err(e) => eprintln!("Warning: MV7 HID not available: {e}"),
        }

        println!("Input: Shure MV7");
    } else {
        // MV7 not connected — fall back to whatever is available
        // If we had a previous non-MV7 selection, keep it
        let current = noise::get_selected_mic();
        if current.as_deref() == Some(mv7_node) || current.is_none() {
            // Find best fallback: prefer headset, then built-in
            let fallback = sources.iter().find(|s| {
                s.node_name.is_some()
                    && !s.name.starts_with("capture.deepfilter")
                    && !s.name.starts_with("deepfilter_source")
                    && !s.name.contains("Digital Microphone") // deprioritize built-in
            }).or_else(|| sources.iter().find(|s| {
                s.node_name.is_some()
                    && !s.name.starts_with("capture.deepfilter")
                    && !s.name.starts_with("deepfilter_source")
            }));

            if let Some(fb) = fallback {
                if let Some(node_name) = &fb.node_name {
                    noise::set_input(node_name).ok();
                    println!("MV7 not found — using: {}", fb.name);
                }
            } else {
                println!("MV7 not found — no fallback mic available");
            }
        } else {
            println!("MV7 not found — keeping current mic");
        }
    }
}

fn cmd_select() {
    use std::process::Command;
    use std::io::Write;

    let sources = pipewire::list_sources();

    // Get the currently active rnnoise input node name
    let active_input = pipewire::get_rnnoise_input_node_name()
        .or_else(|| noise::get_selected_mic());

    // Only show real hardware mics (not rnnoise nodes)
    let selectable: Vec<&pipewire::Source> = sources
        .iter()
        .filter(|s| {
            !s.name.starts_with("capture.deepfilter")
                && !s.name.starts_with("deepfilter_source")
        })
        .collect();

    if selectable.is_empty() {
        eprintln!("No audio sources found");
        std::process::exit(1);
    }

    // Mark the currently active mic
    let menu_items: Vec<String> = selectable
        .iter()
        .map(|s| {
            let is_active = match (&s.node_name, &active_input) {
                (Some(n), Some(a)) => n == a,
                _ => false,
            };
            let marker = if is_active { "* " } else { "  " };
            format!("{}{}", marker, s.name)
        })
        .collect();

    let input = menu_items.join("\n");

    let mut child = Command::new("walker")
        .args(["--dmenu", "--placeholder", "Select input microphone"])
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .spawn()
        .unwrap_or_else(|e| {
            eprintln!("Failed to launch walker: {e}");
            std::process::exit(1);
        });

    child.stdin.as_mut().unwrap().write_all(input.as_bytes()).ok();

    let output = child.wait_with_output().unwrap_or_else(|e| {
        eprintln!("Walker error: {e}");
        std::process::exit(1);
    });

    let selected = String::from_utf8_lossy(&output.stdout).trim().to_string();
    if selected.is_empty() {
        return;
    }

    // Match selection back to a source by display name
    let display_name = selected.trim_start_matches('*').trim();
    let source = selectable.iter().find(|s| s.name == display_name);

    match source {
        Some(s) => {
            if let Some(node_name) = &s.node_name {
                if noise::is_enabled() {
                    // Update rnnoise input target
                    noise::set_input(node_name).unwrap_or_else(|e| {
                        eprintln!("Error: {e}");
                        std::process::exit(1);
                    });
                } else {
                    // No noise suppression — just set as default source
                    noise::set_selected_mic(node_name).ok();
                    pipewire::set_default_source(s.id).unwrap_or_else(|e| {
                        eprintln!("Error: {e}");
                        std::process::exit(1);
                    });
                }
                println!("Input mic set to: {}", s.name);
            } else {
                eprintln!("Could not determine node name for: {}", s.name);
                std::process::exit(1);
            }
        }
        None => {
            eprintln!("Could not match selection: {display_name}");
            std::process::exit(1);
        }
    }
}
