use crate::sim::trace::Payload;
use serde::Serialize;
#[derive(Serialize)]
pub(crate) struct CityFounded {
    pub x: i32,
    pub y: i32,
    pub name: String,
}
impl Payload for CityFounded {
    const KIND: &'static str = "settlement.city_founded";
}
#[derive(Serialize)]
pub(crate) struct SiteSelected {
    pub destination: (i32, i32),
}
impl Payload for SiteSelected {
    const KIND: &'static str = "settlement.site_selected";
}
