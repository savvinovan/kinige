#!/usr/bin/env python3
"""Tests of image-diff.py with negative controls: every exit code is named, so the script cannot pass for the wrong reason (a crash is not 2).
Run: python3 -I scripts/screens/image-diff.test.py"""

import os
import struct
import subprocess
import sys
import tempfile
import unittest
import zlib

SCRIPT = os.path.join(os.path.dirname(os.path.abspath(__file__)), "image-diff.py")


def chunk(kind, body):
    return struct.pack(">I", len(body)) + kind + body + struct.pack(">I", zlib.crc32(kind + body))


def png(pixels, color=6, depth=8, interlace=0, sub=False):
    """pixels: rows of tuples. With `sub` every row is stored with the Sub filter (type 1), which the reader must undo."""
    height, width, channels = len(pixels), len(pixels[0]), 4 if color == 6 else 3
    raw = bytearray()
    for row in pixels:
        line = bytes(value for pixel in row for value in pixel[:channels])
        if sub:
            raw.append(1)
            raw.extend((line[i] - (line[i - channels] if i >= channels else 0)) & 255 for i in range(len(line)))
        else:
            raw.append(0)
            raw.extend(line)
    header = struct.pack(">IIBBBBB", width, height, depth, color, 0, 0, interlace)
    return b"\x89PNG\r\n\x1a\n" + chunk(b"IHDR", header) + chunk(b"IDAT", zlib.compress(bytes(raw))) + chunk(b"IEND", b"")


def grid(width, height, base=(10, 20, 30, 255)):
    return [[base for _ in range(width)] for _ in range(height)]


class ImageDiff(unittest.TestCase):
    def run_script(self, first, second):
        with tempfile.TemporaryDirectory() as folder:
            paths = []
            for index, content in enumerate((first, second)):
                path = os.path.join(folder, f"{index}.png")
                with open(path, "wb") as handle:
                    handle.write(content)
                paths.append(path)
            done = subprocess.run([sys.executable, "-I", SCRIPT, *paths], capture_output=True, text=True)
            return done.returncode, done.stdout.strip(), done.stderr.strip()

    def test_identical_pictures_exit_0(self):
        code, out, err = self.run_script(png(grid(5, 4)), png(grid(5, 4)))
        self.assertEqual((code, out, err), (0, "identical", ""))

    def test_a_changed_pixel_exits_1_and_the_box_is_right(self):
        moved = grid(6, 5)
        moved[1][2] = (200, 20, 30, 255)
        moved[3][4] = (10, 20, 31, 255)
        code, out, err = self.run_script(png(grid(6, 5)), png(moved))
        self.assertEqual((code, err), (1, ""))
        self.assertEqual(out, "differ: 2 pixels, box 2,1,4,3")

    def test_the_sub_filter_is_undone_so_the_same_picture_in_two_encodings_is_identical(self):
        picture = [[(x * 20, y * 30, 7, 255) for x in range(7)] for y in range(6)]
        code, out, _ = self.run_script(png(picture), png(picture, sub=True))
        self.assertEqual((code, out), (0, "identical"))

    def test_rgb_is_read_too(self):
        code, out, _ = self.run_script(png(grid(3, 3), color=2), png(grid(3, 3), color=2))
        self.assertEqual((code, out), (0, "identical"))

    def test_garbage_and_foreign_pictures_exit_2_not_crash(self):
        cases = {
            "not a png": b"hello",
            "empty": b"",
            "truncated": png(grid(4, 4))[:40],
            "16 bits": png(grid(2, 2), depth=16),
            "interlaced": png(grid(2, 2), interlace=1),
        }
        for name, bad in cases.items():
            code, out, err = self.run_script(bad, png(grid(4, 4)))
            self.assertEqual(code, 2, f"{name}: {out} {err}")
            self.assertTrue(out.startswith("unreadable:"), f"{name}: {out}")
            self.assertNotIn("Traceback", err, name)

    def test_different_sizes_exit_2(self):
        code, out, _ = self.run_script(png(grid(4, 4)), png(grid(5, 4)))
        self.assertEqual(code, 2)
        self.assertIn("the sizes differ", out)

    def test_a_missing_file_exits_2(self):
        done = subprocess.run([sys.executable, "-I", SCRIPT, "/nonexistent/a.png", "/nonexistent/b.png"], capture_output=True, text=True)
        self.assertEqual(done.returncode, 2)

    def test_the_arguments_are_checked(self):
        done = subprocess.run([sys.executable, "-I", SCRIPT], capture_output=True, text=True)
        self.assertEqual(done.returncode, 2)


if __name__ == "__main__":
    unittest.main()
