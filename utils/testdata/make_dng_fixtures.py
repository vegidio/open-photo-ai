"""Build the synthetic DNG fixtures for utils/dng_expand_test.go.

  lossy_linear_raw.dng   64x48 LinearRaw, 3x8-bit, lossy JPEG tiles (compression 34892) of 32x32, like Adobe DNG
                         Converter's lossy DNGs and Lightroom Smart Previews. OpcodeList2 maps plane 0 with a linear
                         polynomial and plane 1 with a quadratic; plane 2 has none, so it takes LibRaw's sRGB fallback.
  jxl_linear_raw.dng     64x48 LinearRaw, 3x16-bit, lossless JPEG XL tiles (compression 52546) of 32x32, tagged linear
                         Rec.2100 like DxO PureRAW's: that colour encoding is what exposed a use-after-free in
                         gen2brain/jpegxl's WebAssembly decoder, which a plain sRGB tile does not.

IFD0 is an 8x8 RGB thumbnail and the main image lives in SubIFD 0, as in real files. Samples follow sample8() and
sample16(), and the bottom tile row is padded past the image edge.

Usage: python3 make_dng_fixtures.py   (run in this directory; needs cjpeg from libjpeg-turbo and cjxl from libjxl)
"""
import os
import struct
import subprocess
import tempfile

W, H, T = 64, 48, 32
SHORT, LONG, RATIONAL, SRATIONAL, ASCII, BYTE, UNDEFINED = 3, 4, 5, 10, 2, 1, 7


def sample8(x, y):
    # Smooth gradients, so lossy JPEG stays within a few levels of the source.
    return x * 255 // (W - 1), y * 255 // (H - 1), (x + y) * 255 // (W + H - 2)


def sample16(x, y):
    return (x * 512 + y) & 0xFFFF, (40000 - x * 300 - y * 7) & 0xFFFF, (y * 1000 + x) & 0xFFFF


def jxl_tile(tx, ty, td):
    rows = [struct.pack(">3H", *sample16(min(x, W - 1), min(y, H - 1)))
            for y in range(ty * T, ty * T + T) for x in range(tx * T, tx * T + T)]
    open(f"{td}/t.ppm", "wb").write(f"P6\n{T} {T}\n65535\n".encode() + b"".join(rows))
    subprocess.run(["cjxl", f"{td}/t.ppm", f"{td}/t.jxl", "-d", "0", "-e", "3", "-x", "color_space=RGB_D65_202_Rel_Lin",
                    "--quiet"], check=True)
    return open(f"{td}/t.jxl", "rb").read()


def jpeg_tile(tx, ty, td):
    rows = [bytes(sample8(min(x, W - 1), min(y, H - 1)))
            for y in range(ty * T, ty * T + T) for x in range(tx * T, tx * T + T)]
    open(f"{td}/t.ppm", "wb").write(f"P6\n{T} {T}\n255\n".encode() + b"".join(rows))
    subprocess.run(["cjpeg", "-quality", "100", "-sample", "1x1", "-outfile", f"{td}/t.jpg", f"{td}/t.ppm"], check=True)
    return open(f"{td}/t.jpg", "rb").read()


def map_polynomial(plane, coeffs):
    """One MapPolynomial opcode (id 8) for a single plane over the whole image. Opcode lists are big-endian."""
    params = struct.pack(">9I", 0, 0, H, W, plane, 1, 1, 1, len(coeffs) - 1) + struct.pack(f">{len(coeffs)}d", *coeffs)
    return struct.pack(">4I", 8, 0x01030000, 0, len(params)) + params


def ifd_bytes(entries, base, next_ifd=0):
    size = {BYTE: 1, ASCII: 1, SHORT: 2, LONG: 4, RATIONAL: 8, SRATIONAL: 8, UNDEFINED: 1}
    entries = sorted(entries, key=lambda e: e[0])
    head = 2 + 12 * len(entries) + 4
    extra, out = bytearray(), bytearray(struct.pack("<H", len(entries)))
    for tag, typ, vals in entries:
        if typ == ASCII:
            raw = vals.encode() + b"\0"
        elif typ == UNDEFINED:
            raw = vals
        elif typ in (RATIONAL, SRATIONAL):
            raw = b"".join(struct.pack("<ii" if typ == SRATIONAL else "<II", n, d) for n, d in vals)
        else:
            raw = struct.pack("<" + {BYTE: "B", SHORT: "H", LONG: "I"}[typ] * len(vals), *vals)
        count = len(raw) // size[typ]
        if len(raw) <= 4:
            out += struct.pack("<HHI", tag, typ, count) + raw.ljust(4, b"\0")
        else:
            out += struct.pack("<HHII", tag, typ, count, base + head + len(extra))
            extra += raw + (b"\0" if len(raw) % 2 else b"")
    return bytes(out + struct.pack("<I", next_ifd) + extra)


def build(path, tiles, main_tags, dng_version=(1, 4, 0, 0)):
    data = bytearray(b"II*\0\0\0\0\0")
    thumb_off = len(data)
    data += bytes([128] * 8 * 8 * 3)
    offs = []
    for t in tiles:
        data += b"\0" * (len(data) % 2)
        offs.append(len(data))
        data += t
    data += b"\0" * (len(data) % 2)
    sub_off = len(data)
    data += ifd_bytes(main_tags + [
        (254, LONG, [0]), (256, LONG, [W]), (257, LONG, [H]), (262, SHORT, [34892]), (277, SHORT, [3]),
        (284, SHORT, [1]), (322, LONG, [T]), (323, LONG, [T]), (324, LONG, offs), (325, LONG, [len(t) for t in tiles]),
    ], sub_off)
    data += b"\0" * (len(data) % 2)
    ifd0_off = len(data)
    ident = [(1, 1), (0, 1), (0, 1), (0, 1), (1, 1), (0, 1), (0, 1), (0, 1), (1, 1)]
    data += ifd_bytes([
        (254, LONG, [1]), (256, LONG, [8]), (257, LONG, [8]), (258, SHORT, [8, 8, 8]), (259, SHORT, [1]),
        (262, SHORT, [2]), (271, ASCII, "OPAI"), (272, ASCII, "DNG Fixture"), (273, LONG, [thumb_off]),
        (277, SHORT, [3]), (278, LONG, [8]), (279, LONG, [8 * 8 * 3]), (330, LONG, [sub_off]),
        (50706, BYTE, list(dng_version)), (50707, BYTE, [1, 4, 0, 0]), (50708, ASCII, "OPAI DNG Fixture"),
        (50721, SRATIONAL, ident), (50778, SHORT, [21]), (50728, RATIONAL, [(1, 1)] * 3),
    ], ifd0_off)
    struct.pack_into("<I", data, 4, ifd0_off)
    open(path, "wb").write(data)
    print(path, len(data), "bytes,", len(tiles), "tiles")


# The fixture's photometric interpretation is LinearRaw (34892, the same number as the lossy compression by
# coincidence); both are set explicitly above and below.
tiles_across, tiles_down = W // T, -(-H // T)
with tempfile.TemporaryDirectory() as td:
    lossy = [jpeg_tile(tx, ty, td) for ty in range(tiles_down) for tx in range(tiles_across)]
    jxl = [jxl_tile(tx, ty, td) for ty in range(tiles_down) for tx in range(tiles_across)]
opcodes = map_polynomial(0, [0.0, 1.0]) + map_polynomial(1, [0.0, 0.0, 1.0])
build("lossy_linear_raw.dng", lossy, [
    (258, SHORT, [8, 8, 8]), (259, SHORT, [34892]), (50717, SHORT, [255, 255, 255]),
    (51009, UNDEFINED, struct.pack(">I", 2) + opcodes),
])
build("jxl_linear_raw.dng", jxl, [
    (258, SHORT, [16, 16, 16]), (259, SHORT, [52546]), (50717, SHORT, [65535, 65535, 65535]),
], dng_version=(1, 7, 0, 0))
