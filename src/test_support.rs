//! Immutable seeded geography shared by non-mapgen tests. Every caller owns an
//! independent copy, including the post-generation gameplay RNG state.
use crate::world::GeneratedMap;
use std::sync::OnceLock;

pub(crate) fn seeded_map() -> GeneratedMap {
    static MAP: OnceLock<GeneratedMap> = OnceLock::new();
    let map = MAP.get_or_init(|| crate::mapgen::generate(42));
    GeneratedMap {
        seed: map.seed,
        map_seed: map.map_seed,
        map_wrap: map.map_wrap,
        game_random: map.game_random.clone(),
        tiles: map.tiles.clone(),
        starts: map.starts.clone(),
        civilization_count: map.civilization_count,
    }
}

#[test]
fn cached_maps_preserve_generation_and_isolate_mutations_and_random_draws() {
    let fresh = crate::mapgen::generate(42);
    let mut first = seeded_map();
    let second = seeded_map();
    assert_eq!(fresh.seed, second.seed);
    assert_eq!(fresh.map_seed, second.map_seed);
    assert_eq!(fresh.map_wrap, second.map_wrap);
    assert_eq!(fresh.starts, second.starts);
    assert_eq!(fresh.civilization_count, second.civilization_count);
    assert_eq!(
        bincode::serialize(&fresh.tiles).unwrap(),
        bincode::serialize(&second.tiles).unwrap()
    );
    assert_eq!(fresh.game_random, second.game_random);
    first.tiles[0].hut = !first.tiles[0].hut;
    first.starts.clear();
    first.game_random.draw(1000);
    let third = seeded_map();
    assert_eq!(third.tiles[0].hut, second.tiles[0].hut);
    assert_eq!(third.starts, second.starts);
    assert_eq!(third.game_random, second.game_random);
}

/// Check every `const NAME: Self = Self(N);` in `source` against the catalog
/// entry now at index N. `labels(N)` returns that entry's accepted spellings
/// (id, display name). Reordering a TOML catalog must fail here instead of
/// silently retargeting a hand-written index constant or persisted save value.
pub(crate) fn assert_index_constants_match_catalog(
    source: &str,
    labels: impl Fn(usize) -> Vec<String>,
    minimum: usize,
) {
    fn normalized(text: &str) -> String {
        text.chars()
            .filter(char::is_ascii_alphanumeric)
            .map(|c| c.to_ascii_lowercase())
            .collect()
    }
    let mut checked = 0;
    for line in source.lines() {
        let Some(rest) = line.trim().strip_prefix("pub(crate) const ") else {
            continue;
        };
        let Some((name, tail)) = rest.split_once(": Self = Self(") else {
            continue;
        };
        let Some(index) = tail
            .strip_suffix(");")
            .and_then(|n| n.parse::<usize>().ok())
        else {
            continue;
        };
        let spellings = labels(index);
        let expected = normalized(name);
        assert!(
            spellings.iter().any(|label| normalized(label) == expected),
            "{name} = Self({index}) but index {index} is {spellings:?}"
        );
        checked += 1;
    }
    assert!(checked >= minimum, "only checked {checked} constants");
}
