use hidapi::{HidApi, HidDevice};
use std::fmt;
use std::thread;
use std::time::Duration;

const SHURE_VENDOR_ID: u16 = 0x14ED;
const MV7_PRODUCT_ID: u16 = 0x1012;
const MV7_DATA_INTERFACE: i32 = 3;
const HID_REPORT_SIZE: usize = 64;

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Compressor {
    Off = 0,
    Light = 1,
    Medium = 2,
    Heavy = 3,
}

impl fmt::Display for Compressor {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            Compressor::Off => write!(f, "off"),
            Compressor::Light => write!(f, "light"),
            Compressor::Medium => write!(f, "medium"),
            Compressor::Heavy => write!(f, "heavy"),
        }
    }
}

impl Compressor {
    pub fn from_str(s: &str) -> Option<Self> {
        match s.to_lowercase().as_str() {
            "off" | "0" => Some(Compressor::Off),
            "light" | "1" => Some(Compressor::Light),
            "medium" | "2" => Some(Compressor::Medium),
            "heavy" | "3" => Some(Compressor::Heavy),
            _ => None,
        }
    }

    fn from_hex(s: &str) -> Option<Self> {
        let val = u32::from_str_radix(s.trim(), 16).ok()?;
        match val {
            0 => Some(Compressor::Off),
            1 => Some(Compressor::Light),
            2 => Some(Compressor::Medium),
            3 => Some(Compressor::Heavy),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum DspMode {
    Manual = 1,
    Auto = 2,
}

impl fmt::Display for DspMode {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            DspMode::Manual => write!(f, "manual"),
            DspMode::Auto => write!(f, "auto"),
        }
    }
}

#[derive(Debug, Clone)]
pub struct Mv7State {
    pub firmware_version: Option<String>,
    pub serial_number: Option<String>,
    pub mode: Option<DspMode>,
    pub input_gain: Option<f32>,
    pub input_mute: Option<bool>,
    pub monitor_volume: Option<f32>,
    pub monitor_mute: Option<bool>,
    pub compressor: Option<Compressor>,
    pub limiter: Option<bool>,
    pub high_pass_filter: Option<bool>,
    pub presence_filter: Option<bool>,
}

pub struct Mv7 {
    device: HidDevice,
}

impl Mv7 {
    pub fn open() -> Result<Self, String> {
        let api = HidApi::new().map_err(|e| format!("Failed to init HID: {e}"))?;

        for info in api.device_list() {
            if info.vendor_id() == SHURE_VENDOR_ID
                && info.product_id() == MV7_PRODUCT_ID
                && info.interface_number() == MV7_DATA_INTERFACE
            {
                let device = info
                    .open_device(&api)
                    .map_err(|e| format!("Failed to open MV7: {e}"))?;
                let mut mv7 = Mv7 { device };
                mv7.init()?;
                return Ok(mv7);
            }
        }

        Err("No Shure MV7 found".to_string())
    }

    pub fn is_available() -> bool {
        let Ok(api) = HidApi::new() else { return false };
        let result = api.device_list().any(|info| {
            info.vendor_id() == SHURE_VENDOR_ID
                && info.product_id() == MV7_PRODUCT_ID
                && info.interface_number() == MV7_DATA_INTERFACE
        });
        result
    }

    fn init(&mut self) -> Result<(), String> {
        // Authenticate as admin
        self.send("su adm")?;
        self.wait_for("su=adm", Duration::from_secs(3))?;

        // Wait for DSP boot
        self.send("bootDSP C")?;
        self.wait_for("dspBooted", Duration::from_secs(5))?;

        Ok(())
    }

    fn send(&self, cmd: &str) -> Result<(), String> {
        let mut buf = [0u8; HID_REPORT_SIZE];
        for (i, b) in cmd.bytes().enumerate().take(HID_REPORT_SIZE) {
            buf[i] = b;
        }
        self.device
            .write(&buf)
            .map_err(|e| format!("HID write error: {e}"))?;
        Ok(())
    }

    fn read(&self, timeout_ms: i32) -> Option<String> {
        let mut buf = [0u8; HID_REPORT_SIZE];
        let len = self.device.read_timeout(&mut buf, timeout_ms).ok()?;
        if len == 0 {
            return None;
        }
        let end = buf.iter().position(|&b| b == 0).unwrap_or(len);
        Some(String::from_utf8_lossy(&buf[..end]).to_string())
    }

    fn wait_for(&self, needle: &str, timeout: Duration) -> Result<(), String> {
        let start = std::time::Instant::now();
        while start.elapsed() < timeout {
            if let Some(msg) = self.read(200) {
                if msg.contains(needle) {
                    return Ok(());
                }
            }
        }
        Err(format!("Timeout waiting for '{needle}'"))
    }

    fn query(&self, cmd: &str) -> Result<Vec<String>, String> {
        self.send(cmd)?;
        thread::sleep(Duration::from_millis(300));

        let mut messages = Vec::new();
        while let Some(msg) = self.read(200) {
            messages.push(msg.trim().to_string());
        }
        Ok(messages)
    }

    fn query_value(&self, cmd: &str, prefix: &str) -> Result<String, String> {
        let messages = self.query(cmd)?;
        for msg in &messages {
            if let Some(val) = msg.strip_prefix(&format!("{prefix}=")) {
                return Ok(val.to_string());
            }
            if msg.starts_with("block ") {
                let rest = &msg[6..];
                if let Some(val) = rest.strip_prefix(&format!("{prefix} ")) {
                    return Ok(val.to_string());
                }
            }
        }
        Err(format!("No response for '{cmd}' with prefix '{prefix}'"))
    }

    pub fn get_state(&self) -> Mv7State {
        let firmware_version = self.query_value("fwVersion", "fwVersion").ok();
        let serial_number = self.query_value("serialNum", "serialNum").ok();

        let mode = self.query_value("dspMode", "dspMode").ok().map(|v| {
            if v == "1" { DspMode::Manual } else { DspMode::Auto }
        });

        let input_gain = self.query_value("inputGain", "inputGain").ok().and_then(|v| {
            v.trim_end_matches("dB").parse::<f32>().ok()
        });

        let input_mute = self.query_value("micMute", "micMute").ok().map(|v| v == "on");
        let monitor_volume = self.query_value("volume", "volume").ok().and_then(|v| {
            v.trim_end_matches("dB").parse::<f32>().ok()
        });
        let monitor_mute = self.query_value("audioMute", "audioMute").ok().map(|v| v == "on");

        let compressor = self.query_value("getBlock 19", "19").ok().and_then(|v| {
            Compressor::from_hex(&v)
        });

        let limiter = self.query_value("getBlock 1F", "1F").ok().map(|v| v == "00000001");

        let eq_bits = self.query_value("getBlock 31", "31").ok().and_then(|v| {
            u32::from_str_radix(v.trim(), 16).ok()
        });

        let high_pass_filter = eq_bits.map(|b| b & 1 != 0);
        let presence_filter = eq_bits.map(|b| b & 2 != 0);

        Mv7State {
            firmware_version,
            serial_number,
            mode,
            input_gain,
            input_mute,
            monitor_volume,
            monitor_mute,
            compressor,
            limiter,
            high_pass_filter,
            presence_filter,
        }
    }

    pub fn set_compressor(&self, state: Compressor) -> Result<(), String> {
        let val = format!("{:08}", state as u32);
        self.send(&format!("setBlock 19 {val}"))
    }

    pub fn set_limiter(&self, on: bool) -> Result<(), String> {
        let val = if on { "00000001" } else { "00000000" };
        self.send(&format!("setBlock 1F {val}"))
    }

    pub fn set_high_pass_filter(&self, on: bool) -> Result<(), String> {
        // Read current EQ state to preserve the other bit
        let current = self.query_value("getBlock 31", "31")
            .ok()
            .and_then(|v| u32::from_str_radix(v.trim(), 16).ok())
            .unwrap_or(0);

        let val = if on { current | 1 } else { current & !1 };
        self.send(&format!("setBlock 31 {:08}", val))
    }

    pub fn set_presence_filter(&self, on: bool) -> Result<(), String> {
        let current = self.query_value("getBlock 31", "31")
            .ok()
            .and_then(|v| u32::from_str_radix(v.trim(), 16).ok())
            .unwrap_or(0);

        let val = if on { current | 2 } else { current & !2 };
        self.send(&format!("setBlock 31 {:08}", val))
    }

    pub fn set_input_gain(&self, db: f32) -> Result<(), String> {
        let clamped = db.clamp(0.0, 36.0);
        self.send(&format!("inputGain {clamped:.2}"))
    }

    pub fn set_dsp_mode(&self, mode: DspMode) -> Result<(), String> {
        self.send(&format!("dspMode {}", mode as u32))
    }

    pub fn identify(&self) -> Result<(), String> {
        self.send("identify")
    }
}
