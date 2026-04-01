use std::fs;
use std::path::PathBuf;
use std::process::Command;

const PIPEWIRE_CONF_DIR: &str = ".config/pipewire/pipewire.conf.d";
const FILTER_FILENAME: &str = "99-noise-suppression.conf";
const STATE_DIR: &str = ".config/omarchy-mic";
const SELECTED_MIC_FILE: &str = "selected-mic";
const ENABLED_FILE: &str = "noise-enabled";

fn config_path() -> PathBuf {
    let home = std::env::var("HOME").unwrap_or_else(|_| "/home".to_string());
    PathBuf::from(home).join(PIPEWIRE_CONF_DIR).join(FILTER_FILENAME)
}

fn state_dir() -> PathBuf {
    let home = std::env::var("HOME").unwrap_or_else(|_| "/home".to_string());
    PathBuf::from(home).join(STATE_DIR)
}

fn deepfilter_plugin_path() -> PathBuf {
    let home = std::env::var("HOME").unwrap_or_else(|_| "/home".to_string());
    PathBuf::from(home).join(".ladspa/libdeep_filter_ladspa.so")
}

fn filter_config() -> String {
    let plugin_path = deepfilter_plugin_path();
    let plugin = plugin_path.to_string_lossy();

    format!(
        r#"context.modules = [
    {{   name = libpipewire-module-filter-chain
        args = {{
            node.description = "Noise Cancelling Source"
            media.name        = "Noise Cancelling Source"
            filter.graph = {{
                nodes = [
                    {{
                        type   = ladspa
                        name   = deepfilter
                        plugin = {plugin}
                        label  = deep_filter_mono
                        control = {{
                            "Attenuation Limit (dB)" 12
                        }}
                    }}
                ]
            }}
            audio.rate = 48000
            audio.position = [MONO]
            capture.props = {{
                node.name      = "capture.deepfilter_source"
                node.passive   = true
            }}
            playback.props = {{
                node.name    = "deepfilter_source"
                media.class  = Audio/Source
            }}
        }}
    }}
]
"#
    )
}

/// Check if the filter config file exists (filter is loaded in PipeWire)
pub fn is_loaded() -> bool {
    config_path().exists()
}

/// Check if noise suppression is enabled (user wants it active)
pub fn is_enabled() -> bool {
    let path = state_dir().join(ENABLED_FILE);
    fs::read_to_string(path).ok().map(|s| s.trim() == "true").unwrap_or(false)
}

fn set_enabled_state(enabled: bool) -> Result<(), String> {
    let dir = state_dir();
    fs::create_dir_all(&dir).map_err(|e| format!("Failed to create state dir: {e}"))?;
    fs::write(dir.join(ENABLED_FILE), if enabled { "true" } else { "false" })
        .map_err(|e| format!("Failed to save state: {e}"))
}

pub fn is_plugin_installed() -> bool {
    deepfilter_plugin_path().exists()
        || PathBuf::from("/usr/lib/ladspa/libdeep_filter_ladspa.so").exists()
}

pub fn is_active() -> bool {
    let output = Command::new("wpctl")
        .args(["status"])
        .output()
        .ok();

    output
        .map(|o| String::from_utf8_lossy(&o.stdout).contains("deepfilter_source"))
        .unwrap_or(false)
}

pub fn get_selected_mic() -> Option<String> {
    let path = state_dir().join(SELECTED_MIC_FILE);
    fs::read_to_string(path).ok().map(|s| s.trim().to_string()).filter(|s| !s.is_empty())
}

pub fn set_selected_mic(node_name: &str) -> Result<(), String> {
    let dir = state_dir();
    fs::create_dir_all(&dir).map_err(|e| format!("Failed to create state dir: {e}"))?;
    fs::write(dir.join(SELECTED_MIC_FILE), node_name)
        .map_err(|e| format!("Failed to save selected mic: {e}"))
}

/// Ensure the filter config is installed (for PipeWire to load on next start)
pub fn ensure_config() -> Result<(), String> {
    if !is_plugin_installed() {
        return Err(
            "DeepFilterNet not installed. Download from: https://github.com/Rikorose/DeepFilterNet/releases".to_string()
        );
    }

    let path = config_path();
    if !path.exists() {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).map_err(|e| format!("Failed to create config dir: {e}"))?;
        }
        fs::write(&path, filter_config()).map_err(|e| format!("Failed to write config: {e}"))?;
    }
    Ok(())
}

/// Enable noise suppression: set filter as default source
pub fn enable() -> Result<(), String> {
    ensure_config()?;
    set_enabled_state(true)?;

    if !is_active() {
        // Filter not loaded yet — needs a PipeWire restart (only on first setup)
        return Err("Filter not loaded. Please restart PipeWire once: systemctl --user restart pipewire pipewire-pulse".to_string());
    }

    // Set the filter as default source
    set_filter_as_default()
}

/// Disable noise suppression: set the real mic as default source (filter stays loaded but unused)
pub fn disable() -> Result<(), String> {
    set_enabled_state(false)?;

    // Find the selected mic or any real mic and set it as default
    let selected = get_selected_mic();
    if let Some(node_name) = selected {
        set_source_as_default(&node_name)
    } else {
        // Just leave it — PipeWire will use whatever
        Ok(())
    }
}

fn set_filter_as_default() -> Result<(), String> {
    // Find the deepfilter_source node ID and set it as default
    let output = Command::new("wpctl")
        .args(["status"])
        .output()
        .map_err(|e| format!("wpctl error: {e}"))?;

    let stdout = String::from_utf8_lossy(&output.stdout);
    for line in stdout.lines() {
        if line.contains("deepfilter_source") && !line.contains("capture.deepfilter") {
            // Strip box-drawing chars and find the ID
            let clean: String = line.chars().map(|c| {
                if "│├└─┬┤┘┐┌┼".contains(c) { ' ' } else { c }
            }).collect();
            let trimmed = clean.trim().trim_start_matches('*').trim();
            if let Some(dot) = trimmed.find('.') {
                if let Ok(id) = trimmed[..dot].trim().parse::<u32>() {
                    Command::new("wpctl")
                        .args(["set-default", &id.to_string()])
                        .status()
                        .map_err(|e| format!("wpctl error: {e}"))?;
                    return Ok(());
                }
            }
        }
    }
    Err("Filter node not found".to_string())
}

fn set_source_as_default(node_name: &str) -> Result<(), String> {
    // Find the node ID for this node_name via pw-cli
    let output = Command::new("wpctl")
        .args(["status"])
        .output()
        .map_err(|e| format!("wpctl error: {e}"))?;

    let stdout = String::from_utf8_lossy(&output.stdout);

    // We need to match by checking pw-cli for each source ID
    // Simpler: just iterate sources and find the matching one
    let pw_output = Command::new("pw-link")
        .args(["-o"])
        .output()
        .map_err(|e| format!("pw-link error: {e}"))?;

    let pw_stdout = String::from_utf8_lossy(&pw_output.stdout);
    if !pw_stdout.lines().any(|l| l.starts_with(node_name)) {
        return Err(format!("Node '{node_name}' not found"));
    }

    // Search wpctl status for the display name to find ID
    // This is imperfect but works for common cases
    for line in stdout.lines() {
        let clean = line.replace(['│', '├', '└', '─', '┬', '┤', '┘', '┐', '┌', '┼'], " ");
        let trimmed = clean.trim().trim_start_matches('*').trim();
        if let Some(dot) = trimmed.find('.') {
            if let Ok(id) = trimmed[..dot].trim().parse::<u32>() {
                // Check if this ID matches our node_name
                let info = Command::new("pw-cli")
                    .args(["info", &id.to_string()])
                    .output()
                    .ok();
                if let Some(info) = info {
                    let info_str = String::from_utf8_lossy(&info.stdout);
                    if info_str.contains(node_name) {
                        Command::new("wpctl")
                            .args(["set-default", &id.to_string()])
                            .status()
                            .map_err(|e| format!("wpctl error: {e}"))?;
                        return Ok(());
                    }
                }
            }
        }
    }

    Err(format!("Could not find wpctl ID for '{node_name}'"))
}

/// Dynamically switch the filter input to a different mic using pw-link (no restart)
pub fn set_input(node_name: &str) -> Result<(), String> {
    set_selected_mic(node_name)?;

    if !is_active() {
        return Ok(());
    }

    disconnect_filter_input()?;
    link_input(node_name)
}

fn disconnect_filter_input() -> Result<(), String> {
    let output = Command::new("pw-link")
        .args(["-l"])
        .output()
        .map_err(|e| format!("pw-link error: {e}"))?;

    let stdout = String::from_utf8_lossy(&output.stdout);
    let mut found = false;

    for line in stdout.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with("capture.deepfilter_source:") {
            found = true;
            continue;
        }
        if found && trimmed.starts_with("|<-") {
            let source_port = trimmed.trim_start_matches("|<-").trim();
            for input_port in &["capture.deepfilter_source:input_MONO", "capture.deepfilter_source:input_FL"] {
                Command::new("pw-link")
                    .args(["-d", source_port, input_port])
                    .status()
                    .ok();
            }
            found = false;
        }
        if found && !trimmed.starts_with('|') {
            found = false;
        }
    }
    Ok(())
}

fn link_input(node_name: &str) -> Result<(), String> {
    let output = Command::new("pw-link")
        .args(["-o"])
        .output()
        .map_err(|e| format!("pw-link error: {e}"))?;

    let stdout = String::from_utf8_lossy(&output.stdout);
    let capture_port = stdout.lines().find(|line| {
        line.starts_with(node_name) && line.contains(":capture_")
    });

    let inputs = Command::new("pw-link")
        .args(["-i"])
        .output()
        .map_err(|e| format!("pw-link error: {e}"))?;
    let input_stdout = String::from_utf8_lossy(&inputs.stdout);
    let filter_input = input_stdout.lines().find(|line| {
        line.starts_with("capture.deepfilter_source:input_")
    }).unwrap_or("capture.deepfilter_source:input_MONO");

    match capture_port {
        Some(port) => {
            Command::new("pw-link")
                .args([port, filter_input])
                .status()
                .map_err(|e| format!("Failed to link: {e}"))?;
            Ok(())
        }
        None => Err(format!("No capture port found for '{node_name}'"))
    }
}
