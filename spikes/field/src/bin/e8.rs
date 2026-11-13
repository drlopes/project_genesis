//! E8: classification look and cost (docs/21-phase-1-spike-plan.md#e8--classification-cost-and-look).
//!
//! Renders top-down biome maps of the stub content pack (8 aerial families incl. the barren
//! fallback, 1 ecotone, 1 variant) for one factor at a time around the docs/19 defaults, and
//! measures cost, border width and patch sizes.
//!
//! ```text
//! cargo run --release --bin e8 -- --out ../target/e8 [--size 600] [--extent-km 20]
//! cargo run --release --bin e8 -- --out dir --only --k 4 --mosaic 0.35 --jitter 1   # one map
//! ```
//!
//! Colours: one per family, blended by the family weights (a soft border shows as a blend); the
//! variant is a lighter shade of its family; the ecotone is magenta; water (h0 below the sea) is
//! dark blue. Classification below the sea (benthic) is not part of the stub.

use genesis_core::Seed;
use spike_field::classify::MAXF;
use spike_field::cube::normalize;
use spike_field::{Body, Depth, Sample};
use std::time::Instant;

type Vec3 = [f64; 3];

const COLOURS: [[f32; 3]; 8] = [
    [0.55, 0.55, 0.55], // 0 barren fallback
    [0.20, 0.55, 0.20], // 1 (has the variant; ecotone with 2)
    [0.85, 0.80, 0.30], // 2
    [0.15, 0.60, 0.60], // 3
    [0.90, 0.50, 0.15], // 4
    [0.55, 0.30, 0.70], // 5
    [0.65, 0.25, 0.20], // 6
    [0.35, 0.75, 0.45], // 7
];
const ECOTONE: [f32; 3] = [0.90, 0.20, 0.75];
const WATER: [f32; 3] = [0.08, 0.14, 0.30];

#[derive(Clone, Copy)]
struct Config {
    k: u32,
    mosaic: f32,
    jitter: f32,
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let get = |name: &str| {
        args.iter()
            .position(|a| a == name)
            .and_then(|i| args.get(i + 1))
            .cloned()
    };
    let num = |name: &str, default: f64| get(name).and_then(|v| v.parse().ok()).unwrap_or(default);
    let out = get("--out").unwrap_or_else(|| "../target/e8".into());
    let size = num("--size", 600.0) as usize;
    let extent = num("--extent-km", 20.0) * 1000.0;
    std::fs::create_dir_all(&out).expect("create output directory");

    let mut body = Body::bake_with_radius(Seed(3), 24_000.0, 8, 4, 4, 4, 256);
    // `--overlap`: families 2, 4, 6 take the climate envelope of 1, 3, 5, so each zone has two
    // fully suitable families and only palette weight × mosaic decides between them (docs/19's
    // forest-and-grassland case, which the climate-separated stub never produces).
    if args.iter().any(|a| a == "--overlap") {
        for i in [2usize, 4, 6] {
            let albedo = body.content.families[i].base.albedo;
            body.content.families[i].base = body.content.families[i - 1].base;
            body.content.families[i].base.albedo = albedo;
        }
    }
    let pw: Vec<String> = body
        .content
        .families
        .iter()
        .map(|f| format!("{:.3}", f.palette_weight))
        .collect();
    println!(
        "{{ \"test\": \"palette\", \"palette_weight\": [{}] }}",
        pw.join(", ")
    );
    // The mosaic noise's real range: one field, and the difference of two independent fields
    // (what decides between two competing families).
    {
        let a = Seed(21);
        let b = Seed(22);
        let mut one = Vec::new();
        let mut diff = Vec::new();
        for (i, d) in spike_field::directions(Seed(9), 200_000).iter().enumerate() {
            let q = [
                d[0] * body.radius / 1500.0,
                d[1] * body.radius / 1500.0,
                d[2] * body.radius / 1500.0,
            ];
            let na = genesis_core::noise::gradient3(a, q) as f64;
            let nb = genesis_core::noise::gradient3(b, q) as f64;
            one.push(na);
            diff.push(na - nb);
            let _ = i;
        }
        let stats = |v: &mut Vec<f64>| {
            v.sort_by(f64::total_cmp);
            let n = v.len();
            let mean = v.iter().sum::<f64>() / n as f64;
            let sd = (v.iter().map(|x| (x - mean).powi(2)).sum::<f64>() / n as f64).sqrt();
            (sd, v[n / 100], v[n * 9 / 10], v[n * 99 / 100], v[n - 1])
        };
        let (s1, _, p90a, p99a, maxa) = stats(&mut one);
        let (s2, _, p90d, p99d, maxd) = stats(&mut diff);
        println!(
            "{{ \"test\": \"noise\", \"one\": {{ \"sd\": {s1:.3}, \"p90\": {p90a:.3}, \"p99\": {p99a:.3}, \"max\": {maxa:.3} }}, \
             \"diff\": {{ \"sd\": {s2:.3}, \"p90\": {p90d:.3}, \"p99\": {p99d:.3}, \"max\": {maxd:.3} }} }}"
        );
    }
    let defaults = Config {
        k: 4,
        mosaic: 0.35,
        jitter: 1.0,
    };

    // The window: the land site where the most families meet, at the default settings.
    let t = Instant::now();
    let centre = pick_window(&mut body, defaults, extent);
    println!(
        "{{ \"test\": \"window\", \"centre\": [{:.4}, {:.4}, {:.4}], \"extent_km\": {:.1}, \"size_px\": {}, \"m_per_px\": {:.1}, \"search_ms\": {:.0} }}",
        centre[0],
        centre[1],
        centre[2],
        extent / 1000.0,
        size,
        extent / size as f64,
        t.elapsed().as_secs_f64() * 1000.0
    );

    if args.iter().any(|a| a == "--only") {
        let c = Config {
            k: num("--k", 4.0) as u32,
            mosaic: num("--mosaic", 0.35) as f32,
            jitter: num("--jitter", 1.0) as f32,
        };
        let map = render(&mut body, c, centre, extent, size);
        let name = format!("{out}/k{}-m{}-j{}.png", c.k, c.mosaic, c.jitter);
        save(&name, size, size, &map.rgb);
        report(c, &map, extent / size as f64, &name);
        return;
    }

    // Overview of the whole body at the defaults (equirectangular), with the window marked.
    apply(&mut body, defaults);
    let (w, h) = (1024usize, 512usize);
    let mut rgb = vec![0u8; w * h * 3];
    let mut s = Sample::default();
    for y in 0..h {
        let lat = std::f64::consts::FRAC_PI_2 - (y as f64 + 0.5) / h as f64 * std::f64::consts::PI;
        for x in 0..w {
            let lon = (x as f64 + 0.5) / w as f64 * std::f64::consts::TAU - std::f64::consts::PI;
            let d = [lat.cos() * lon.cos(), lat.sin(), lat.cos() * lon.sin()];
            body.sample(d, 1000.0, Depth::L2, &mut s);
            put(&mut rgb, w, x, y, colour(&body, &s));
        }
    }
    // Mark the window centre.
    let lat = centre[1].asin();
    let lon = centre[2].atan2(centre[0]);
    let cx = ((lon + std::f64::consts::PI) / std::f64::consts::TAU * w as f64) as i64;
    let cy = ((std::f64::consts::FRAC_PI_2 - lat) / std::f64::consts::PI * h as f64) as i64;
    for i in -6i64..=6 {
        for (x, y) in [(cx + i, cy), (cx, cy + i)] {
            if (0..w as i64).contains(&x) && (0..h as i64).contains(&y) {
                put(&mut rgb, w, x as usize, y as usize, [1.0, 1.0, 1.0]);
            }
        }
    }
    save(&format!("{out}/overview.png"), w, h, &rgb);

    // One factor at a time around the defaults; the default sits in the middle of each row.
    let rows: [[Config; 3]; 3] = [
        [2, 4, 6].map(|k| Config { k, ..defaults }),
        [0.2, 0.35, 0.6].map(|mosaic| Config { mosaic, ..defaults }),
        [0.5, 1.0, 2.0].map(|jitter| Config { jitter, ..defaults }),
    ];
    let gap = 6;
    let sheet_w = 3 * size + 2 * gap;
    let mut sheet = vec![255u8; sheet_w * sheet_w * 3];
    for (r, row) in rows.iter().enumerate() {
        for (c, cfg) in row.iter().enumerate() {
            let map = render(&mut body, *cfg, centre, extent, size);
            let name = format!("{out}/k{}-m{}-j{}.png", cfg.k, cfg.mosaic, cfg.jitter);
            save(&name, size, size, &map.rgb);
            report(*cfg, &map, extent / size as f64, &name);
            for y in 0..size {
                let dst = ((r * (size + gap) + y) * sheet_w + c * (size + gap)) * 3;
                let src = y * size * 3;
                sheet[dst..dst + size * 3].copy_from_slice(&map.rgb[src..src + size * 3]);
            }
        }
    }
    save(&format!("{out}/sheet.png"), sheet_w, sheet_w, &sheet);

    // Cost: L2 increment over L1, per config (k changes the power; mosaic and jitter change
    // values, not work, so they are measured as a check).
    let dirs = spike_field::directions(Seed(5), 100_000);
    for cfg in [rows[0][0], rows[0][1], rows[0][2], rows[1][2], rows[2][2]] {
        apply(&mut body, cfg);
        let l1 = time(&body, &dirs, Depth::L1);
        let l2 = time(&body, &dirs, Depth::L2);
        println!(
            "{{ \"test\": \"cost\", \"k\": {}, \"mosaic\": {}, \"jitter\": {}, \"l1_ns\": {:.0}, \"l2_ns\": {:.0}, \"l2_increment_ns\": {:.0} }}",
            cfg.k,
            cfg.mosaic,
            cfg.jitter,
            l1,
            l2,
            l2 - l1
        );
    }
}

fn apply(body: &mut Body, c: Config) {
    body.content.k = c.k;
    body.content.set_mosaic_strength(c.mosaic);
    body.jitter = c.jitter;
}

/// Min over 5 runs of the mean ns per sample.
fn time(body: &Body, dirs: &[Vec3], depth: Depth) -> f64 {
    let mut s = Sample::default();
    let mut best = f64::INFINITY;
    let mut sink = 0.0f32;
    for _ in 0..5 {
        let t = Instant::now();
        for d in dirs {
            body.sample(*d, 1000.0, depth, &mut s);
            sink += s.temp;
        }
        best = best.min(t.elapsed().as_secs_f64() * 1e9 / dirs.len() as f64);
    }
    std::hint::black_box(sink);
    best
}

fn frame(up: Vec3) -> (Vec3, Vec3) {
    let r = if up[1].abs() < 0.9 {
        [0.0, 1.0, 0.0]
    } else {
        [1.0, 0.0, 0.0]
    };
    let east = normalize([
        up[1] * r[2] - up[2] * r[1],
        up[2] * r[0] - up[0] * r[2],
        up[0] * r[1] - up[1] * r[0],
    ]);
    let north = [
        up[1] * east[2] - up[2] * east[1],
        up[2] * east[0] - up[0] * east[2],
        up[0] * east[1] - up[1] * east[0],
    ];
    (east, north)
}

/// Direction of the tangent-plane point (x, y) metres from `c` (gnomonic: straight lines stay
/// straight; arc lengths at the window edge are a few per cent short).
fn at(body: &Body, c: Vec3, e: Vec3, n: Vec3, x: f64, y: f64) -> Vec3 {
    let r = body.radius;
    normalize([
        c[0] * r + x * e[0] + y * n[0],
        c[1] * r + x * e[1] + y * n[1],
        c[2] * r + x * e[2] + y * n[2],
    ])
}

/// Dominant class: a family index, `n` for the ecotone, or `None` for water.
fn dominant(body: &Body, s: &Sample) -> Option<(usize, f32)> {
    if s.height < body.sea_surface {
        return None;
    }
    let n = s.weights.n;
    let mut best = (0usize, -1.0f32);
    for i in 0..n {
        if s.weights.family[i] > best.1 {
            best = (i, s.weights.family[i]);
        }
    }
    if s.weights.family[MAXF] > best.1 {
        best = (n, s.weights.family[MAXF]);
    }
    Some(best)
}

fn colour(body: &Body, s: &Sample) -> [f32; 3] {
    if s.height < body.sea_surface {
        return WATER;
    }
    let mut c = [0.0f32; 3];
    for i in 0..s.weights.n {
        let [wb, wv] = s.weights.leaf[i];
        let base = COLOURS[i % COLOURS.len()];
        let light = base.map(|v| v + 0.45 * (1.0 - v));
        for k in 0..3 {
            c[k] += wb * base[k] + wv * light[k];
        }
    }
    for k in 0..3 {
        c[k] += s.weights.family[MAXF] * ECOTONE[k];
    }
    c
}

/// Land site (≥ 80 % land) where the most families each cover ≥ 3 % of the window; ties go to
/// the most even mix. Evaluated on a 16 × 16 grid per candidate.
fn pick_window(body: &mut Body, defaults: Config, extent: f64) -> Vec3 {
    apply(body, defaults);
    let mut s = Sample::default();
    let mut best = (0usize, f64::NEG_INFINITY, [0.0, 1.0, 0.0]);
    for c in spike_field::directions(Seed(11), 1500) {
        let (e, n) = frame(c);
        let mut counts = [0usize; MAXF + 1];
        let mut land = 0usize;
        for j in 0..16 {
            for i in 0..16 {
                let x = (i as f64 / 15.0 - 0.5) * extent;
                let y = (j as f64 / 15.0 - 0.5) * extent;
                body.sample(at(body, c, e, n, x, y), 500.0, Depth::L2, &mut s);
                if let Some((d, _)) = dominant(body, &s) {
                    counts[d] += 1;
                    land += 1;
                }
            }
        }
        if land < 205 {
            continue;
        }
        let present = counts.iter().filter(|&&k| k * 100 >= 3 * land).count();
        let entropy: f64 = counts
            .iter()
            .filter(|&&k| k > 0)
            .map(|&k| {
                let p = k as f64 / land as f64;
                -p * p.ln()
            })
            .sum();
        if present > best.0 || (present == best.0 && entropy > best.1) {
            best = (present, entropy, c);
        }
    }
    best.2
}

struct Map {
    rgb: Vec<u8>,
    /// Dominant class per pixel (`u8::MAX` = water) and its weight.
    class: Vec<u8>,
    top: Vec<f32>,
    size: usize,
    ms: f64,
}

fn render(body: &mut Body, cfg: Config, c: Vec3, extent: f64, size: usize) -> Map {
    apply(body, cfg);
    let (e, n) = frame(c);
    let px = extent / size as f64;
    let mut map = Map {
        rgb: vec![0; size * size * 3],
        class: vec![u8::MAX; size * size],
        top: vec![0.0; size * size],
        size,
        ms: 0.0,
    };
    let mut s = Sample::default();
    let t = Instant::now();
    for y in 0..size {
        for x in 0..size {
            let lx = (x as f64 + 0.5) * px - extent / 2.0;
            let ly = extent / 2.0 - (y as f64 + 0.5) * px; // north up
            body.sample(at(body, c, e, n, lx, ly), px, Depth::L2, &mut s);
            put(&mut map.rgb, size, x, y, colour(body, &s));
            if let Some((d, w)) = dominant(body, &s) {
                map.class[y * size + x] = d as u8;
                map.top[y * size + x] = w;
            }
        }
    }
    map.ms = t.elapsed().as_secs_f64() * 1000.0;
    map
}

fn report(cfg: Config, map: &Map, px: f64, name: &str) {
    let n = map.size;
    let px_area = px * px;
    let land = map.class.iter().filter(|&&c| c != u8::MAX).count();
    // Transition: land pixels whose dominant class has < 90 % of the weight.
    let transition = map
        .class
        .iter()
        .zip(&map.top)
        .filter(|(c, w)| **c != u8::MAX && **w < 0.9)
        .count();
    // Border length: neighbouring land pixels with different dominant classes.
    let mut edges = 0usize;
    for y in 0..n {
        for x in 0..n {
            let a = map.class[y * n + x];
            if a == u8::MAX {
                continue;
            }
            if x + 1 < n {
                let b = map.class[y * n + x + 1];
                if b != u8::MAX && b != a {
                    edges += 1;
                }
            }
            if y + 1 < n {
                let b = map.class[(y + 1) * n + x];
                if b != u8::MAX && b != a {
                    edges += 1;
                }
            }
        }
    }
    let border_km = edges as f64 * px / 1000.0;
    let width_m = if edges > 0 {
        transition as f64 * px_area / (edges as f64 * px)
    } else {
        f64::NAN
    };
    // Patches: 4-connected components of the dominant class.
    let mut seen = vec![false; n * n];
    let mut areas: Vec<f64> = Vec::new();
    let mut stack = Vec::new();
    for start in 0..n * n {
        if seen[start] || map.class[start] == u8::MAX {
            continue;
        }
        let cls = map.class[start];
        let mut count = 0usize;
        seen[start] = true;
        stack.push(start);
        while let Some(i) = stack.pop() {
            count += 1;
            let (x, y) = (i % n, i / n);
            let mut visit = |j: usize| {
                if !seen[j] && map.class[j] == cls {
                    seen[j] = true;
                    stack.push(j);
                }
            };
            if x > 0 {
                visit(i - 1);
            }
            if x + 1 < n {
                visit(i + 1);
            }
            if y > 0 {
                visit(i - n);
            }
            if y + 1 < n {
                visit(i + n);
            }
        }
        areas.push(count as f64 * px_area / 1e6);
    }
    areas.sort_by(f64::total_cmp);
    let q = |p: f64| {
        if areas.is_empty() {
            f64::NAN
        } else {
            areas[((areas.len() - 1) as f64 * p) as usize]
        }
    };
    let specks = areas.iter().filter(|&&a| a < 0.05).count();
    let mut classes = [0usize; MAXF + 1];
    for &c in &map.class {
        if c != u8::MAX {
            classes[c as usize] += 1;
        }
    }
    let present: Vec<String> = classes
        .iter()
        .enumerate()
        .filter(|(_, k)| **k > 0)
        .map(|(i, k)| format!("\"{}\": {:.1}", i, 100.0 * *k as f64 / land.max(1) as f64))
        .collect();
    println!(
        "{{ \"test\": \"map\", \"k\": {}, \"mosaic\": {}, \"jitter\": {}, \"file\": \"{}\", \"render_ms\": {:.0}, \
         \"land_pct\": {:.1}, \"transition_pct\": {:.1}, \"border_km\": {:.1}, \"border_width_m\": {:.0}, \
         \"patches\": {}, \"patch_km2\": {{ \"p50\": {:.3}, \"p90\": {:.2}, \"max\": {:.1} }}, \"specks_lt_0_05_km2\": {}, \
         \"class_pct\": {{ {} }} }}",
        cfg.k,
        cfg.mosaic,
        cfg.jitter,
        name,
        map.ms,
        100.0 * land as f64 / (n * n) as f64,
        100.0 * transition as f64 / land.max(1) as f64,
        border_km,
        width_m,
        areas.len(),
        q(0.5),
        q(0.9),
        areas.last().copied().unwrap_or(f64::NAN),
        specks,
        present.join(", ")
    );
}

fn put(rgb: &mut [u8], w: usize, x: usize, y: usize, c: [f32; 3]) {
    let i = (y * w + x) * 3;
    for k in 0..3 {
        rgb[i + k] = (c[k].clamp(0.0, 1.0) * 255.0).round() as u8;
    }
}

fn save(path: &str, w: usize, h: usize, rgb: &[u8]) {
    let file = std::fs::File::create(path).expect("create png");
    let mut enc = png::Encoder::new(std::io::BufWriter::new(file), w as u32, h as u32);
    enc.set_color(png::ColorType::Rgb);
    enc.set_depth(png::BitDepth::Eight);
    let mut writer = enc.write_header().expect("png header");
    writer.write_image_data(rgb).expect("png data");
}
