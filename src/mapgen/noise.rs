//! Seeded, tileable fractal noise for the Civ III-style terrain generator.
//!
//! OpenCiv3 uses OpenSimplex in a cylindrical 3D embedding for horizontally
//! wrapped maps. This module uses classic 3D simplex noise with the same
//! embedding and octave schedule. It is an explicit algorithmic approximation
//! that replaces the previous subdivision diamond field.

const GRADIENTS: [[f64; 3]; 12] = [
    [1.0, 1.0, 0.0],
    [-1.0, 1.0, 0.0],
    [1.0, -1.0, 0.0],
    [-1.0, -1.0, 0.0],
    [1.0, 0.0, 1.0],
    [-1.0, 0.0, 1.0],
    [1.0, 0.0, -1.0],
    [-1.0, 0.0, -1.0],
    [0.0, 1.0, 1.0],
    [0.0, -1.0, 1.0],
    [0.0, 1.0, -1.0],
    [0.0, -1.0, -1.0],
];

pub(super) struct Simplex3 {
    permutation: [u8; 512],
}

impl Simplex3 {
    pub(super) fn new(seed: u64) -> Self {
        let mut source: Vec<u8> = (0..=255).collect();
        let mut state = seed;
        for i in (1..source.len()).rev() {
            state = state.wrapping_add(0x9e37_79b9_7f4a_7c15);
            let mut value = state;
            value = (value ^ (value >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
            value = (value ^ (value >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
            value ^= value >> 31;
            source.swap(i, (value as usize) % (i + 1));
        }
        let mut permutation = [0; 512];
        for i in 0..512 {
            permutation[i] = source[i & 255];
        }
        Self { permutation }
    }

    pub(super) fn sample(&self, x: f64, y: f64, z: f64) -> f64 {
        const F3: f64 = 1.0 / 3.0;
        const G3: f64 = 1.0 / 6.0;
        let skew = (x + y + z) * F3;
        let i = (x + skew).floor() as i32;
        let j = (y + skew).floor() as i32;
        let k = (z + skew).floor() as i32;
        let unskew = f64::from(i + j + k) * G3;
        let x0 = x - (f64::from(i) - unskew);
        let y0 = y - (f64::from(j) - unskew);
        let z0 = z - (f64::from(k) - unskew);

        let (i1, j1, k1, i2, j2, k2) = if x0 >= y0 {
            if y0 >= z0 {
                (1, 0, 0, 1, 1, 0)
            } else if x0 >= z0 {
                (1, 0, 0, 1, 0, 1)
            } else {
                (0, 0, 1, 1, 0, 1)
            }
        } else if y0 < z0 {
            (0, 0, 1, 0, 1, 1)
        } else if x0 < z0 {
            (0, 1, 0, 0, 1, 1)
        } else {
            (0, 1, 0, 1, 1, 0)
        };

        let corners = [
            (0, 0, 0, x0, y0, z0),
            (
                i1,
                j1,
                k1,
                x0 - f64::from(i1) + G3,
                y0 - f64::from(j1) + G3,
                z0 - f64::from(k1) + G3,
            ),
            (
                i2,
                j2,
                k2,
                x0 - f64::from(i2) + 2.0 * G3,
                y0 - f64::from(j2) + 2.0 * G3,
                z0 - f64::from(k2) + 2.0 * G3,
            ),
            (
                1,
                1,
                1,
                x0 - 1.0 + 3.0 * G3,
                y0 - 1.0 + 3.0 * G3,
                z0 - 1.0 + 3.0 * G3,
            ),
        ];
        let mut total = 0.0;
        for (di, dj, dk, dx, dy, dz) in corners {
            let attenuation = 0.6 - dx * dx - dy * dy - dz * dz;
            if attenuation <= 0.0 {
                continue;
            }
            let ii = (i + di).rem_euclid(256) as usize;
            let jj = (j + dj).rem_euclid(256) as usize;
            let kk = (k + dk).rem_euclid(256) as usize;
            let hash = self.permutation
                [ii + self.permutation[jj + self.permutation[kk] as usize] as usize]
                as usize
                % 12;
            let gradient = GRADIENTS[hash];
            let dot = gradient[0] * dx + gradient[1] * dy + gradient[2] * dz;
            let attenuation2 = attenuation * attenuation;
            total += attenuation2 * attenuation2 * dot;
        }
        32.0 * total
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn seeded_noise_is_repeatable_and_seed_sensitive() {
        let first = Simplex3::new(42);
        let repeat = Simplex3::new(42);
        let other = Simplex3::new(43);
        let point = (1003.25, 999.5, 1007.75);
        assert_eq!(
            first.sample(point.0, point.1, point.2),
            repeat.sample(point.0, point.1, point.2)
        );
        assert_ne!(
            first.sample(point.0, point.1, point.2),
            other.sample(point.0, point.1, point.2)
        );
    }
}
