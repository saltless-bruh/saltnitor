//! `nvidia-smi` output parsing. A value that could not be measured is an error with a reason,
//! never a `0` or a made-up capacity (INV-18, REQ-TUI-006/AC1).

/// `--query-gpu=memory.used,temperature.gpu,power.draw,power.limit,utilization.gpu,
/// utilization.memory,fan.speed,clocks.gr,clocks.mem` (csv, noheader, nounits).
#[derive(Debug, Clone, PartialEq)]
pub struct GpuSample {
    pub vram_used_gb: f64,
    pub temp_c: i32,
    pub power: String,
    pub util: String,
    pub vram_util: String,
    pub fan: String,
    pub clocks: String,
}

/// Number of comma-separated fields in the sample query above.
const SAMPLE_FIELDS: usize = 9;

/// Parse one sample line. `Err` carries the operator-facing reason.
pub fn parse_sample(stdout: &str) -> Result<GpuSample, String> {
    let line = stdout.trim();
    let parts: Vec<&str> = line.split(", ").collect();
    if parts.len() < SAMPLE_FIELDS {
        return Err(format!(
            "nvidia-smi returned {} of {SAMPLE_FIELDS} fields: {line:?}",
            parts.len()
        ));
    }
    let used_mib: f64 = parts[0]
        .parse()
        .map_err(|_| format!("memory.used is not a number: {:?}", parts[0]))?;
    let temp_c: i32 = parts[1]
        .parse()
        .map_err(|_| format!("temperature.gpu is not a number: {:?}", parts[1]))?;
    Ok(GpuSample {
        vram_used_gb: used_mib / 1024.0,
        temp_c,
        power: format!("{}W / {}W", parts[2], parts[3]),
        util: parts[4].to_string(),
        vram_util: parts[5].to_string(),
        fan: parts[6].to_string(),
        clocks: format!("{} MHz / {} MHz", parts[7], parts[8]),
    })
}

/// `--query-gpu=name,memory.total`: the card's name and total VRAM in GB.
pub fn parse_device(stdout: &str) -> Result<(String, f64), String> {
    let line = stdout.trim();
    let parts: Vec<&str> = line.split(", ").collect();
    if parts.len() != 2 {
        return Err(format!(
            "expected `name, memory.total` from nvidia-smi, got {line:?}"
        ));
    }
    let total_mib: f64 = parts[1]
        .parse()
        .map_err(|_| format!("memory.total is not a number: {:?}", parts[1]))?;
    Ok((parts[0].to_string(), total_mib / 1024.0))
}
