//! How dragging one rate slider redistributes trade among the other two.

/// Moves `rates[index]` toward `target` in steps of ten, keeping the total at
/// 100 and every share within `0..=max`. Raising a rate takes from the largest
/// other share; lowering gives to the smallest other share that has room. It
/// stops early if no other share can give or take.
pub(super) fn rebalanced_rates(
    mut rates: [i32; 3],
    max: i32,
    index: usize,
    target: i32,
) -> [i32; 3] {
    let target = target.clamp(0, max);
    let others = || (0..3).filter(move |&other| other != index);
    while rates[index] < target {
        let Some(donor) = others()
            .filter(|&other| rates[other] >= 10)
            .max_by_key(|&other| (rates[other], std::cmp::Reverse(other)))
        else {
            break;
        };
        rates[donor] -= 10;
        rates[index] += 10;
    }
    while rates[index] > target {
        let Some(receiver) = others()
            .filter(|&other| rates[other] + 10 <= max)
            .min_by_key(|&other| (rates[other], other))
        else {
            break;
        };
        rates[receiver] += 10;
        rates[index] -= 10;
    }
    rates
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn raising_takes_from_the_largest_other_rate() {
        assert_eq!(rebalanced_rates([60, 40, 0], 60, 2, 20), [40, 40, 20]);
        assert_eq!(rebalanced_rates([30, 50, 20], 60, 0, 50), [50, 30, 20]);
    }

    #[test]
    fn lowering_gives_to_the_smallest_other_rate_with_room() {
        assert_eq!(rebalanced_rates([60, 40, 0], 60, 0, 40), [40, 40, 20]);
        // Tax is at the cap, so the freed share can only go to luxury.
        assert_eq!(rebalanced_rates([30, 60, 10], 60, 0, 10), [10, 60, 30]);
    }

    #[test]
    fn the_target_is_clamped_to_the_cap_and_the_total_stays_100() {
        assert_eq!(rebalanced_rates([40, 40, 20], 60, 0, 100), [60, 20, 20]);
        for index in 0..3 {
            for target in (0..=100).step_by(10) {
                let rates = rebalanced_rates([40, 40, 20], 60, index, target);
                assert_eq!(rates.iter().sum::<i32>(), 100);
                assert!(rates.iter().all(|&rate| (0..=60).contains(&rate)));
            }
        }
    }

    #[test]
    fn lowering_never_pushes_another_rate_past_the_cap() {
        assert_eq!(rebalanced_rates([20, 60, 20], 60, 0, 0), [0, 60, 40]);
    }
}
