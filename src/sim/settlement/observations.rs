use crate::sim::trace::Payload;
use serde::Serialize;
#[derive(Serialize)]
pub(crate) struct CityFounded {
    pub x: i32,
    pub y: i32,
    pub name: String,
    pub nearest_city_id: Option<u32>,
    pub nearest_city_distance: Option<i32>,
}
impl Payload for CityFounded {
    const KIND: &'static str = "settlement.city_founded";
}
#[derive(Serialize)]
pub(crate) struct SiteSelected {
    pub destination: (i32, i32),
    pub score: f64,
    pub travel_distance: i32,
    pub nearest_city_id: Option<u32>,
    pub nearest_city_distance: Option<i32>,
    /// Shared potential-work tiles with all current city radii, counted per city.
    pub city_radius_overlap_tiles: usize,
    /// Position of the tile selected for the first citizen to work, if any.
    pub first_worker_tile: Option<(i32, i32)>,
}
impl Payload for SiteSelected {
    const KIND: &'static str = "settlement.site_selected";
}
