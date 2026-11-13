//! E7 harness: build a voxel region, mesh it, edit it, and measure the seam with the heightfield.
//!
//! ```text
//! cargo run --release -p spike_voxel -- --radius 64 --voxel 0.5 --edits --seam
//! ```

use genesis_core::Seed;
use spike_field::Body;
use spike_voxel::{Edit, Site, Voxels, mesh};
use std::collections::HashSet;
use std::time::Instant;

struct Args {
    radius: f64,
    voxel: f64,
    blocky: bool,
    edits: bool,
    seam: bool,
    hf_spacing: f64,
}

impl Args {
    fn parse() -> Args {
        let a: Vec<String> = std::env::args().collect();
        let num = |flag: &str, default: f64| {
            a.iter()
                .position(|x| x == flag)
                .and_then(|i| a.get(i + 1))
                .and_then(|v| v.parse().ok())
                .unwrap_or(default)
        };
        Args {
            radius: num("--radius", 64.0),
            voxel: num("--voxel", 0.5),
            blocky: a.iter().any(|x| x == "--blocky"),
            edits: a.iter().any(|x| x == "--edits"),
            seam: a.iter().any(|x| x == "--seam"),
            hf_spacing: num("--hf", 0.5),
        }
    }
}

/// Apply an edit and append it to the record log; a record that changed nothing is still logged
/// (it is what a client would have sent), and counted as a no-op.
fn rec(vx: &mut Voxels, body: &Body, log: &mut Vec<Edit>, e: Edit) -> Vec<[i32; 3]> {
    log.push(e);
    vx.apply(body, &e)
}

fn pct(v: &[f64], q: f64) -> f64 {
    if v.is_empty() {
        return f64::NAN;
    }
    v[((v.len() - 1) as f64 * q) as usize]
}

struct Rng(u64);

impl Rng {
    fn next(&mut self) -> f64 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        (self.0 >> 11) as f64 / (1u64 << 53) as f64
    }
    fn range(&mut self, lo: f64, hi: f64) -> f64 {
        lo + (hi - lo) * self.next()
    }
}

fn main() {
    let a = Args::parse();
    let body = Body::bake_with_radius(Seed(3), 24_000.0, 16, 4, 4, 4, 256);
    let site = Site::pick(&body, a.voxel, a.radius);
    let half = site.half();
    let boxm = site.box_metres();
    let t = Instant::now();
    let (mut vx, build) = Voxels::build(&body, site);
    let build_wall = t.elapsed().as_secs_f64() * 1000.0;
    println!(
        "{{ \"test\": \"voxel\", \"radius_arg\": {:.0}, \"voxel\": {}, \"half\": {:.1}, \"box_m\": [{:.0}, {:.0}, {:.0}], \
         \"chunks\": [{}, {}, {}], \"bricks\": {}, \"samples\": {}, \"ram_mb\": {:.1}, \"build_ms\": {:.0}, \"build_wall_ms\": {:.0}, \
         \"z0\": {:.1}, \"z1\": {:.1} }}",
        a.radius,
        a.voxel,
        half,
        boxm[0],
        boxm[1],
        boxm[2],
        site.ncx,
        site.ncy,
        site.ncz,
        build.bricks,
        build.samples,
        build.bytes as f64 / 1e6,
        build.ms,
        build_wall,
        site.z0,
        site.z0 + site.ncz as f64 * 32.0 * site.voxel
    );

    // ---- meshing ----
    let t = Instant::now();
    let mut times = Vec::new();
    let mut m = mesh::Mesh::default();
    let mut keys: Vec<[i32; 3]> = vx.bricks.keys().copied().collect();
    keys.sort_unstable();
    for c in &keys {
        let s = Instant::now();
        let cm = mesh::chunk_surface_nets(&vx, &body, *c);
        times.push(s.elapsed().as_secs_f64() * 1000.0);
        m.append(&cm);
    }
    let mesh_ms = t.elapsed().as_secs_f64() * 1000.0;
    times.sort_by(f64::total_cmp);
    println!(
        "{{ \"test\": \"mesh\", \"extractor\": \"surface-nets\", \"chunks\": {}, \"verts\": {}, \"tris\": {}, \"mesh_mb\": {:.1}, \
         \"mesh_total_ms\": {:.1}, \"chunk_ms\": {{ \"p50\": {:.3}, \"p99\": {:.3}, \"max\": {:.3} }} }}",
        keys.len(),
        m.verts.len(),
        m.tris.len(),
        m.bytes() as f64 / 1e6,
        mesh_ms,
        pct(&times, 0.5),
        pct(&times, 0.99),
        times.last().copied().unwrap_or(f64::NAN)
    );

    if a.blocky {
        let t = Instant::now();
        let b = mesh::blocky(&vx, &body);
        println!(
            "{{ \"test\": \"mesh\", \"extractor\": \"blocky\", \"verts\": {}, \"tris\": {}, \"mesh_mb\": {:.1}, \"mesh_total_ms\": {:.1} }}",
            b.verts.len(),
            b.tris.len(),
            b.bytes() as f64 / 1e6,
            t.elapsed().as_secs_f64() * 1000.0
        );
    }

    if a.seam {
        let s = mesh::seam(&vx, &body, &m, a.voxel.max(0.5), a.hf_spacing);
        println!(
            "{{ \"test\": \"seam\", \"when\": \"base\", \"spacing\": {}, \"hf_spacing\": {}, \"points\": {}, \"hits\": {}, \"holes\": {}, \
             \"step_m\": {{ \"p50\": {:.4}, \"p99\": {:.4}, \"max\": {:.4} }}, \
             \"edge_step_m\": {{ \"p50\": {:.4}, \"p99\": {:.4}, \"max\": {:.4} }}, \
             \"vox_err_m\": {{ \"p50\": {:.4}, \"p99\": {:.4}, \"max\": {:.4} }}, \
             \"worst\": {{ \"x\": {:.1}, \"y\": {:.1}, \"d\": {:.4}, \"vox\": {:.2}, \"hf\": {:.2}, \"analytic\": {:.2}, \"tris\": {} }} }}",
            a.voxel.max(0.5),
            a.hf_spacing,
            s.points,
            s.hits,
            s.holes,
            s.step_p50,
            s.step_p99,
            s.step_max,
            s.edge_p50,
            s.edge_p99,
            s.edge_max,
            s.err_p50,
            s.err_p99,
            s.err_max,
            s.worst[0],
            s.worst[1],
            s.worst[2],
            s.worst_z[0],
            s.worst_z[1],
            s.worst_z[2],
            s.worst_tris
        );

        // Diagnose the worst seam point: is the chunk that holds the true surface resident, and
        // how steep is the field around it?
        let (wx, wy) = (s.worst[0], s.worst[1]);
        let ca = site.chunk_of(wx, wy, s.worst_z[2]);
        let cv = site.chunk_of(wx, wy, s.worst_z[0]);
        let mut rng = (f64::INFINITY, f64::NEG_INFINITY);
        for j in 0..=8 {
            for i in 0..=8 {
                let x = wx - 1.0 + i as f64 * 0.25;
                let y = wy - 1.0 + j as f64 * 0.25;
                let z = site.ground_z(&body, x, y);
                rng = (rng.0.min(z), rng.1.max(z));
            }
        }
        println!(
            "{{ \"test\": \"probe\", \"x\": {:.1}, \"y\": {:.1}, \"chunk_analytic\": [{}, {}, {}], \
             \"has_analytic\": {}, \"has_vox\": {}, \"relief_2m\": {:.2} }}",
            wx,
            wy,
            ca[0],
            ca[1],
            ca[2],
            vx.bricks.contains_key(&ca),
            vx.bricks.contains_key(&cv),
            rng.1 - rng.0
        );
        let mut prof = Vec::new();
        for i in -10..=10 {
            let z = site.ground_z(&body, wx + i as f64 * 0.1, wy);
            prof.push(format!("{z:.1}"));
        }
        println!(
            "{{ \"test\": \"profile\", \"axis\": \"x\", \"from\": -1.0, \"step\": 0.1, \"z\": [{}] }}",
            prof.join(", ")
        );
    }

    if !a.edits {
        return;
    }

    // ---- edits ----
    let surface = site.ground_z(&body, 0.0, 0.0);
    let t = Instant::now();
    let mut log: Vec<Edit> = Vec::new();
    let d = rec(
        &mut vx,
        &body,
        &mut log,
        Edit::Flatten {
            c: [0.0, 0.0, surface],
            r: 8.0,
            z: surface - 1.0,
        },
    );
    let flatten_ms = t.elapsed().as_secs_f64() * 1000.0;
    let mut dirty: HashSet<[i32; 3]> = d.into_iter().collect();

    let t = Instant::now();
    for i in 0..=16 {
        let x = -8.0 + i as f64;
        let z = site.ground_z(&body, x, 0.0) - 4.0;
        dirty.extend(rec(
            &mut vx,
            &body,
            &mut log,
            Edit::Dig {
                c: [x, 0.0, z],
                r: 1.5,
            },
        ));
    }
    let tunnel_ms = t.elapsed().as_secs_f64() * 1000.0;

    let t = Instant::now();
    let carve = site.ground_z(&body, 24.0, 6.0) - 3.0;
    dirty.extend(rec(
        &mut vx,
        &body,
        &mut log,
        Edit::Dig {
            c: [24.0, 6.0, carve],
            r: 6.0,
        },
    ));
    let carve_ms = t.elapsed().as_secs_f64() * 1000.0;

    // 500 small digs, deterministic, on the surface.
    let mut rng = Rng(0x9e37_79b9_7f4a_7c15);
    let t = Instant::now();
    let mut dig_dirty = HashSet::new();
    for _ in 0..500 {
        let x = rng.range(-half * 0.85, half * 0.85);
        let y = rng.range(-half * 0.85, half * 0.85);
        let z = site.ground_z(&body, x, y) - rng.range(0.0, 1.5);
        dig_dirty.extend(rec(
            &mut vx,
            &body,
            &mut log,
            Edit::Dig {
                c: [x, y, z],
                r: 1.0,
            },
        ));
    }
    let digs_ms = t.elapsed().as_secs_f64() * 1000.0;

    // Remesh everything the scripted edits touched, as a program would on one frame's budget.
    let t = Instant::now();
    let mut touch: Vec<[i32; 3]> = dirty.iter().copied().collect();
    touch.sort_unstable();
    for c in &touch {
        if vx.bricks.contains_key(c) {
            let _ = mesh::chunk_surface_nets(&vx, &body, *c);
        }
    }
    let remesh_ms = t.elapsed().as_secs_f64() * 1000.0;

    // Largest brush whose dirty-chunk remesh fits one 60 Hz frame.
    let mut budget_radius = 0.0;
    let mut budget_ms = 0.0;
    for r in [1.0f64, 2.0, 3.0, 4.0, 6.0, 8.0] {
        let x = 12.0;
        let y = -10.0;
        let z = site.ground_z(&body, x, y) - 2.0;
        // Timed from the edit, not the remesh: a dig below the band allocates bricks first.
        let t = Instant::now();
        let changed = rec(&mut vx, &body, &mut log, Edit::Dig { c: [x, y, z], r });
        for c in &changed {
            let _ = mesh::chunk_surface_nets(&vx, &body, *c);
        }
        let ms = t.elapsed().as_secs_f64() * 1000.0;
        if ms <= 16.7 {
            budget_radius = r;
            budget_ms = ms;
        }
    }

    // Final mesh after edits.
    let t = Instant::now();
    let mut after = mesh::Mesh::default();
    let mut after_times = Vec::new();
    // Edits may have allocated bricks the build did not (a dig below the band).
    let mut keys: Vec<[i32; 3]> = vx.bricks.keys().copied().collect();
    keys.sort_unstable();
    for c in &keys {
        let s = Instant::now();
        after.append(&mesh::chunk_surface_nets(&vx, &body, *c));
        after_times.push(s.elapsed().as_secs_f64() * 1000.0);
    }
    let after_ms = t.elapsed().as_secs_f64() * 1000.0;
    after_times.sort_by(f64::total_cmp);

    println!(
        "{{ \"test\": \"edits\", \"flatten_ms\": {:.2}, \"tunnel_ms\": {:.2}, \"carve_ms\": {:.2}, \"digs500_ms\": {:.2}, \
         \"dirty_chunks\": {}, \"allocated_by_edits\": {}, \"remesh_ms\": {:.2}, \"frame_budget_ms\": 16.7, \
         \"budget_radius_m\": {:.1}, \"budget_radius_ms\": {:.2}, \
         \"after_tris\": {}, \"after_mesh_ms\": {:.1}, \"after_chunk_ms\": {{ \"p50\": {:.3}, \"p99\": {:.3} }} }}",
        flatten_ms,
        tunnel_ms,
        carve_ms,
        digs_ms,
        touch.len(),
        vx.allocated,
        remesh_ms,
        budget_radius,
        budget_ms,
        after.tris.len(),
        after_ms,
        pct(&after_times, 0.5),
        pct(&after_times, 0.99)
    );
    let _ = dig_dirty;

    if a.seam {
        let s = mesh::seam(&vx, &body, &after, a.voxel.max(0.5), a.hf_spacing);
        println!(
            "{{ \"test\": \"seam\", \"when\": \"edited\", \"points\": {}, \"hits\": {}, \"holes\": {}, \"hole_box\": [{:.2}, {:.2}, {:.2}, {:.2}], \
             \"step_m\": {{ \"p50\": {:.4}, \"p99\": {:.4}, \"max\": {:.4} }}, \
             \"edge_step_m\": {{ \"p50\": {:.4}, \"p99\": {:.4}, \"max\": {:.4} }} }}",
            s.points,
            s.hits,
            s.holes,
            s.hole_box[0],
            s.hole_box[1],
            s.hole_box[2],
            s.hole_box[3],
            s.step_p50,
            s.step_p99,
            s.step_max,
            s.edge_p50,
            s.edge_p99,
            s.edge_max
        );
    }

    // ---- records: replay, storage, no-ops ----
    // Edits must be a function of the record set (docs/18): rebuilding the base and replaying
    // the log in order has to reproduce the live-edited region bit for bit.
    let (base, _) = Voxels::build(&body, site);
    let mut replay = Voxels::build(&body, site).0;
    let t = Instant::now();
    let mut noops = 0usize;
    for e in &log {
        if replay.apply(&body, e).is_empty() {
            noops += 1;
        }
    }
    let replay_ms = t.elapsed().as_secs_f64() * 1000.0;
    let identical = replay.same_as(&vx);
    let changed = vx.changed_samples(&base, &body);
    // A changed-voxel delta: u16 index within the brick (32³ = 2¹⁵ fits) + f16 value, plus a
    // 12-byte chunk key per brick that has any change.
    let delta_bricks = vx
        .bricks
        .iter()
        .filter(|(c, b)| match base.bricks.get(*c) {
            Some(o) => b.s != o.s,
            None => true,
        })
        .count();
    let delta_bytes = changed * 4 + delta_bricks * 12;
    let record_bytes = log.len() * Edit::RECORD_BYTES;
    println!(
        "{{ \"test\": \"records\", \"records\": {}, \"noops\": {}, \"record_bytes\": {}, \"bytes_per_1000\": {}, \
         \"replay_ms\": {:.1}, \"replay_identical\": {}, \"changed_samples\": {}, \"delta_bricks\": {}, \"delta_bytes\": {} }}",
        log.len(),
        noops,
        record_bytes,
        Edit::RECORD_BYTES * 1000,
        replay_ms,
        identical,
        changed,
        delta_bricks,
        delta_bytes
    );
}
