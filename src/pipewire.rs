use std::process::Command;

#[derive(Debug)]
pub struct Source {
    pub id: u32,
    pub name: String,
    pub node_name: Option<String>,
    pub is_default: bool,
}

fn strip_box_drawing(line: &str) -> String {
    let mut result = String::new();
    for ch in line.chars() {
        match ch {
            '│' | '├' | '└' | '─' | '┬' | '┤' | '┘' | '┐' | '┌' | '┼' => result.push(' '),
            _ => result.push(ch),
        }
    }
    result
}

/// Get node.name for a given wpctl ID by inspecting pw-cli
fn get_node_name(id: u32) -> Option<String> {
    let output = Command::new("pw-cli")
        .args(["info", &id.to_string()])
        .output()
        .ok()?;
    let stdout = String::from_utf8_lossy(&output.stdout);
    for line in stdout.lines() {
        let trimmed = line.trim().trim_start_matches('*').trim();
        if trimmed.starts_with("node.name") {
            if let Some(val) = trimmed.split('=').nth(1) {
                let clean = val.trim().trim_matches('"').to_string();
                return Some(clean);
            }
        }
    }
    None
}

pub fn list_sources() -> Vec<Source> {
    let output = Command::new("wpctl")
        .args(["status"])
        .output()
        .ok();

    let stdout = match output {
        Some(o) => String::from_utf8_lossy(&o.stdout).to_string(),
        None => return Vec::new(),
    };

    let mut sources = Vec::new();
    let mut in_audio_sources = false;
    let mut in_audio_filters = false;
    let mut in_audio = false;

    for line in stdout.lines() {
        let clean = strip_box_drawing(line);
        let trimmed = clean.trim();

        // Track the Audio section (skip Video)
        if trimmed == "Audio" {
            in_audio = true;
            continue;
        }
        if trimmed == "Video" {
            in_audio = false;
            in_audio_sources = false;
            in_audio_filters = false;
            continue;
        }

        if !in_audio {
            continue;
        }

        if trimmed.starts_with("Sources:") {
            in_audio_sources = true;
            in_audio_filters = false;
            continue;
        }
        if trimmed.starts_with("Filters:") {
            in_audio_sources = false;
            in_audio_filters = true;
            continue;
        }
        if trimmed.starts_with("Sinks:") || trimmed.starts_with("Streams:")
            || trimmed.starts_with("Devices:")
        {
            in_audio_sources = false;
            in_audio_filters = false;
            continue;
        }

        if trimmed.is_empty() {
            continue;
        }

        if (in_audio_sources || in_audio_filters) && !trimmed.starts_with('-') {
            let is_default = trimmed.starts_with('*');
            let clean_line = trimmed.trim_start_matches('*').trim();

            if let Some(dot_pos) = clean_line.find('.') {
                if let Ok(id) = clean_line[..dot_pos].trim().parse::<u32>() {
                    let rest = clean_line[dot_pos + 1..].trim();
                    let name = if let Some(bracket) = rest.rfind('[') {
                        rest[..bracket].trim().to_string()
                    } else {
                        rest.to_string()
                    };

                    if !name.is_empty() {
                        let node_name = get_node_name(id);
                        sources.push(Source { id, name, is_default, node_name });
                    }
                }
            }
        }
    }

    sources
}

pub fn set_default_source(id: u32) -> Result<(), String> {
    Command::new("wpctl")
        .args(["set-default", &id.to_string()])
        .status()
        .map_err(|e| format!("wpctl error: {e}"))?;
    Ok(())
}

pub fn get_default_source_name() -> Option<String> {
    list_sources().into_iter().find(|s| s.is_default).map(|s| s.name)
}

/// Resolve a PipeWire node.name to its display name
pub fn resolve_display_name(node_name: &str) -> Option<String> {
    list_sources()
        .into_iter()
        .find(|s| s.node_name.as_deref() == Some(node_name))
        .map(|s| s.name)
}

/// Get the active physical input feeding rnnoise by checking pw-link
pub fn get_rnnoise_input() -> Option<String> {
    let output = Command::new("pw-link")
        .args(["-l"])
        .output()
        .ok()?;
    let stdout = String::from_utf8_lossy(&output.stdout);

    // Look for: capture.deepfilter_source:input_MONO
    //             |<- some_alsa_input:capture_MONO
    let mut found_rnnoise = false;
    for line in stdout.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with("capture.deepfilter_source:") {
            found_rnnoise = true;
            continue;
        }
        if found_rnnoise && trimmed.starts_with("|<-") {
            let node_port = trimmed.trim_start_matches("|<-").trim();
            if let Some(colon) = node_port.find(':') {
                let node_name = &node_port[..colon];
                // Resolve to display name
                return resolve_display_name(node_name)
                    .or_else(|| Some(node_name.to_string()));
            }
        }
        if found_rnnoise && !trimmed.starts_with("|") {
            found_rnnoise = false;
        }
    }
    None
}

/// Get the raw node.name of the active rnnoise input (not display name)
pub fn get_rnnoise_input_node_name() -> Option<String> {
    let output = Command::new("pw-link")
        .args(["-l"])
        .output()
        .ok()?;
    let stdout = String::from_utf8_lossy(&output.stdout);

    let mut found = false;
    for line in stdout.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with("capture.deepfilter_source:") {
            found = true;
            continue;
        }
        if found && trimmed.starts_with("|<-") {
            let node_port = trimmed.trim_start_matches("|<-").trim();
            if let Some(colon) = node_port.find(':') {
                return Some(node_port[..colon].to_string());
            }
        }
        if found && !trimmed.starts_with('|') {
            found = false;
        }
    }
    None
}
