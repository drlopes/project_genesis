//! Per-frame recording and the end-of-run summary written as JSON.

use std::fmt::Write as _;

#[derive(Default)]
pub struct Frame {
    pub t: f64,
    pub frame_ms: f32,
    pub terrain_ms: f32,
    pub displayed: u32,
    pub resident: u32,
    pub triangles: u64,
    pub queued: u32,
    pub in_flight: u32,
    /// Resident terrain vertex + index data (MB), and the same patches in E3's float format.
    pub mesh_mb: u32,
    pub float_format_mb: u32,
    pub missing: u32,
    pub altitude: f32,
    /// Window unfocused or minimised: the OS may throttle frames, so the run is not valid.
    pub disturbed: bool,
}

#[derive(Default)]
pub struct Recorder {
    pub frames: Vec<Frame>,
    pub patch_build_ms: Vec<f32>,
    pub patches_built: u64,
    pub patches_discarded: u64,
}

fn pct(sorted: &[f32], p: f64) -> f32 {
    if sorted.is_empty() {
        return 0.0;
    }
    let i = ((sorted.len() - 1) as f64 * p).round() as usize;
    sorted[i]
}

impl Recorder {
    /// Per-frame data as CSV, for finding where on the path a peak happens.
    pub fn frames_csv(&self) -> String {
        let mut s = String::from(
            "t,altitude_m,frame_ms,terrain_ms,displayed,resident,triangles,queued,in_flight,missing\n",
        );
        for f in &self.frames {
            let _ = writeln!(
                s,
                "{:.3},{:.1},{:.3},{:.3},{},{},{},{},{},{}",
                f.t,
                f.altitude,
                f.frame_ms,
                f.terrain_ms,
                f.displayed,
                f.resident,
                f.triangles,
                f.queued,
                f.in_flight,
                f.missing
            );
        }
        s
    }

    pub fn summary_json(
        &self,
        config: &str,
        ram_mb: Option<f64>,
        vram: &str,
        duration_s: f64,
    ) -> String {
        // Skip the first second: shader compilation and the initial burst are not steady state.
        let steady: Vec<&Frame> = self.frames.iter().filter(|f| f.t > 1.0).collect();
        let mut ft: Vec<f32> = steady.iter().map(|f| f.frame_ms).collect();
        let mut tm: Vec<f32> = steady.iter().map(|f| f.terrain_ms).collect();
        let mut pb = self.patch_build_ms.clone();
        ft.sort_by(f32::total_cmp);
        tm.sort_by(f32::total_cmp);
        pb.sort_by(f32::total_cmp);
        let max_of = |f: fn(&Frame) -> u64| steady.iter().map(|x| f(x)).max().unwrap_or(0);
        let missing_frames = steady.iter().filter(|f| f.missing > 0).count();
        let disturbed_frames = steady.iter().filter(|f| f.disturbed).count();
        let mut s = String::new();
        let _ = write!(
            s,
            "{{\n  \"config\": {config},\n  \"duration_s\": {duration_s:.1},\n  \"frames\": {},\n  \
             \"frame_ms\": {{ \"p50\": {:.2}, \"p99\": {:.2}, \"p999\": {:.2}, \"max\": {:.2} }},\n  \
             \"terrain_main_ms\": {{ \"p50\": {:.3}, \"p99\": {:.3}, \"max\": {:.3} }},\n  \
             \"patch_build_ms\": {{ \"p50\": {:.2}, \"p99\": {:.2}, \"count\": {}, \"discarded_stale\": {} }},\n  \
             \"max_displayed_patches\": {},\n  \"max_resident_patches\": {},\n  \"max_triangles\": {},\n  \
             \"max_queued\": {},\n  \"max_terrain_mesh_mb\": {},\n  \
             \"max_terrain_mesh_mb_float_format\": {},\n  \"frames_with_missing_patches\": {},\n  \
             \"disturbed_frames\": {},\n  \"valid\": {},\n  \
             \"ram_mb\": {},\n  \"vram\": \"{vram}\"\n}}\n",
            steady.len(),
            pct(&ft, 0.5),
            pct(&ft, 0.99),
            pct(&ft, 0.999),
            ft.last().copied().unwrap_or(0.0),
            pct(&tm, 0.5),
            pct(&tm, 0.99),
            tm.last().copied().unwrap_or(0.0),
            pct(&pb, 0.5),
            pct(&pb, 0.99),
            self.patches_built,
            self.patches_discarded,
            max_of(|f| f.displayed as u64),
            max_of(|f| f.resident as u64),
            max_of(|f| f.triangles),
            max_of(|f| f.queued as u64),
            max_of(|f| f.mesh_mb as u64),
            max_of(|f| f.float_format_mb as u64),
            missing_frames,
            disturbed_frames,
            disturbed_frames == 0,
            ram_mb.map_or("null".to_string(), |m| format!("{m:.0}")),
        );
        s
    }
}

/// Used GPU memory in MiB from `nvidia-smi`, if available.
pub fn vram_used_mib() -> Option<u64> {
    let out = std::process::Command::new("nvidia-smi")
        .args(["--query-gpu=memory.used", "--format=csv,noheader,nounits"])
        .output()
        .ok()?;
    String::from_utf8(out.stdout)
        .ok()?
        .trim()
        .lines()
        .next()?
        .trim()
        .parse()
        .ok()
}

/// Polls `nvidia-smi` once a second on its own thread and keeps the peak. The end-of-run snapshot
/// alone varied 2.0–2.8 GB for the same configuration in E3.
pub fn start_vram_sampler() -> std::sync::Arc<std::sync::atomic::AtomicU64> {
    use std::sync::atomic::{AtomicU64, Ordering};
    let peak = std::sync::Arc::new(AtomicU64::new(0));
    let p = peak.clone();
    std::thread::Builder::new()
        .name("vram-sampler".into())
        .spawn(move || {
            loop {
                if let Some(v) = vram_used_mib() {
                    p.fetch_max(v, Ordering::Relaxed);
                }
                std::thread::sleep(std::time::Duration::from_secs(1));
            }
        })
        .expect("spawn sampler");
    peak
}

/// Used and total GPU memory from `nvidia-smi`, if available. Includes other processes, so the
/// run records a baseline at start-up for comparison.
pub fn vram() -> String {
    std::process::Command::new("nvidia-smi")
        .args([
            "--query-gpu=memory.used,memory.total",
            "--format=csv,noheader",
        ])
        .output()
        .ok()
        .and_then(|o| String::from_utf8(o.stdout).ok())
        .map(|s| s.trim().to_string())
        .unwrap_or_else(|| "unavailable".to_string())
}

/// Process working set in MB, read through PowerShell's `Get-Process` because `std` has no API for
/// it. Spike-only and Windows-only; returns `None` if the query fails.
pub fn ram_mb() -> Option<f64> {
    let pid = std::process::id();
    let out = std::process::Command::new("powershell")
        .args([
            "-NoProfile",
            "-Command",
            &format!("(Get-Process -Id {pid}).WorkingSet64"),
        ])
        .output()
        .ok()?;
    String::from_utf8(out.stdout)
        .ok()?
        .trim()
        .parse::<f64>()
        .ok()
        .map(|b| b / 1e6)
}
