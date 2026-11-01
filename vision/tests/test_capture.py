import os
import sys
import threading
import unittest
from unittest.mock import patch

sys.path.insert(0, os.path.abspath(os.path.join(os.path.dirname(__file__), "..")))

from air_control_vision.service import LatestFrameCapture, State, preview_settings


class FakeCamera:
    """A read blocks until the test supplies a frame, like a real camera."""

    def __init__(self):
        import queue
        self.frames = queue.Queue()
        self.released = threading.Event()
        self.owner = None
        self.release_owner = None

    def read(self):
        self.owner = threading.get_ident()
        return self.frames.get(timeout=2)

    def release(self):
        self.release_owner = threading.get_ident()
        self.released.set()


class CaptureTest(unittest.TestCase):
    def start_reader(self, camera):
        reader = LatestFrameCapture(None, {})
        with patch("air_control_vision.service.open_capture", return_value=camera):
            reader.start()
            self.assertTrue(reader.wait_ready(1))
        return reader

    def stop_reader(self, reader, camera):
        reader.stop_event.set()
        camera.frames.put((False, None))
        self.assertTrue(reader.stop())
        self.assertTrue(camera.released.is_set())
        self.assertEqual(camera.owner, camera.release_owner)

    def test_latest_frame_replaces_old_frames_and_is_not_repeated(self):
        camera = FakeCamera()
        reader = self.start_reader(camera)
        try:
            camera.frames.put((True, "first"))
            first = reader.read_latest(0, 1)
            self.assertEqual(first[1], "first")
            self.assertIsNone(reader.read_latest(first[0], 0.01))
            camera.frames.put((True, "second"))
            camera.frames.put((True, "third"))
            with reader.condition:
                self.assertTrue(reader.condition.wait_for(lambda: reader.sequence == 3, 1))
            newest = reader.read_latest(first[0], 1)
            self.assertEqual(newest[0], 3)
            self.assertEqual(newest[1], "third")
            self.assertGreater(newest[2], first[2])
            self.assertGreaterEqual(newest[3], 0)
            self.assertIsNone(reader.read_latest(3, 0.01))
        finally:
            self.stop_reader(reader, camera)

    def test_device_loss_releases_and_a_replacement_can_start(self):
        camera = FakeCamera()
        reader = self.start_reader(camera)
        for _ in range(12):
            camera.frames.put((False, None))
        self.assertTrue(reader.finished.wait(1))
        self.assertTrue(reader.stop())
        self.assertTrue(camera.released.is_set())
        self.assertIsNone(reader.read_latest(0, 0.01))
        replacement = FakeCamera()
        reader = self.start_reader(replacement)
        try:
            replacement.frames.put((True, "reconnected"))
            self.assertEqual(reader.read_latest(0, 1)[1], "reconnected")
        finally:
            self.stop_reader(reader, replacement)

    def test_open_failure_is_reported_without_hanging(self):
        reader = LatestFrameCapture(None, {})
        with patch("air_control_vision.service.open_capture", return_value=None):
            reader.start()
            self.assertFalse(reader.wait_ready(1))
        self.assertTrue(reader.finished.wait(1))
        self.assertTrue(reader.stop())

    def test_soft_configuration_does_not_reopen_camera(self):
        state = State()
        config = {"camera_index": 0, "width": 640, "height": 480,
                  "fps": 30, "preview": True, "preview_every": 2}
        state.set_config(config)
        generation, _ = state.current_config()
        state.set_config({**config, "preview_every": 1, "preview_width": 480,
                          "preview_quality": 70, "mirror": False})
        self.assertEqual(state.current_config()[0], generation)
        state.set_config({**config, "camera_index": 1})
        self.assertEqual(state.current_config()[0], generation + 1)
        state.clear_config()
        self.assertEqual(state.current_config(), (generation + 2, None))

    def test_preview_settings_are_bounded_and_compatible(self):
        self.assertEqual(preview_settings({}), (720, 78))
        self.assertEqual(preview_settings({"preview_width": 480, "preview_quality": 70}), (480, 70))
        self.assertEqual(preview_settings({"preview_width": -1, "preview_quality": 200}), (240, 90))


if __name__ == "__main__":
    unittest.main()
