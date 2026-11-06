//! Bounded cache of drawing geometry. Ownership, visibility and colors are read live.
use super::*;
use std::collections::HashMap;
const MAX_GEOMETRY_ENTRIES: usize = 4096;
#[derive(Hash, PartialEq, Eq)]
struct GeometryKey {
    vertices: [u32; 8],
    width: u32,
    edge: usize,
}
impl GeometryKey {
    fn new(points: &[Vec2; 4], edge: usize, width: f32) -> Self {
        Self {
            vertices: std::array::from_fn(|i| {
                if i % 2 == 0 {
                    points[i / 2].x.to_bits()
                } else {
                    points[i / 2].y.to_bits()
                }
            }),
            width: width.to_bits(),
            edge,
        }
    }
}
struct EdgeGeometry {
    outline: Vec<Vec2>,
    band: Vec<Vec2>,
    stitches: Vec<Vec<Vec2>>,
}
impl EdgeGeometry {
    fn new(points: &[Vec2; 4], edge: usize, width: f32) -> Self {
        let (start, end) = (points[edge], points[(edge + 1) % 4]);
        let center = points.iter().copied().sum::<Vec2>() * 0.25;
        let tangent = (end - start).normalize();
        let normal = territory_inward_normal(start, end, center);
        let length = start.distance(end);
        let count = (length / 9.0).floor().max(1.0) as usize;
        let stitches = (0..count)
            .map(|index| {
                let midpoint = start + tangent * (length * (index as f32 + 0.5) / count as f32);
                let a = midpoint + normal * width * 0.22 - tangent * width * 0.18;
                let b = midpoint + normal * width * 0.78 + tangent * width * 0.18;
                let cross = vec2(-(b - a).y, (b - a).x).normalize() * 0.6;
                clip_territory_polygon(vec![a - cross, b - cross, b + cross, a + cross], points)
            })
            .collect();
        Self {
            outline: territory_edge_band(points, edge, width + 1.0),
            band: territory_edge_band(points, edge, width),
            stitches,
        }
    }
}
#[derive(Default)]
pub(super) struct BorderGeometryCache {
    entries: HashMap<GeometryKey, EdgeGeometry>,
    hits: u64,
    misses: u64,
}
impl BorderGeometryCache {
    fn geometry(&mut self, points: &[Vec2; 4], edge: usize, width: f32) -> &EdgeGeometry {
        let key = GeometryKey::new(points, edge, width);
        if self.entries.contains_key(&key) {
            self.hits += 1;
        } else {
            self.misses += 1;
            if self.entries.len() >= MAX_GEOMETRY_ENTRIES {
                self.entries.clear();
            }
            self.entries.insert(
                GeometryKey::new(points, edge, width),
                EdgeGeometry::new(points, edge, width),
            );
        }
        &self.entries[&key]
    }
    pub(super) fn draw(
        &mut self,
        points: &[Vec2; 4],
        edge: usize,
        width: f32,
        color: Color,
        shadow: Color,
        thread: Color,
    ) {
        let geometry = self.geometry(points, edge, width);
        fill_territory_polygon(&geometry.outline, shadow);
        fill_territory_polygon(&geometry.band, color);
        for polygon in &geometry.stitches {
            fill_territory_polygon(polygon, thread);
        }
    }
    pub(super) fn stats(&self) -> (u64, u64, usize) {
        (self.hits, self.misses, self.entries.len())
    }
}

fn territory_inward_normal(start: Vec2, end: Vec2, center: Vec2) -> Vec2 {
    let tangent = (end - start).normalize();
    let normal = vec2(-tangent.y, tangent.x);
    if normal.dot(center - (start + end) * 0.5) < 0.0 {
        -normal
    } else {
        normal
    }
}

/// A band lies completely inside the owning diamond and touches its edge.
fn territory_edge_band(diamond: &[Vec2; 4], edge: usize, width: f32) -> Vec<Vec2> {
    let center = diamond.iter().copied().sum::<Vec2>() * 0.25;
    let (a, b) = (diamond[edge], diamond[(edge + 1) % 4]);
    let offset = territory_inward_normal(a, b, center) * width;
    clip_territory_polygon(vec![a, b, b + offset, a + offset], diamond)
}

/// Clip convex geometry against the four owning half-planes, including corners.
fn clip_territory_polygon(mut polygon: Vec<Vec2>, diamond: &[Vec2; 4]) -> Vec<Vec2> {
    let center = diamond.iter().copied().sum::<Vec2>() * 0.25;
    for edge in 0..4 {
        let (a, b) = (diamond[edge], diamond[(edge + 1) % 4]);
        let normal = territory_inward_normal(a, b, center);
        let input = std::mem::take(&mut polygon);
        let Some(&last) = input.last() else {
            break;
        };
        let mut previous = last;
        let mut previous_distance = (previous - a).dot(normal);
        for current in input {
            let distance = (current - a).dot(normal);
            if (distance >= 0.0) != (previous_distance >= 0.0) {
                let t = previous_distance / (previous_distance - distance);
                polygon.push(previous + (current - previous) * t);
            }
            if distance >= 0.0 {
                polygon.push(current);
            }
            previous = current;
            previous_distance = distance;
        }
    }
    polygon
}

fn fill_territory_polygon(polygon: &[Vec2], color: Color) {
    for index in 1..polygon.len().saturating_sub(1) {
        draw_triangle(polygon[0], polygon[index], polygon[index + 1], color);
    }
}

#[cfg(test)]
mod border_tests {
    use super::*;
    fn diamond(center: Vec2) -> [Vec2; 4] {
        [
            vec2(0.0, -24.0),
            vec2(48.0, 0.0),
            vec2(0.0, 24.0),
            vec2(-48.0, 0.0),
        ]
        .map(|p| p + center)
    }
    fn assert_inside(points: &[Vec2], tile: &[Vec2; 4]) {
        let center = tile.iter().copied().sum::<Vec2>() * 0.25;
        for edge in 0..4 {
            let normal = territory_inward_normal(tile[edge], tile[(edge + 1) % 4], center);
            for point in points {
                assert!(
                    (*point - tile[edge]).dot(normal) >= -0.001,
                    "border protrudes from tile: {point:?}"
                );
            }
        }
    }
    #[test]
    fn neighboring_colors_meet_exactly_at_shared_boundary() {
        let red = diamond(Vec2::ZERO);
        let gold = diamond(vec2(48.0, 24.0));
        let red_band = territory_edge_band(&red, 1, 3.5);
        let gold_band = territory_edge_band(&gold, 3, 3.5);
        for endpoint in [red[1], red[2]] {
            assert!(red_band.iter().any(|p| p.distance(endpoint) < 0.001));
            assert!(gold_band.iter().any(|p| p.distance(endpoint) < 0.001));
        }
        assert_inside(&red_band, &red);
        assert_inside(&gold_band, &gold);
    }
    #[test]
    fn bands_and_outlines_never_protrude_at_any_corner() {
        let tile = diamond(Vec2::ZERO);
        for width in [2.75, 3.5, 4.75, 5.75] {
            for edge in 0..4 {
                let band = territory_edge_band(&tile, edge, width);
                assert!(band.len() >= 3);
                assert_inside(&band, &tile);
                assert!(band.iter().any(|p| p.distance(tile[edge]) < 0.001));
                assert!(
                    band.iter()
                        .any(|p| p.distance(tile[(edge + 1) % 4]) < 0.001)
                );
            }
        }
    }
    #[test]
    fn patterned_strokes_are_clipped_at_the_tile_corner() {
        let tile = diamond(Vec2::ZERO);
        let pattern = vec![
            vec2(43.0, -4.0),
            vec2(55.0, -4.0),
            vec2(55.0, 4.0),
            vec2(43.0, 4.0),
        ];
        let clipped = clip_territory_polygon(pattern, &tile);
        assert!(clipped.len() >= 3);
        assert_inside(&clipped, &tile);
    }
}

#[cfg(test)]
mod cache_tests {
    use super::*;
    #[test]
    fn cached_geometry_matches_uncached_clipping_and_key_changes() {
        let mut cache = BorderGeometryCache::default();
        for center in [Vec2::ZERO, vec2(391.35, 208.2)] {
            for scale in [0.55, 0.82, 1.0, 1.6] {
                let points = [
                    vec2(0.0, -24.0),
                    vec2(48.0, 0.0),
                    vec2(0.0, 24.0),
                    vec2(-48.0, 0.0),
                ]
                .map(|p| center + p * scale);
                for edge in 0..4 {
                    let geometry = cache.geometry(&points, edge, 3.5);
                    assert_eq!(geometry.band, territory_edge_band(&points, edge, 3.5));
                    assert_eq!(geometry.outline, territory_edge_band(&points, edge, 4.5));
                    let expected = geometry.stitches.clone();
                    assert_eq!(cache.geometry(&points, edge, 3.5).stitches, expected);
                }
            }
        }
        let (hits, misses, _) = cache.stats();
        assert_eq!(hits, 32);
        assert_eq!(misses, 32);
    }
    #[test]
    fn panning_cannot_grow_geometry_cache_without_bound() {
        let mut cache = BorderGeometryCache::default();
        for x in 0..MAX_GEOMETRY_ENTRIES + 4 {
            let c = vec2(x as f32, 20.0);
            let p = [
                vec2(0.0, -24.0),
                vec2(48.0, 0.0),
                vec2(0.0, 24.0),
                vec2(-48.0, 0.0),
            ]
            .map(|p| c + p);
            cache.geometry(&p, 0, 3.5);
            assert!(cache.entries.len() <= MAX_GEOMETRY_ENTRIES);
        }
    }
}
