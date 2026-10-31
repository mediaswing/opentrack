"""Renders Open Budget Tracker's icon (three rising white bars on a green
rounded square) as PNG, ICO and ICNS, with no libraries beyond Python's own."""
import struct, sys, zlib, os

def inside_round_rect(x, y, x0, y0, x1, y1, r):
    if x < x0 or x > x1 or y < y0 or y > y1:
        return False
    cx = min(max(x, x0 + r), x1 - r)
    cy = min(max(y, y0 + r), y1 - r)
    return (x - cx) ** 2 + (y - cy) ** 2 <= r * r

BARS = [(0.21, 0.36, 0.52), (0.425, 0.575, 0.38), (0.64, 0.79, 0.22)]
BOTTOM = 0.78
TOP_GREEN, BOTTOM_GREEN = (46, 166, 108), (20, 112, 64)

def sample(x, y):
    """Colour and alpha at a point in the unit square."""
    if not inside_round_rect(x, y, 0.03, 0.03, 0.97, 0.97, 0.21):
        return None
    for left, right, top in BARS:
        if inside_round_rect(x, y, left, top, right, BOTTOM + 0.05, 0.035) and y <= BOTTOM:
            return (255, 255, 255)
    t = (y - 0.03) / 0.94
    return tuple(round(a + (b - a) * t) for a, b in zip(TOP_GREEN, BOTTOM_GREEN))

def render(size, ss=4):
    rows = []
    for py in range(size):
        row = bytearray()
        for px in range(size):
            r = g = b = n = 0
            for sy in range(ss):
                for sx in range(ss):
                    c = sample((px + (sx + 0.5) / ss) / size, (py + (sy + 0.5) / ss) / size)
                    if c:
                        r += c[0]; g += c[1]; b += c[2]; n += 1
            if n:
                row += bytes((r // n, g // n, b // n, 255 * n // (ss * ss)))
            else:
                row += b"\0\0\0\0"
        rows.append(bytes(row))
    return rows

def png(size):
    rows = render(size, 4 if size <= 256 else 2)
    raw = b"".join(b"\0" + r for r in rows)
    def chunk(kind, data):
        return struct.pack(">I", len(data)) + kind + data + struct.pack(">I", zlib.crc32(kind + data))
    return (b"\x89PNG\r\n\x1a\n" + chunk(b"IHDR", struct.pack(">IIBBBBB", size, size, 8, 6, 0, 0, 0))
            + chunk(b"IDAT", zlib.compress(raw, 9)) + chunk(b"IEND", b""))

out = sys.argv[1]
os.makedirs(out, exist_ok=True)
pngs = {s: png(s) for s in (16, 24, 32, 48, 64, 128, 256, 512, 1024)}

with open(os.path.join(out, "opentrack.png"), "wb") as f:
    f.write(pngs[256])

# ICO with PNG-compressed images, which Windows Vista and later read.
ico_sizes = [16, 24, 32, 48, 64, 128, 256]
header = struct.pack("<HHH", 0, 1, len(ico_sizes))
offset = 6 + 16 * len(ico_sizes)
entries, data = b"", b""
for s in ico_sizes:
    d = pngs[s]
    entries += struct.pack("<BBBBHHII", s % 256, s % 256, 0, 0, 1, 32, len(d), offset + len(data))
    data += d
with open(os.path.join(out, "opentrack.ico"), "wb") as f:
    f.write(header + entries + data)

# ICNS with PNG images: the types macOS 10.7 and later read.
icns_types = [(b"icp4", 16), (b"icp5", 32), (b"icp6", 64), (b"ic07", 128), (b"ic08", 256),
              (b"ic09", 512), (b"ic10", 1024), (b"ic11", 32), (b"ic12", 64), (b"ic13", 256), (b"ic14", 512)]
body = b"".join(t + struct.pack(">I", 8 + len(pngs[s])) + pngs[s] for t, s in icns_types)
with open(os.path.join(out, "opentrack.icns"), "wb") as f:
    f.write(b"icns" + struct.pack(">I", 8 + len(body)) + body)
print("written", sorted(os.listdir(out)))
