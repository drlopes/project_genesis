//! Platform-independent maths for generation code.
//!
//! `std`'s transcendental functions (`f32::sin`, `f64::exp`, …) call the platform's maths
//! library, whose last bits can differ between Windows and Linux. Generation code must use
//! these wrappers instead; they call the pure-Rust `libm` crate, which gives the same bits
//! everywhere. Clippy's `disallowed_methods` (see `clippy.toml`) enforces this.
//!
//! `+ − × ÷` and `sqrt` are IEEE-754 correctly rounded on every platform, so `std` versions
//! of those are fine and are not wrapped here.
//!
//! See `docs/20-tech-stack-and-workspace.md#determinism-rules`, rule 1.

/// Single-precision functions, for surface-local fields.
pub mod f32 {
    #[inline]
    pub fn sin(x: f32) -> f32 {
        libm::sinf(x)
    }
    #[inline]
    pub fn cos(x: f32) -> f32 {
        libm::cosf(x)
    }
    #[inline]
    pub fn tan(x: f32) -> f32 {
        libm::tanf(x)
    }
    #[inline]
    pub fn asin(x: f32) -> f32 {
        libm::asinf(x)
    }
    #[inline]
    pub fn acos(x: f32) -> f32 {
        libm::acosf(x)
    }
    #[inline]
    pub fn atan(x: f32) -> f32 {
        libm::atanf(x)
    }
    #[inline]
    pub fn atan2(y: f32, x: f32) -> f32 {
        libm::atan2f(y, x)
    }
    #[inline]
    pub fn exp(x: f32) -> f32 {
        libm::expf(x)
    }
    #[inline]
    pub fn ln(x: f32) -> f32 {
        libm::logf(x)
    }
    #[inline]
    pub fn powf(x: f32, y: f32) -> f32 {
        libm::powf(x, y)
    }
    #[inline]
    pub fn tanh(x: f32) -> f32 {
        libm::tanhf(x)
    }

    /// Hermite smoothstep: 0 at `e0`, 1 at `e1`, C¹-continuous. Used for every soft edge in
    /// classification (docs/19-biome-classification.md).
    #[inline]
    pub fn smoothstep(e0: f32, e1: f32, x: f32) -> f32 {
        if e0 == e1 {
            return if x < e0 { 0.0 } else { 1.0 };
        }
        let t = ((x - e0) / (e1 - e0)).clamp(0.0, 1.0);
        t * t * (3.0 - 2.0 * t)
    }

    #[inline]
    pub fn lerp(a: f32, b: f32, t: f32) -> f32 {
        a + (b - a) * t
    }
}

/// Double-precision functions, for system- and galaxy-scale positions.
pub mod f64 {
    #[inline]
    pub fn sin(x: f64) -> f64 {
        libm::sin(x)
    }
    #[inline]
    pub fn cos(x: f64) -> f64 {
        libm::cos(x)
    }
    #[inline]
    pub fn tan(x: f64) -> f64 {
        libm::tan(x)
    }
    #[inline]
    pub fn asin(x: f64) -> f64 {
        libm::asin(x)
    }
    #[inline]
    pub fn acos(x: f64) -> f64 {
        libm::acos(x)
    }
    #[inline]
    pub fn atan(x: f64) -> f64 {
        libm::atan(x)
    }
    #[inline]
    pub fn atan2(y: f64, x: f64) -> f64 {
        libm::atan2(y, x)
    }
    #[inline]
    pub fn exp(x: f64) -> f64 {
        libm::exp(x)
    }
    #[inline]
    pub fn ln(x: f64) -> f64 {
        libm::log(x)
    }
    #[inline]
    pub fn powf(x: f64, y: f64) -> f64 {
        libm::pow(x, y)
    }
    #[inline]
    pub fn tanh(x: f64) -> f64 {
        libm::tanh(x)
    }
}

#[cfg(test)]
mod tests {
    use super::f32::*;

    #[test]
    fn smoothstep_edges_and_midpoint() {
        assert_eq!(smoothstep(0.0, 1.0, -1.0), 0.0);
        assert_eq!(smoothstep(0.0, 1.0, 2.0), 1.0);
        assert_eq!(smoothstep(0.0, 1.0, 0.5), 0.5);
        // Degenerate interval is a step, not a NaN.
        assert_eq!(smoothstep(1.0, 1.0, 0.5), 0.0);
        assert_eq!(smoothstep(1.0, 1.0, 1.5), 1.0);
    }

    #[test]
    fn wrappers_are_sane() {
        assert!((sin(0.5) - 0.479_425_55).abs() < 1e-6);
        assert!((exp(1.0) - core::f32::consts::E).abs() < 1e-6);
        assert!((powf(2.0, 10.0) - 1024.0).abs() < 1e-3);
    }
}
