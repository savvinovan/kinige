#!/usr/bin/env python3
"""Compares two PNG screenshots of the same window (slice 5c, the GIF steps of `5c.sh`): `image-diff.py a.png b.png`.

Exit 0: the pictures are identical. Exit 1: they differ; the line printed says how many pixels and the bounding box (`x0,y0,x1,y1`, edges included).
Exit 2: a file cannot be read as a PNG this script understands (8 bits per channel, RGB or RGBA, not interlaced), the sizes differ, or an option is malformed.
Exit 3: they differ but the box breaks a condition given with `--min-box=WxH` (at least that wide and that tall) or `--within=x0,y0,x1,y1` (inside that box).
Standard library only, so it runs on the runner and in a test. The steps use it to prove that a GIF moves (two shots half a second apart differ) and
that nothing else does (the same two shots before the GIF is pressed are identical).
"""

import struct
import sys
import zlib

SIGNATURE = b"\x89PNG\r\n\x1a\n"
CHANNELS = {2: 3, 6: 4}  # colour type -> bytes per pixel at 8 bits


class Unreadable(Exception):
    pass


def read_png(path):
    """(width, height, bytes per pixel, rows) of an 8-bit RGB or RGBA PNG, the rows unfiltered."""
    try:
        with open(path, "rb") as handle:
            data = handle.read()
    except OSError as error:
        raise Unreadable(f"{path}: {error.strerror}")
    if not data.startswith(SIGNATURE):
        raise Unreadable(f"{path}: not a PNG")
    position, header, compressed = len(SIGNATURE), None, []
    while position + 8 <= len(data):
        (length,) = struct.unpack(">I", data[position : position + 4])
        kind = data[position + 4 : position + 8]
        body = data[position + 8 : position + 8 + length]
        if len(body) != length:
            raise Unreadable(f"{path}: truncated chunk")
        if kind == b"IHDR" and length == 13:
            header = struct.unpack(">IIBBBBB", body)
        elif kind == b"IDAT":
            compressed.append(body)
        elif kind == b"IEND":
            break
        position += 12 + length
    if header is None or not compressed:
        raise Unreadable(f"{path}: no image data")
    width, height, depth, color, _method, _filter, interlace = header
    if depth != 8 or color not in CHANNELS or interlace != 0 or width == 0 or height == 0:
        raise Unreadable(f"{path}: only 8-bit RGB or RGBA, not interlaced (depth {depth}, type {color}, interlace {interlace})")
    pixel = CHANNELS[color]
    stride = width * pixel
    try:
        raw = zlib.decompress(b"".join(compressed))
    except zlib.error:
        raise Unreadable(f"{path}: damaged image data")
    if len(raw) != height * (stride + 1):
        raise Unreadable(f"{path}: the data does not fill {width}x{height}")
    rows, previous = [], bytearray(stride)
    for y in range(height):
        start = y * (stride + 1)
        kind, line = raw[start], bytearray(raw[start + 1 : start + 1 + stride])
        for i in range(stride):
            left = line[i - pixel] if i >= pixel else 0
            up = previous[i]
            corner = previous[i - pixel] if i >= pixel else 0
            if kind == 1:
                line[i] = (line[i] + left) & 255
            elif kind == 2:
                line[i] = (line[i] + up) & 255
            elif kind == 3:
                line[i] = (line[i] + (left + up) // 2) & 255
            elif kind == 4:
                guess = left + up - corner
                a, b, c = abs(guess - left), abs(guess - up), abs(guess - corner)
                line[i] = (line[i] + (left if a <= b and a <= c else up if b <= c else corner)) & 255
            elif kind != 0:
                raise Unreadable(f"{path}: unknown filter {kind}")
        rows.append(bytes(line))
        previous = line
    return width, height, pixel, rows


def main(argv, min_box=None, within=None):
    if len(argv) != 3:
        print("usage: image-diff.py [--min-box=WxH] [--within=x0,y0,x1,y1] a.png b.png")
        return 2
    try:
        first, second = read_png(argv[1]), read_png(argv[2])
    except Unreadable as error:
        print(f"unreadable: {error}")
        return 2
    if first[:3] != second[:3]:
        print(f"unreadable: the sizes differ ({first[0]}x{first[1]} against {second[0]}x{second[1]})")
        return 2
    width, height, pixel, rows_a = first
    rows_b = second[3]
    changed, x0, y0, x1, y1 = 0, width, height, -1, -1
    for y in range(height):
        if rows_a[y] == rows_b[y]:
            continue
        for x in range(width):
            if rows_a[y][x * pixel : (x + 1) * pixel] != rows_b[y][x * pixel : (x + 1) * pixel]:
                changed += 1
                x0, x1, y0, y1 = min(x0, x), max(x1, x), min(y0, y), max(y1, y)
    if changed == 0:
        print("identical")
        return 0
    summary = f"differ: {changed} pixels, box {x0},{y0},{x1},{y1}"
    # A caret blinking in a field also "differs": the box must be a picture's, not a line's (`--min-box`), and where the picture is (`--within`).
    if min_box is not None and (x1 - x0 + 1 < min_box[0] or y1 - y0 + 1 < min_box[1]):
        print(f"{summary}; too small, the box must be at least {min_box[0]}x{min_box[1]}")
        return 3
    if within is not None and not (x0 >= within[0] and y0 >= within[1] and x1 <= within[2] and y1 <= within[3]):
        print(f"{summary}; not inside {within[0]},{within[1]},{within[2]},{within[3]}")
        return 3
    print(summary)
    return 1


def options(argv):
    """(positional arguments, min box or None, within box or None); `None` for the arguments when an option is malformed."""
    rest, min_box, within = [], None, None
    try:
        for argument in argv[1:]:
            if argument.startswith("--min-box="):
                width, height = argument[len("--min-box=") :].split("x")
                min_box = (int(width), int(height))
            elif argument.startswith("--within="):
                box = tuple(int(part) for part in argument[len("--within=") :].split(","))
                if len(box) != 4:
                    raise ValueError
                within = box
            else:
                rest.append(argument)
    except ValueError:
        return None, None, None
    return rest, min_box, within


if __name__ == "__main__":
    positional, min_box, within = options(sys.argv)
    sys.exit(2 if positional is None else main(["image-diff.py", *positional], min_box, within))
