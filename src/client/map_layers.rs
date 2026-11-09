//! Ordered Civ III map-painter passes, shared by the world and city work map.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum MapLayer {
    Blend,
    Terrain,
    Water,
    Roads,
    Special1,
    Grid,
    City,
    Special2,
}
pub(super) const MAP_LAYERS: [MapLayer; 8] = [
    MapLayer::Blend,
    MapLayer::Terrain,
    MapLayer::Water,
    MapLayer::Roads,
    MapLayer::Special1,
    MapLayer::Grid,
    MapLayer::City,
    MapLayer::Special2,
];
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn water_roads_city_and_pollution_order_is_explicit() {
        let index = |layer| MAP_LAYERS.iter().position(|&l| l == layer).unwrap();
        assert!(index(MapLayer::Blend) < index(MapLayer::Terrain));
        assert!(index(MapLayer::Terrain) < index(MapLayer::Water));
        assert!(index(MapLayer::Water) < index(MapLayer::Roads));
        assert!(index(MapLayer::Roads) < index(MapLayer::Special1));
        assert!(index(MapLayer::Special1) < index(MapLayer::Grid));
        assert!(index(MapLayer::Grid) < index(MapLayer::City));
        assert!(index(MapLayer::City) < index(MapLayer::Special2));
    }
}
