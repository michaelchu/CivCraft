//! OpenSimplex 3D noise used by OpenCiv3's cylindrical Civ III map field.
//!
//! The 3D contribution/lookup implementation and gradients follow
//! `../../OpenCiv3/C7Engine/C7GameData/MapGenNoise.cs`, whose source identifies
//! the underlying public-domain OpenSimplex implementation. This reimplementation
//! is limited to its 3D evaluator; it does not include OpenCiv3's 2D/4D variants.

const STRETCH_3D: f64 = -1.0 / 6.0;
const SQUISH_3D: f64 = 1.0 / 3.0;
const NORM_3D: f64 = 1.0 / 103.0;
const GRADIENTS_3D: [[i32; 3]; 24] = [
    [-11, 4, 4],
    [-4, 11, 4],
    [-4, 4, 11],
    [11, 4, 4],
    [4, 11, 4],
    [4, 4, 11],
    [-11, -4, 4],
    [-4, -11, 4],
    [-4, -4, 11],
    [11, -4, 4],
    [4, -11, 4],
    [4, -4, 11],
    [-11, 4, -4],
    [-4, 11, -4],
    [-4, 4, -11],
    [11, 4, -4],
    [4, 11, -4],
    [4, 4, -11],
    [-11, -4, -4],
    [-4, -11, -4],
    [-4, -4, -11],
    [11, -4, -4],
    [4, -11, -4],
    [4, -4, -11],
];
const BASE_3D: [&[i32]; 3] = [
    &[0, 0, 0, 0, 1, 1, 0, 0, 1, 0, 1, 0, 1, 0, 0, 1],
    &[2, 1, 1, 0, 2, 1, 0, 1, 2, 0, 1, 1, 3, 1, 1, 1],
    &[
        1, 1, 0, 0, 1, 0, 1, 0, 1, 0, 0, 1, 2, 1, 1, 0, 2, 1, 0, 1, 2, 0, 1, 1,
    ],
];
const P3D: [i32; 216] = [
    0, 0, 1, -1, 0, 0, 1, 0, -1, 0, 0, -1, 1, 0, 0, 0, 1, -1, 0, 0, -1, 0, 1, 0, 0, -1, 1, 0, 2, 1,
    1, 0, 1, 1, 1, -1, 0, 2, 1, 0, 1, 1, 1, -1, 1, 0, 2, 0, 1, 1, 1, -1, 1, 1, 1, 3, 2, 1, 0, 3, 1,
    2, 0, 1, 3, 2, 0, 1, 3, 1, 0, 2, 1, 3, 0, 2, 1, 3, 0, 1, 2, 1, 1, 1, 0, 0, 2, 2, 0, 0, 1, 1, 0,
    1, 0, 2, 0, 2, 0, 1, 1, 0, 0, 1, 2, 0, 0, 2, 2, 0, 0, 0, 0, 1, 1, -1, 1, 2, 0, 0, 0, 0, 1, -1,
    1, 1, 2, 0, 0, 0, 0, 1, 1, 1, -1, 2, 3, 1, 1, 1, 2, 0, 0, 2, 2, 3, 1, 1, 1, 2, 2, 0, 0, 2, 3,
    1, 1, 1, 2, 0, 2, 0, 2, 1, 1, -1, 1, 2, 0, 0, 2, 2, 1, 1, -1, 1, 2, 2, 0, 0, 2, 1, -1, 1, 1, 2,
    0, 0, 2, 2, 1, -1, 1, 1, 2, 0, 2, 0, 2, 1, 1, 1, -1, 2, 2, 0, 0, 2, 1, 1, 1, -1, 2, 0, 2, 0,
];
const LOOKUP_PAIRS_3D: [(usize, usize); 72] = [
    (0, 2),
    (1, 1),
    (2, 2),
    (5, 1),
    (6, 0),
    (7, 0),
    (32, 2),
    (34, 2),
    (129, 1),
    (133, 1),
    (160, 5),
    (161, 5),
    (518, 0),
    (519, 0),
    (546, 4),
    (550, 4),
    (645, 3),
    (647, 3),
    (672, 5),
    (673, 5),
    (674, 4),
    (677, 3),
    (678, 4),
    (679, 3),
    (680, 13),
    (681, 13),
    (682, 12),
    (685, 14),
    (686, 12),
    (687, 14),
    (712, 20),
    (714, 18),
    (809, 21),
    (813, 23),
    (840, 20),
    (841, 21),
    (1198, 19),
    (1199, 22),
    (1226, 18),
    (1230, 19),
    (1325, 23),
    (1327, 22),
    (1352, 15),
    (1353, 17),
    (1354, 15),
    (1357, 17),
    (1358, 16),
    (1359, 16),
    (1360, 11),
    (1361, 10),
    (1362, 11),
    (1365, 10),
    (1366, 9),
    (1367, 9),
    (1392, 11),
    (1394, 11),
    (1489, 10),
    (1493, 10),
    (1520, 8),
    (1521, 8),
    (1878, 9),
    (1879, 9),
    (1906, 7),
    (1910, 7),
    (2005, 6),
    (2007, 6),
    (2032, 8),
    (2033, 8),
    (2034, 7),
    (2037, 6),
    (2038, 7),
    (2039, 6),
];

#[derive(Clone, Copy)]
struct Contribution {
    dx: f64,
    dy: f64,
    dz: f64,
    xsb: i32,
    ysb: i32,
    zsb: i32,
}
impl Contribution {
    fn new(multiplier: i32, xsb: i32, ysb: i32, zsb: i32) -> Self {
        Self {
            dx: -f64::from(xsb) - f64::from(multiplier) * SQUISH_3D,
            dy: -f64::from(ysb) - f64::from(multiplier) * SQUISH_3D,
            dz: -f64::from(zsb) - f64::from(multiplier) * SQUISH_3D,
            xsb,
            ysb,
            zsb,
        }
    }
}

pub(super) struct OpenSimplex3 {
    perm: [u8; 256],
    perm3d: [u8; 256],
    lookup: [u8; 2048],
    patterns: Vec<Vec<Contribution>>,
}
impl OpenSimplex3 {
    pub(super) fn new(seed: u64) -> Self {
        let mut source = [0u8; 256];
        for (i, value) in source.iter_mut().enumerate() {
            *value = i as u8;
        }
        // OpenCiv3 passes signed 32-bit map seeds to this generator.
        let mut seed = seed as i32 as i64;
        for _ in 0..3 {
            seed = seed
                .wrapping_mul(6_364_136_223_846_793_005)
                .wrapping_add(1_442_695_040_888_963_407);
        }
        let mut perm = [0u8; 256];
        let mut perm3d = [0u8; 256];
        for i in (0..256).rev() {
            seed = seed
                .wrapping_mul(6_364_136_223_846_793_005)
                .wrapping_add(1_442_695_040_888_963_407);
            let r = seed.wrapping_add(31).rem_euclid((i + 1) as i64) as usize;
            perm[i] = source[r];
            perm3d[i] = (usize::from(perm[i]) % 24 * 3) as u8;
            source[r] = source[i];
        }
        let mut patterns = Vec::with_capacity(P3D.len() / 9);
        for i in (0..P3D.len()).step_by(9) {
            let base = BASE_3D[P3D[i] as usize];
            let mut pattern = Vec::with_capacity(base.len() / 4 + 3);
            for c in base.chunks_exact(4) {
                pattern.push(Contribution::new(c[0], c[1], c[2], c[3]));
            }
            for c in P3D[i + 1..i + 9].chunks_exact(4) {
                pattern.push(Contribution::new(c[0], c[1], c[2], c[3]));
            }
            patterns.push(pattern);
        }
        let mut lookup = [0u8; 2048];
        for &(hash, pattern) in &LOOKUP_PAIRS_3D {
            lookup[hash] = pattern as u8;
        }
        Self {
            perm,
            perm3d,
            lookup,
            patterns,
        }
    }
    pub(super) fn sample(&self, x: f64, y: f64, z: f64) -> f64 {
        let stretch = (x + y + z) * STRETCH_3D;
        let xs = x + stretch;
        let ys = y + stretch;
        let zs = z + stretch;
        let xsb = xs.floor() as i32;
        let ysb = ys.floor() as i32;
        let zsb = zs.floor() as i32;
        let squish = f64::from(xsb + ysb + zsb) * SQUISH_3D;
        let dx0 = x - (f64::from(xsb) + squish);
        let dy0 = y - (f64::from(ysb) + squish);
        let dz0 = z - (f64::from(zsb) + squish);
        let xins = xs - f64::from(xsb);
        let yins = ys - f64::from(ysb);
        let zins = zs - f64::from(zsb);
        let sum = xins + yins + zins;
        let hash = ((yins - zins + 1.0) as usize)
            | (((xins - yins + 1.0) as usize) << 1)
            | (((xins - zins + 1.0) as usize) << 2)
            | ((sum as usize) << 3)
            | (((sum + zins) as usize) << 5)
            | (((sum + yins) as usize) << 7)
            | (((sum + xins) as usize) << 9);
        let mut value = 0.0;
        for c in &self.patterns[self.lookup[hash] as usize] {
            let dx = dx0 + c.dx;
            let dy = dy0 + c.dy;
            let dz = dz0 + c.dz;
            let mut attn = 2.0 - dx * dx - dy * dy - dz * dz;
            if attn <= 0.0 {
                continue;
            }
            let px = (xsb + c.xsb) as u8;
            let py = (ysb + c.ysb) as u8;
            let pz = (zsb + c.zsb) as u8;
            let p = self.perm[px as usize].wrapping_add(py);
            let p = self.perm[p as usize].wrapping_add(pz);
            let gradient = self.perm3d[p as usize] as usize / 3;
            let g = GRADIENTS_3D[gradient];
            attn *= attn;
            value +=
                attn * attn * (f64::from(g[0]) * dx + f64::from(g[1]) * dy + f64::from(g[2]) * dz);
        }
        value * NORM_3D
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn seeded_noise_is_repeatable_and_seed_sensitive() {
        let a = OpenSimplex3::new(42);
        let b = OpenSimplex3::new(42);
        let c = OpenSimplex3::new(43);
        let p = (1003.25, 999.5, 1007.75);
        assert_eq!(a.sample(p.0, p.1, p.2), b.sample(p.0, p.1, p.2));
        assert_ne!(a.sample(p.0, p.1, p.2), c.sample(p.0, p.1, p.2));
    }
    #[test]
    fn samples_are_finite_and_normalized() {
        let noise = OpenSimplex3::new(42);
        for x in -16..=16 {
            for y in -16..=16 {
                for z in -16..=16 {
                    let v = noise.sample(x as f64 * 0.37, y as f64 * 0.29, z as f64 * 0.41);
                    assert!(v.is_finite());
                    assert!((-1.0..=1.0).contains(&v), "noise sample {v}");
                }
            }
        }
    }
}
