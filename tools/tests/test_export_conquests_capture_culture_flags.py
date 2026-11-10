import struct
import sys
import unittest
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))

from export_conquests_capture_culture_flags import game_retention_values


def make_game_section(records):
    data = bytearray(b"GAME" + struct.pack("<I", len(records)))
    for playable_civs, retain_culture in records:
        record_length = 7333 + 8 * playable_civs
        record = bytearray(record_length + 4)
        struct.pack_into("<i", record, 0, record_length)
        struct.pack_into("<i", record, 12, playable_civs)
        retention_offset = 16 + 8 * playable_civs + 5304 + 1996
        record[retention_offset] = retain_culture
        data.extend(record)
    return bytes(data)


class GameRetentionFieldTests(unittest.TestCase):
    def test_reads_retention_for_game_records_with_variable_civ_lists(self):
        data = make_game_section([(0, 0), (31, 1)])

        self.assertEqual(game_retention_values(data), [(0, 0), (1, 1)])

    def test_rejects_unknown_retention_byte(self):
        data = make_game_section([(31, 2)])

        with self.assertRaisesRegex(ValueError, "Unexpected GAME.RetainCulture"):
            game_retention_values(data)

    def test_rejects_missing_game_section(self):
        with self.assertRaisesRegex(ValueError, "GAME section"):
            game_retention_values(b"RULE\x00\x00\x00\x00")


if __name__ == "__main__":
    unittest.main()
