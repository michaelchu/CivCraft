import importlib.util
import unittest
from pathlib import Path


SCRIPT = Path(__file__).resolve().parents[1] / "export_civ3_conquests_culture.py"
SPEC = importlib.util.spec_from_file_location("culture_exporter", SCRIPT)
assert SPEC and SPEC.loader
MODULE = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(MODULE)


class EffectiveCulturalConversionTests(unittest.TestCase):
    def test_secret_police_hq_maps_to_culture_capital_catalog_entry(self):
        self.assertEqual(MODULE.CIVCRAFT_BUILDING_INDEX_BY_BIQ_INDEX[82], 90)

    def test_default_rules_use_the_documented_enabled_game_default(self):
        self.assertTrue(MODULE.effective_cultural_conversions(1, False))

    def test_custom_rules_use_the_biq_conversion_flag(self):
        self.assertFalse(MODULE.effective_cultural_conversions(0, False))
        self.assertTrue(MODULE.effective_cultural_conversions(0, True))

    def test_rejects_unrecognized_default_game_rules_values(self):
        with self.assertRaisesRegex(ValueError, "DefaultGameRules"):
            MODULE.effective_cultural_conversions(2, False)


if __name__ == "__main__":
    unittest.main()
