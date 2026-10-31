import json
import os
import sys
import unittest

sys.path.insert(0, os.path.abspath(os.path.join(os.path.dirname(__file__), "..")))

from air_control_vision.protocol import hello_message, tracking_message


class ProtocolTest(unittest.TestCase):
    def test_tracking_shape(self) -> None:
        landmarks = [[0.1, 0.2, 0.0] for _ in range(21)]
        message = tracking_message(
            4,
            640,
            480,
            3.5,
            11.0,
            [{"handedness": "Right", "score": 0.9, "landmarks": landmarks}],
            None,
            frame_ms=33,
            sharpness=88.5,
            brightness=120.0,
            flicker=2.5,
        )
        encoded = json.dumps(message)
        parsed = json.loads(encoded)
        self.assertEqual(parsed["version"], 1)
        self.assertEqual(parsed["type"], "tracking")
        self.assertEqual(len(parsed["hands"][0]["landmarks"]), 21)
        self.assertEqual(parsed["frame_ms"], 33)
        self.assertAlmostEqual(parsed["sharpness"], 88.5)
        self.assertNotIn("command", parsed)

    def test_hello(self) -> None:
        self.assertEqual(hello_message(7, "test")["process_id"], 7)


if __name__ == "__main__":
    unittest.main()
