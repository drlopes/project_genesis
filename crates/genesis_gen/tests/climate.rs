//! P1-C1 physical phases: energy budget, water state, climate summary, biosphere, and the docs/05
//! climate invariants (continuity, range) on the field the summary is computed from.

use genesis_core::math::f64::{cos, sin};
use genesis_gen::bake::relief;
use genesis_gen::climate::{ClimateField, ClimateInputs};
use genesis_gen::cube::{cross, normalize};
use genesis_gen::descriptor::{BiosphereLevel, WaterState};
use genesis_gen::physical::{FREEZING, boiling_point, water_distance};
use genesis_gen::{RockyParams, SeedChain, bake_caches, bake_core};

const SEEDS: std::ops::Range<u64> = 0..40;

#[test]
fn boiling_point_follows_pressure() {
    assert!((boiling_point(101_325.0) - 373.15).abs() < 1e-9);
    assert!(boiling_point(30_000.0) < 350.0);
    assert!(boiling_point(500_000.0) > 410.0);
}

#[test]
fn physical_blocks_are_consistent() {
    let p = RockyParams::default();
    let mut states = std::collections::BTreeMap::new();
    for s in SEEDS {
        let c = bake_core(SeedChain::body(s, 0), &p).unwrap();
        let (e, a, h) = (c.energy.unwrap(), c.atmosphere.unwrap(), &c.hydrosphere);
        let cl = c.climate.unwrap();
        let b = c.biosphere.unwrap();
        *states.entry(format!("{:?}", h.state)).or_insert(0) += 1;

        assert!((0.0..1.0).contains(&e.albedo));
        assert_eq!(a.present, a.surface_pressure > 0.0);
        if !a.present {
            assert_eq!(
                e.greenhouse, -33.0,
                "seed {s}: no atmosphere is −33 K (docs/05)"
            );
        }
        // Water state rules.
        match h.state {
            WaterState::None => assert!(h.sea_radius.is_none() && h.ocean_fraction == 0.0),
            WaterState::Vapour => assert!(
                h.sea_radius.is_none(),
                "seed {s}: boiled-off seas leave no sea"
            ),
            WaterState::Liquid | WaterState::Mixed => {
                assert!(
                    a.surface_pressure >= 611.657
                        && cl.mean_surface_temp < boiling_point(a.surface_pressure)
                );
            }
            WaterState::Ice => assert!(h.sea_radius.is_some()),
        }
        // Ranges are ordered and physical.
        let r = &cl.ranges;
        for (name, st) in [
            ("temperature", r.temperature),
            ("moisture", r.moisture),
            ("altitude", r.altitude),
            ("slope", r.slope),
        ] {
            assert!(
                st.min <= st.mean && st.mean <= st.max,
                "seed {s}: {name} {st:?}"
            );
        }
        assert!(r.temperature.min > 0.0);
        assert!(r.moisture.min >= 0.0 && r.moisture.max <= 1.0);
        if h.sea_radius.is_some() {
            // With no sea, altitude is measured from the reference sphere and may be negative.
            assert!(
                r.altitude.min >= 0.0,
                "seed {s}: land is at or above the sea"
            );
        }
        assert!((0.0..=1.0).contains(&r.land_fraction));
        // Life only with liquid water in its window.
        if !matches!(h.state, WaterState::Liquid | WaterState::Mixed) {
            assert_eq!(b.level, BiosphereLevel::None);
        }
        assert_eq!(b.level == BiosphereLevel::None, b.density == 0.0);
    }
    eprintln!("water states over {} seeds: {states:?}", SEEDS.end);
    assert!(
        states.len() >= 3,
        "the stand-in draws should give several water states: {states:?}"
    );
}

#[test]
fn climate_fields_are_continuous_and_in_range() {
    // docs/05 invariants 1 and 2, on the field the bake summarises: walk great circles in 50 m
    // steps; temperature and moisture must change smoothly (here: by less than 1 K and 0.02 per
    // step, against tens of K and the full [0, 1] across a body).
    let p = RockyParams::default();
    for s in [1337u64, 99] {
        let core = bake_core(SeedChain::body(s, 0), &p).unwrap();
        let caches = bake_caches(&core, &p);
        let r = relief(&core, &p);
        let cl = core.climate.as_ref().unwrap();
        let field = ClimateField {
            inputs: ClimateInputs {
                radius: core.bulk.radius,
                sea_level: core.hydrosphere.sea_radius.map(|x| x - core.bulk.radius),
                t_base: cl.mean_surface_temp,
                a_lat: cl.latitude_amplitude,
                lapse_rate: cl.lapse_rate,
                climate_seed: core.seed_chain.body_seed().derive("climate"),
            },
            params: &p.climate,
        };
        let spacing = 2.0 / p.relief_grid as f64 * core.bulk.radius;
        let at = |d| {
            let h = r.h0(d, spacing);
            let up = r.h0(field.upwind(d, p.climate.wind_distance), spacing);
            let dw = caches
                .water_distance
                .as_ref()
                .map_or(f64::INFINITY, |g| f64::from(g.sample(d)));
            let t = field.temperature(d, h, up, dw);
            (t, field.moisture(d, h, up, dw, t))
        };
        // Largest change per step along three great circles (through both poles, and tilted).
        let walk = |metres: f64| {
            let step = metres / core.bulk.radius;
            let (mut dt, mut dm) = (0.0f64, 0.0f64);
            for axis in [[1.0, 0.0, 0.0], [0.0, 0.0, 1.0], normalize([0.3, 0.5, 0.8])] {
                let a = normalize(cross(axis, [0.0, 1.0, 0.0]));
                let b = cross(axis, a);
                let ring = |t: f64| {
                    normalize([
                        a[0] * cos(t) + b[0] * sin(t),
                        a[1] * cos(t) + b[1] * sin(t),
                        a[2] * cos(t) + b[2] * sin(t),
                    ])
                };
                let n = (std::f64::consts::TAU / step) as usize;
                let mut prev = at(ring(0.0));
                for i in 1..=n {
                    let cur = at(ring(i as f64 * step));
                    assert!(cur.0.is_finite() && cur.0 > 0.0, "temperature {}", cur.0);
                    assert!((0.0..=1.0).contains(&cur.1), "moisture {}", cur.1);
                    dt = dt.max((cur.0 - prev.0).abs());
                    dm = dm.max((cur.1 - prev.1).abs());
                    prev = cur;
                }
            }
            (dt, dm)
        };
        // A continuous field's largest change shrinks with the step (to ~1/4 at a quarter step);
        // a jump keeps its size at any step. Steep terrain legitimately gives large changes, so
        // the size alone cannot tell them apart.
        let (t50, m50) = walk(50.0);
        let (t12, m12) = walk(12.5);
        eprintln!(
            "seed {s}: largest change per 50 m {t50:.3} K / {m50:.4}; per 12.5 m {t12:.3} K / {m12:.4}"
        );
        assert!(
            t12 < 0.5 * t50 && m12 < 0.5 * m50,
            "seed {s}: a change does not shrink with the step"
        );
    }
}

#[test]
fn water_distance_is_zero_at_sea_and_grows_inland() {
    let p = RockyParams::default();
    let core = bake_core(SeedChain::body(1337, 0), &p).unwrap();
    let r = relief(&core, &p);
    let sea = core.hydrosphere.sea_radius.map(|x| x - core.bulk.radius);
    let g = water_distance(&r, sea, 32).expect("this body has a sea");
    let max = g.data.iter().copied().fold(0.0f32, f32::max);
    assert!(g.data.contains(&0.0) && max > 1_000.0, "max {max} m");
    assert!(water_distance(&r, None, 32).is_none());
}

#[test]
fn frozen_seas_mean_cold_bodies() {
    let p = RockyParams::default();
    for s in SEEDS {
        let c = bake_core(SeedChain::body(s, 0), &p).unwrap();
        let cl = c.climate.unwrap();
        if c.hydrosphere.state == WaterState::Ice
            && c.atmosphere.unwrap().surface_pressure >= 611.657
        {
            assert!(cl.frozen_sea_fraction > 0.9);
            assert!(cl.mean_surface_temp < FREEZING + cl.latitude_amplitude);
        }
    }
}
