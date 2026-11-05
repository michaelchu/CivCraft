/* Independent native offensive-advisor arithmetic fixture.
 * Extracted from the bundled Freeciv source:
 *   ai/default/daiunit.c:341 kill_desire
 *   server/advisors/advtools.c:29 amortize
 *   common/fc_types.h: adv_want = float; MORT = 24.
 * The only edits are replacing SHIELD_WEIGHTING/MORT with their constants,
 * removing Freeciv headers and adding this output-only main.
 *
 * Reproduce from the app directory:
 *   cc -std=c11 -O0 -ffp-contract=off tests/fixtures/offense_arithmetic.c \
 *      -lm -o /tmp/civcraft-offense-arithmetic
 *   /tmp/civcraft-offense-arithmetic
 */
#include <math.h>
#include <stdint.h>
#include <stdio.h>

typedef float adv_want;
#define SHIELD_WEIGHTING (adv_want)17
#define MORT 24

static adv_want kill_desire(adv_want benefit, int attack, int loss, int vuln,
                           int victim_count)
{
  adv_want desire;
  desire = (benefit * attack - loss * vuln) * victim_count
    * SHIELD_WEIGHTING / (attack + vuln * victim_count);
  return desire;
}

static adv_want amortize(adv_want benefit, int delay)
{
  double discount = 1.0 - 1.0 / ((double)MORT);
  return benefit * pow(discount, delay);
}

int main(void)
{
  const adv_want values[] = {
    kill_desire(80.25f, 1600, 40, 625, 3),
    kill_desire(30, 400, 20, 225, 1),
    amortize(1234.5f, 7)
  };
  for (unsigned int i = 0; i < sizeof(values) / sizeof(values[0]); i++) {
    union { float value; uint32_t bits; } result = { values[i] };
    printf("%.9g %u\n", result.value, result.bits);
  }
  return 0;
}
