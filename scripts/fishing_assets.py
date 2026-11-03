"""Make the Fishing toy's game files (`toy_fishing`, SJK's own).

A fishing rod propped beside the player, a bucket at their feet, a float on the water
and the line between, and the twelve fish. Nothing here comes from another game: every
part is cylinders, lofted shapes, flat colours or skins painted here. This script writes, into the output folder:

- `fishing.png`: the colour atlas, 128x128, a flat colour in each 16x16 cell;
- `fishing_rod.md3`: a spinning rod along +X, 72 units from its butt (the origin) to its
  tip, with its cork grips, reel seat, guides and reel;
- `fishing_bucket.md3`: an open bucket with water in it and a handle, its origin in the
  middle of its bottom;
- `fishing_float.md3`: the float, red over white, its origin where it meets the water;
- `fishing_line.md3`: a thin square strand along +X, 64 units long (scaled to length);
- `fishing_fish_<id>.md3`, one for each of the twelve fish (`FISH`): its body, fins and
  tail along -X from its mouth at the origin (where the line holds it), lofted from a
  depth profile, a darter's slim, a carp's deep, the eel's long and the ray's a flat
  diamond, with eyes, and some with spines, whiskers or crystals;
- `fishing_fish.png`: the fish atlas, 512x512, each fish's own painted skin and fins in
  a 128x128 tile (countershading, scales, gill covers and each fish's bars, spots,
  patches or glow);
- `fishing_pond.md3`: a small round pond with a muddy bank, mossy stones round it,
  floating leaves and reeds, its origin in the middle of its water at ground level, for
  a cast where there is no water (the toy fishes anywhere). Its water, bank, stones and
  leaves are drawn with the installed game's own swamp textures (named in
  `fishing.shader`; nothing of them is copied here), so the pond looks like the maps'
  water; only the reeds use the atlas;
- `toy_fishing.png`: the toy's icon, 128x128 like `gfx/mp/f_icon_*`: a rod over a
  bucket, on transparency.

    python scripts/fishing_assets.py crates/sjk-viewer/assets/fishing

Standard library only; the output is deterministic. The shader (`fishing.shader`) is
written by hand.
"""

import math
import struct
import sys
import zlib
from pathlib import Path

SHADER = "models/sjk/fishing"
POND_SHADER = "models/sjk/fishing_pond_water"
BANK_SHADER = "models/sjk/fishing_pond_bank"
STONE_SHADER = "models/sjk/fishing_pond_stone"
LEAF_SHADER = "models/sjk/fishing_pond_leaves"
ATLAS = 128
CELL = 16
ICON_SIZE = 128
ROD_LENGTH = 72.0
LINE_LENGTH = 64.0

# The atlas cells, by name: (column, row) and colour.
COLOURS = {
    "cork": ((0, 0), (196, 160, 112)),
    "rod": ((1, 0), (58, 66, 44)),
    "reel": ((2, 0), (178, 184, 190)),
    "grip": ((3, 0), (34, 30, 28)),
    "bucket": ((0, 1), (120, 132, 142)),
    "rim": ((1, 1), (88, 96, 104)),
    "water": ((2, 1), (40, 92, 132)),
    "float_red": ((3, 1), (214, 44, 36)),
    "float_white": ((0, 2), (236, 236, 228)),
    "line": ((1, 2), (220, 224, 226)),
    "eye_gold": ((2, 2), (214, 176, 64)),
    "pupil": ((3, 2), (14, 12, 12)),
    "spine": ((4, 0), (208, 196, 168)),
    "kyber": ((4, 1), (110, 230, 255)),
    "barbel": ((4, 2), (92, 80, 62)),
    "ray_tail": ((5, 1), (86, 92, 108)),
    "blank": ((6, 0), (30, 36, 46)),
    "wrap_red": ((7, 0), (150, 24, 28)),
    "wrap_gold": ((5, 2), (206, 166, 70)),
    "chrome": ((6, 1), (206, 210, 216)),
    "dark_metal": ((7, 1), (56, 60, 66)),
    "cork_dark": ((6, 2), (178, 140, 96)),
    "eva": ((7, 2), (28, 28, 30)),
    "reel_body": ((0, 3), (42, 46, 54)),
    "reel_accent": ((1, 3), (172, 38, 34)),
    "guide_ring": ((2, 3), (110, 116, 126)),
    "reed": ((4, 3), (98, 130, 62)),
    "cattail": ((5, 0), (104, 70, 40)),
}


def uv(cell: str) -> tuple:
    (column, row), _ = COLOURS[cell]
    return ((column + 0.5) * CELL / ATLAS, (row + 0.5) * CELL / ATLAS)


def png(path: Path, width: int, height: int, rgba: bytearray) -> None:
    """Write 8-bit RGBA pixels (rows top first) as a PNG."""
    rows = b"".join(
        b"\0" + bytes(rgba[y * width * 4 : (y + 1) * width * 4]) for y in range(height)
    )

    def chunk(kind: bytes, data: bytes) -> bytes:
        return (
            struct.pack(">I", len(data))
            + kind
            + data
            + struct.pack(">I", zlib.crc32(kind + data) & 0xFFFFFFFF)
        )

    header = struct.pack(">IIBBBBB", width, height, 8, 6, 0, 0, 0)
    path.write_bytes(
        b"\x89PNG\r\n\x1a\n"
        + chunk(b"IHDR", header)
        + chunk(b"IDAT", zlib.compress(rows, 9))
        + chunk(b"IEND", b"")
    )


def atlas(output: Path) -> None:
    pixels = bytearray(ATLAS * ATLAS * 4)
    for (column, row), colour in COLOURS.values():
        for y in range(row * CELL, (row + 1) * CELL):
            for x in range(column * CELL, (column + 1) * CELL):
                i = (y * ATLAS + x) * 4
                pixels[i : i + 4] = bytes((*colour, 255))
    png(output / "fishing.png", ATLAS, ATLAS, pixels)


def encode_normal(n) -> int:
    # The inverse of sjk-model's `decode_normal` (stock's MD3 normal), as in
    # cardboard_box_assets.py.
    length = math.sqrt(sum(c * c for c in n)) or 1.0
    n = [c / length for c in n]
    around = round(math.atan2(n[1], n[0]) * 255 / (2 * math.pi)) & 0xFF
    from_up = round(math.acos(max(-1.0, min(1.0, n[2]))) * 255 / (2 * math.pi)) & 0xFF
    return (around << 8) | from_up


class Mesh:
    """Triangles with a normal and an atlas cell per vertex."""

    def __init__(self) -> None:
        self.positions, self.normals, self.uvs, self.triangles = [], [], [], []

    def vertex(self, p, n, cell: str) -> int:
        self.positions.append(tuple(p))
        self.normals.append(tuple(n))
        self.uvs.append(uv(cell))
        return len(self.positions) - 1

    def vertex_st(self, p, n, st) -> int:
        """A vertex with its own texture coordinates, for a surface with its own
        texture rather than the atlas."""
        self.positions.append(tuple(p))
        self.normals.append(tuple(n))
        self.uvs.append(tuple(st))
        return len(self.positions) - 1

    def tube(self, axis: int, start: float, end: float, r0: float, r1: float, cell: str,
             sides: int = 8, centre=(0.0, 0.0), caps: bool = True) -> None:
        """A tapered tube along `axis` (0 x, 2 z) from `start` to `end`, radius r0 to r1."""
        others = [a for a in range(3) if a != axis]

        def point(along: float, angle: float, radius: float):
            p = [0.0, 0.0, 0.0]
            p[axis] = along
            p[others[0]] = centre[0] + radius * math.cos(angle)
            p[others[1]] = centre[1] + radius * math.sin(angle)
            return p

        def normal(angle: float):
            n = [0.0, 0.0, 0.0]
            n[others[0]] = math.cos(angle)
            n[others[1]] = math.sin(angle)
            return n

        ring0, ring1 = [], []
        for i in range(sides):
            angle = 2 * math.pi * i / sides
            ring0.append(self.vertex(point(start, angle, r0), normal(angle), cell))
            ring1.append(self.vertex(point(end, angle, r1), normal(angle), cell))
        for i in range(sides):
            j = (i + 1) % sides
            self.triangles += [(ring0[i], ring1[j], ring1[i]), (ring0[i], ring0[j], ring1[j])]
        if caps:
            for along, radius, sign in ((start, r0, -1.0), (end, r1, 1.0)):
                n = [0.0, 0.0, 0.0]
                n[axis] = sign
                middle = [0.0, 0.0, 0.0]
                middle[axis] = along
                middle[others[0]], middle[others[1]] = centre
                hub = self.vertex(middle, n, cell)
                ring = [self.vertex(point(along, 2 * math.pi * i / sides, radius), n, cell)
                        for i in range(sides)]
                for i in range(sides):
                    j = (i + 1) % sides
                    self.triangles.append(
                        (hub, ring[i], ring[j]) if sign < 0 else (hub, ring[j], ring[i])
                    )

    def disc(self, z: float, radius: float, cell: str, sides: int = 12) -> None:
        hub = self.vertex((0.0, 0.0, z), (0, 0, 1), cell)
        ring = [
            self.vertex(
                (radius * math.cos(2 * math.pi * i / sides),
                 radius * math.sin(2 * math.pi * i / sides), z),
                (0, 0, 1),
                cell,
            )
            for i in range(sides)
        ]
        for i in range(sides):
            self.triangles.append((hub, ring[(i + 1) % sides], ring[i]))

    def add(self, other: "Mesh", place) -> None:
        """Append `other`, each point moved by `place` (a function of a point and its
        normal that returns both)."""
        base = len(self.positions)
        for p, n in zip(other.positions, other.normals):
            p, n = place(p, n)
            self.positions.append(tuple(p))
            self.normals.append(tuple(n))
        self.uvs += other.uvs
        self.triangles += [(base + a, base + b, base + c) for a, b, c in other.triangles]

    def write(self, output: Path, name: str) -> None:
        write_md3(output, name, [(self, SHADER)])


def write_md3(output: Path, name: str, surfaces) -> None:
    """One frame, a surface for each `(mesh, shader)`."""

    def text(value: str, size: int) -> bytes:
        return value.encode("ascii").ljust(size, b"\0")

    blocks = []
    for index, (mesh, shader) in enumerate(surfaces):
        shaders = text(shader, 64) + struct.pack("<i", 0)
        tris = b"".join(struct.pack("<3i", *t) for t in mesh.triangles)
        sts = b"".join(struct.pack("<2f", *st) for st in mesh.uvs)
        verts = b"".join(
            struct.pack("<3hH", *(round(c * 64) for c in p), encode_normal(n))
            for p, n in zip(mesh.positions, mesh.normals)
        )
        surface_header = 108
        ofs_tris = surface_header + len(shaders)
        ofs_st = ofs_tris + len(tris)
        ofs_verts = ofs_st + len(sts)
        surface_end = ofs_verts + len(verts)
        blocks.append(
            b"IDP3"
            + text(f"{name}_{index}" if index else name, 64)
            + struct.pack(
                "<10i", 0, 1, 1, len(mesh.positions), len(mesh.triangles),
                ofs_tris, surface_header, ofs_st, ofs_verts, surface_end,
            )
            + shaders + tris + sts + verts
        )
    points = [p for mesh, _ in surfaces for p in mesh.positions]
    lo = [min(p[a] for p in points) for a in range(3)]
    hi = [max(p[a] for p in points) for a in range(3)]
    middle = [(lo[a] + hi[a]) / 2 for a in range(3)]
    radius = math.sqrt(sum((hi[a] - middle[a]) ** 2 for a in range(3)))
    frame = struct.pack("<10f", *lo, *hi, *middle, radius) + text(name, 16)
    body = b"".join(blocks)
    header_size = 108
    ofs_frames = header_size
    ofs_surfaces = ofs_frames + len(frame)
    end = ofs_surfaces + len(body)
    header = (
        b"IDP3"
        + struct.pack("<i", 15)
        + text(f"models/sjk/{name}.md3", 64)
        + struct.pack(
            "<9i", 0, 1, 0, len(surfaces), 0, ofs_frames, ofs_surfaces, ofs_surfaces, end
        )
    )
    (output / f"{name}.md3").write_bytes(header + frame + body)


def torus(mesh: Mesh, centre, u, v, radius: float, thick: float, cell: str,
          segments: int = 14, sides: int = 5, start: float = 0.0,
          end: float = 2 * math.pi) -> None:
    """A ring (or an arc of one, from `start` to `end`) round `centre` in the plane of
    the unit vectors `u` and `v`, its wire `thick` in radius."""
    n = (u[1] * v[2] - u[2] * v[1], u[2] * v[0] - u[0] * v[2], u[0] * v[1] - u[1] * v[0])
    closed = end - start >= 2 * math.pi - 1e-6
    steps = segments if closed else segments + 1
    loops = []
    for i in range(steps):
        a = start + (end - start) * i / segments
        out = [u[k] * math.cos(a) + v[k] * math.sin(a) for k in range(3)]
        loop = []
        for j in range(sides):
            b = 2 * math.pi * j / sides
            normal = [out[k] * math.cos(b) + n[k] * math.sin(b) for k in range(3)]
            point = [centre[k] + out[k] * radius + normal[k] * thick for k in range(3)]
            loop.append(mesh.vertex(point, normal, cell))
        loops.append(loop)
    pairs = list(zip(loops, loops[1:])) + ([(loops[-1], loops[0])] if closed else [])
    for l0, l1 in pairs:
        for j in range(sides):
            k = (j + 1) % sides
            mesh.triangles += [(l0[j], l1[j], l1[k]), (l0[j], l1[k], l0[k])]


# The rod's blank: from the fore grip to the tip, its radius at each end.
BLANK_START = 17.6
BLANK_RADII = (0.42, 0.1)


def blank_radius(x: float) -> float:
    f = (x - BLANK_START) / (ROD_LENGTH - BLANK_START)
    return BLANK_RADII[0] + (BLANK_RADII[1] - BLANK_RADII[0]) * f ** 0.8


def rod(output: Path) -> None:
    """A spinning rod along +X from its butt (the origin) to its tip at `ROD_LENGTH`,
    the reel hanging under it (-Z): a rubber butt cap, a cork rear grip in rings, the
    reel seat with its locking nuts (held at 8, `fishing.rs`'s `GRIP`), a cork fore grip,
    then the dark blank tapering to the tip with gold-edged red thread wraps, seven line
    guides under it shrinking towards the tip-top, and the reel: its body and foot, the
    rotor with its bail, the spool wound with line, and the crank."""
    handle = Mesh()
    sides = 12
    # The butt cap, rounded.
    handle.tube(0, 0.0, 0.25, 0.7, 0.92, "eva", sides=sides)
    handle.tube(0, 0.25, 1.0, 0.92, 0.94, "eva", sides=sides, caps=False)
    # The rear grip: cork rings, a little swollen in the middle.
    x, k = 1.0, 0
    while x < 6.5 - 1e-6:
        x1 = min(6.5, x + 0.55)

        def swell(at: float) -> float:
            return 0.86 + 0.08 * math.sin(math.pi * (at - 1.0) / 5.5)

        handle.tube(0, x, x1, swell(x), swell(x1), "cork" if k % 2 else "cork_dark",
                    sides=sides, caps=False)
        x, k = x1, k + 1
    # The reel seat: a dark tube between two chrome-edged locking nuts, the reel's foot
    # under it at its middle.
    handle.tube(0, 6.5, 7.3, 0.86, 0.8, "chrome", sides=sides, caps=False)
    handle.tube(0, 7.3, 10.8, 0.62, 0.62, "dark_metal", sides=sides, caps=False)
    for x0 in (7.3, 9.9):
        handle.tube(0, x0, x0 + 0.9, 0.76, 0.76, "reel_body", sides=sides, caps=False)
        handle.tube(0, x0 + 0.05, x0 + 0.15, 0.79, 0.79, "chrome", sides=sides, caps=False)
        handle.tube(0, x0 + 0.75, x0 + 0.85, 0.79, 0.79, "chrome", sides=sides, caps=False)
    handle.tube(0, 10.8, 11.5, 0.8, 0.86, "chrome", sides=sides, caps=False)
    # The fore grip, tapering to the winding check.
    x, k = 11.5, 0
    while x < 17.0 - 1e-6:
        x1 = min(17.0, x + 0.55)
        r0 = 0.84 - 0.3 * (x - 11.5) / 5.5
        r1 = 0.84 - 0.3 * (x1 - 11.5) / 5.5
        handle.tube(0, x, x1, r0, r1, "cork" if k % 2 else "cork_dark", sides=sides,
                    caps=False)
        x, k = x1, k + 1
    handle.tube(0, 17.0, 17.6, 0.6, 0.46, "chrome", sides=sides)

    blank = Mesh()
    step = 3.0
    x = BLANK_START
    while x < ROD_LENGTH - 1e-6:
        x1 = min(ROD_LENGTH, x + step)
        blank.tube(0, x, x1, blank_radius(x), blank_radius(x1), "blank", sides=8,
                   caps=x1 >= ROD_LENGTH)
        x = x1

    def wrap(at: float, width: float, trim: bool = True) -> None:
        """A thread wrap round the blank at `at`: red, edged in gold."""
        r = blank_radius(at) + 0.04
        blank.tube(0, at - width / 2, at + width / 2, r, r, "wrap_red", sides=8, caps=False)
        if trim:
            for edge in (at - width / 2, at + width / 2 - 0.12):
                blank.tube(0, edge, edge + 0.12, r + 0.02, r + 0.02, "wrap_gold", sides=8,
                           caps=False)

    # The decorative wraps above the fore grip, and the hook keeper.
    wrap(18.4, 1.0)
    wrap(20.2, 0.3, trim=False)
    keeper = 20.2
    torus(blank, (keeper, 0.0, -blank_radius(keeper) - 0.35), (1, 0, 0), (0, 0, 1), 0.3,
          0.05, "chrome", segments=8, sides=4)

    # The guides under the blank: a two-legged frame up to its foot on the blank, and a
    # ring with a dark insert the line runs through, towards the tip ever smaller and
    # closer to the blank.
    rings = Mesh()
    guides = ((26.0, 1.55), (34.0, 1.1), (41.0, 0.82), (47.5, 0.64), (53.5, 0.52),
              (59.0, 0.44), (64.5, 0.38))
    for at, radius in guides:
        under = blank_radius(at)
        lift = radius * 1.15 + 0.35
        middle = (at, 0.0, -under - lift)
        torus(rings, middle, (0, 1, 0), (0, 0, 1), radius, 0.12 + radius * 0.06, "chrome",
              segments=12, sides=4)
        torus(rings, middle, (0, 1, 0), (0, 0, 1), radius - 0.08, 0.07, "guide_ring",
              segments=12, sides=3)
        foot = 0.9 + radius * 0.7
        for side in (1.0, -1.0):
            top = (at - foot * 0.5, 0.0, -under * 0.8)
            rim = (at, side * radius * 0.75, middle[2] + radius * 0.6)
            segment(rings, top, rim, 0.09, 0.07, "chrome", 4)
        segment(rings, (at - foot, 0.0, -under * 0.85), (at + 0.1, 0.0, -under * 0.85),
                0.08, 0.08, "chrome", 4)
        wrap(at - foot * 0.55, foot * 0.9)
    # The tip-top, the line's last ring.
    torus(rings, (ROD_LENGTH - 0.25, 0.0, -0.32), (0, 1, 0), (0, 0, 1), 0.24, 0.06,
          "chrome", segments=10, sides=4)
    segment(rings, (ROD_LENGTH - 1.0, 0.0, 0.0), (ROD_LENGTH, 0.0, 0.0), 0.14, 0.12,
            "chrome", 6)
    wrap(ROD_LENGTH - 1.3, 0.5, trim=False)

    reel = Mesh()
    # The foot on the seat and the stem down to the body.
    stem = 9.1
    segment(reel, (stem - 1.2, 0.0, -0.66), (stem + 1.2, 0.0, -0.66), 0.14, 0.14,
            "reel_body", 4)
    segment(reel, (stem, 0.0, -0.7), (stem - 0.3, 0.0, -2.9), 0.34, 0.28, "reel_body", 6)
    body_z = -3.9
    # The gearbox: a rounded body along the rod, its side covers chrome-rimmed.
    reel.tube(0, 6.2, 7.0, 0.5, 1.25, "reel_body", sides=12, centre=(0.0, body_z))
    reel.tube(0, 7.0, 10.6, 1.25, 1.35, "reel_body", sides=12, caps=False,
              centre=(0.0, body_z))
    for side in (1.0, -1.0):
        reel.tube(1, side * 1.15, side * 1.4, 1.05, 1.0, "reel_accent", sides=12,
                  centre=(8.6, body_z))
        torus(reel, (8.6, side * 1.4, body_z), (1, 0, 0), (0, 0, 1), 1.0, 0.07, "chrome",
              segments=14, sides=4)
    # The rotor and the spool, wound with line, ahead of the body.
    reel.tube(0, 10.6, 11.0, 1.35, 1.0, "chrome", sides=12, caps=False, centre=(0.0, body_z))
    reel.tube(0, 11.0, 12.6, 1.0, 1.55, "reel_body", sides=12, caps=False,
              centre=(0.0, body_z))
    reel.tube(0, 12.6, 12.9, 1.55, 1.55, "reel_accent", sides=12, caps=False,
              centre=(0.0, body_z))
    reel.tube(0, 12.9, 13.2, 1.45, 1.45, "chrome", sides=12, centre=(0.0, body_z))
    reel.tube(0, 13.2, 15.0, 1.25, 1.25, "line", sides=12, caps=False, centre=(0.0, body_z))
    reel.tube(0, 15.0, 15.3, 1.42, 1.42, "chrome", sides=12, centre=(0.0, body_z))
    reel.tube(0, 15.3, 15.9, 0.6, 0.35, "reel_accent", sides=10, centre=(0.0, body_z))
    # The bail: two arms from the rotor forward, and the wire hoop round the spool's
    # front between them, over the top.
    for side in (1.0, -1.0):
        segment(reel, (12.4, side * 1.5, body_z), (15.6, side * 1.75, body_z), 0.1, 0.09,
                "chrome", 4)
    torus(reel, (15.6, 0.0, body_z), (0, 1, 0), (0, 0, 1), 1.75, 0.08, "chrome",
          segments=16, sides=4, start=0.0, end=math.pi)
    # The crank on the left: an axle out of the side cover, the arm and its knob.
    axle = (8.6, -1.4, body_z)
    out = (8.6, -2.6, body_z)
    segment(reel, axle, out, 0.18, 0.16, "chrome", 6)
    elbow = (8.2, -2.7, body_z - 2.6)
    segment(reel, out, elbow, 0.16, 0.13, "reel_body", 6)
    reel.tube(1, -2.75, -3.8, 0.38, 0.42, "eva", sides=8, centre=(elbow[0], elbow[2]))
    # The reel drawn at three quarters of the size above, from its foot on the seat.
    foot = (stem, 0.0, -0.62)
    small = Mesh()
    small.add(reel, lambda p, n: ([foot[k] + (p[k] - foot[k]) * 0.75 for k in range(3)], n))
    write_md3(output, "fishing_rod", [(handle, SHADER), (blank, SHADER), (rings, SHADER),
                                         (small, SHADER)])


def bucket(output: Path) -> None:
    mesh = Mesh()
    mesh.tube(2, 0.0, 11.0, 6.0, 7.2, "bucket", sides=14, caps=False)
    # Its inside, facing in, and its bottom.
    inner = Mesh()
    inner.tube(2, 0.2, 10.8, 5.8, 7.0, "rim", sides=14, caps=False)
    base = len(mesh.positions)
    mesh.positions += inner.positions
    mesh.normals += [(-n[0], -n[1], -n[2]) for n in inner.normals]
    mesh.uvs += inner.uvs
    mesh.triangles += [(base + a, base + c, base + b) for a, b, c in inner.triangles]
    mesh.disc(0.3, 6.0, "rim", sides=14)
    mesh.tube(2, 10.6, 11.4, 7.5, 7.5, "rim", sides=14, caps=False)
    mesh.disc(8.0, 6.7, "water", sides=14)
    # The handle: a half hoop over the top, from one side to the other.
    steps = 8
    for i in range(steps):
        a0 = math.pi * i / steps
        a1 = math.pi * (i + 1) / steps
        p0 = (7.3 * math.cos(a0), 0.0, 11.0 + 6.0 * math.sin(a0))
        p1 = (7.3 * math.cos(a1), 0.0, 11.0 + 6.0 * math.sin(a1))
        segment = Mesh()
        length = math.dist(p0, p1)
        segment.tube(0, 0.0, length, 0.3, 0.3, "rim", sides=4, caps=False)
        angle = math.atan2(p1[2] - p0[2], p1[0] - p0[0])
        c, s = math.cos(angle), math.sin(angle)
        base = len(mesh.positions)
        for p, n in zip(segment.positions, segment.normals):
            mesh.positions.append((p0[0] + p[0] * c - p[2] * s, p[1], p0[2] + p[0] * s + p[2] * c))
            mesh.normals.append((n[0] * c - n[2] * s, n[1], n[0] * s + n[2] * c))
        mesh.uvs += segment.uvs
        mesh.triangles += [(base + a, base + b, base + c) for a, b, c in segment.triangles]
    mesh.write(output, "fishing_bucket")


def float_model(output: Path) -> None:
    mesh = Mesh()
    mesh.tube(2, -1.6, 0.0, 0.3, 1.2, "float_white", sides=10)
    mesh.tube(2, 0.0, 1.8, 1.2, 0.3, "float_red", sides=10)
    mesh.tube(2, 1.8, 3.2, 0.15, 0.15, "float_red", sides=4)
    mesh.write(output, "fishing_float")


def line(output: Path) -> None:
    mesh = Mesh()
    mesh.tube(0, 0.0, LINE_LENGTH, 0.1, 0.1, "line", sides=4, caps=False)
    mesh.write(output, "fishing_line")


FISH_SHADER = "models/sjk/fishing_fish"
FISH_ATLAS = 512
FISH_TILE = 128
# A fish's tile: its skin in the top rows (along the body left to right, from the back
# at the top to the belly at the bottom, both flanks alike), its fins in the rest (along
# a fin's base left to right, from the base at the top to the edge at the bottom).
SKIN_ROWS = 96
TILE_PAD = 3

# The twelve fish of the catch table (`fishing.rs`'s `CATCHES`), each its own shape and
# skin. Lengths are game units from the mouth (the model's origin) back to the tail's
# tip; `height` and `width` are the body's deepest and widest; `peak` is where along the
# body (0 the mouth, 1 the root of the tail) it is deepest; `tail` the share of the length
# the tail fin takes. Colours: `back`, `side`, `belly` and `fin`. The rest picks the fins,
# the extras and the pattern painted on the skin.
FISH = [
    dict(id="silver_darter", length=9.0, height=2.1, width=1.1, peak=0.34, blunt=0.75,
         tail=0.24, peduncle=0.16, tail_shape="fork", tail_height=1.15,
         dorsal=[(0.32, 0.55, 0.75)], anal=(0.66, 0.84, 0.4), pectoral=0.9, pelvic=0.5,
         eye=0.2, back=(46, 66, 88), side=(190, 200, 210), belly=(238, 240, 236),
         fin=(176, 186, 194), pattern="lateral", wave=0.25),
    dict(id="mud_snapper", length=11.0, height=3.9, width=1.7, peak=0.38, blunt=0.6,
         tail=0.2, peduncle=0.22, tail_shape="square", tail_height=0.9,
         dorsal=[(0.24, 0.5, 0.5), (0.5, 0.72, 0.42)], anal=(0.64, 0.82, 0.45),
         pectoral=1.2, pelvic=0.8, eye=0.18, spines=(0.25, 0.5, 0.75), gape=0.32,
         back=(70, 56, 36), side=(132, 104, 64), belly=(206, 184, 136), fin=(150, 78, 46),
         pattern="mottled"),
    dict(id="swamp_gulper", length=12.0, height=4.0, width=3.4, peak=0.26, blunt=0.45,
         width_peak=0.2, mouth=0.6, tail=0.18, peduncle=0.2, tail_shape="round",
         tail_height=0.75, dorsal=[(0.45, 0.7, 0.35)], anal=(0.62, 0.8, 0.3),
         pectoral=1.3, pelvic=0.7, eye=0.16, eye_up=0.6, gape=0.55,
         back=(44, 64, 40), side=(96, 114, 60), belly=(200, 196, 144), fin=(82, 98, 54),
         pattern="spots"),
    dict(id="striped_bonefish", length=13.0, height=2.9, width=1.5, peak=0.36,
         blunt=0.8, tail=0.24, peduncle=0.14, tail_shape="fork", tail_height=1.25,
         dorsal=[(0.3, 0.5, 0.8)], anal=(0.7, 0.82, 0.45), pectoral=1.1, pelvic=0.6,
         eye=0.2, back=(98, 110, 118), side=(212, 214, 206), belly=(242, 240, 230),
         fin=(222, 198, 112), pattern="bars", wave=0.3),
    dict(id="glow_perch", length=10.0, height=3.3, width=1.6, peak=0.38, blunt=0.65,
         tail=0.2, peduncle=0.2, tail_shape="square", tail_height=0.95,
         dorsal=[(0.2, 0.46, 0.7), (0.48, 0.7, 0.5)], anal=(0.64, 0.8, 0.45),
         pectoral=1.0, pelvic=0.7, eye=0.2, spines=(0.21, 0.45, 0.65),
         back=(40, 82, 58), side=(214, 150, 46), belly=(242, 214, 150), fin=(214, 70, 40),
         pattern="glow"),
    dict(id="spiny_pike", length=20.0, height=2.8, width=1.9, peak=0.55, blunt=1.25,
         mouth=0.25, tail=0.17, peduncle=0.26, tail_shape="fork", tail_height=1.0,
         dorsal=[(0.74, 0.9, 0.6)], anal=(0.76, 0.92, 0.5), pectoral=1.1, pelvic=0.8,
         eye=0.15, eye_up=0.55, gape=0.2, crest=(0.18, 0.68, 11),
         back=(38, 60, 36), side=(98, 124, 66), belly=(228, 224, 192), fin=(124, 112, 60),
         pattern="pike", wave=0.6),
    dict(id="sun_carp", length=14.0, height=5.0, width=2.6, peak=0.42, blunt=0.6,
         mouth=0.2, tail=0.2, peduncle=0.22, tail_shape="fork", tail_height=1.05,
         dorsal=[(0.3, 0.74, 0.42)], anal=(0.7, 0.84, 0.4), pectoral=1.3, pelvic=0.9,
         eye=0.17, barbels=[(2.4, 0.15, -0.4)], back=(150, 94, 20), side=(232, 168, 38),
         belly=(246, 222, 140), fin=(236, 116, 38), pattern="scales"),
    dict(id="moon_eel", length=26.0, height=2.6, width=1.9, peak=0.3, blunt=0.9,
         mouth=0.35, tail=0.04, peduncle=0.05, tail_shape="round", tail_height=0.6,
         rings=34, dorsal=[(0.24, 1.0, 0.36)], anal=(0.5, 1.0, 0.32), pectoral=1.0,
         eye=0.13, eye_up=0.45, back=(40, 50, 92), side=(86, 100, 146), belly=(184, 190, 210),
         fin=(70, 84, 132), pattern="moons", wave=2.2),
    dict(id="kyber_minnow", length=6.0, height=1.5, width=0.9, peak=0.36, blunt=0.75,
         tail=0.26, peduncle=0.15, tail_shape="fork", tail_height=1.2,
         dorsal=[(0.42, 0.6, 0.7)], anal=(0.62, 0.78, 0.45), pectoral=0.7, pelvic=0.4,
         eye=0.28, crystals=5, back=(26, 84, 136), side=(110, 188, 230),
         belly=(222, 240, 250), fin=(160, 214, 238), pattern="kyber", wave=0.15),
    dict(id="ghost_ray", length=17.0, ray=True, height=0.9, width=12.0,
         back=(188, 198, 212), side=(150, 160, 182), belly=(244, 246, 250), fin=(96, 102, 118),
         pattern="ghost"),
    dict(id="temple_koi", length=15.0, height=4.0, width=2.2, peak=0.4, blunt=0.65,
         mouth=0.22, tail=0.3, peduncle=0.18, tail_shape="flowing", tail_height=1.7,
         dorsal=[(0.3, 0.7, 0.5)], anal=(0.7, 0.82, 0.5), pectoral=2.0, pelvic=1.2,
         eye=0.17, barbels=[(1.6, 0.12, -0.3)], back=(244, 242, 234), side=(246, 244, 238),
         belly=(250, 248, 244), fin=(240, 232, 222), pattern="koi", wave=0.5),
    dict(id="old_whiskers", length=24.0, height=5.0, width=6.0, peak=0.3, blunt=0.5,
         width_peak=0.12, mouth=0.55, tail=0.16, peduncle=0.2, tail_shape="fork",
         tail_height=0.8, dorsal=[(0.28, 0.4, 0.55)], adipose=(0.66, 0.8), anal=(0.55, 0.86, 0.3),
         pectoral=2.0, pelvic=1.2, eye=0.07, eye_up=0.55, gape=0.5,
         barbels=[(14.0, 0.38, 0.55), (5.0, 0.2, -0.65), (4.0, 0.18, -0.85)],
         back=(62, 58, 50), side=(106, 96, 80), belly=(204, 194, 172), fin=(66, 60, 52),
         pattern="catfish", wave=0.8),
]


def fish_model(id_: str) -> str:
    return f"fishing_fish_{id_}"


def tile_of(index: int) -> tuple:
    per_row = FISH_ATLAS // FISH_TILE
    return (index % per_row) * FISH_TILE, (index // per_row) * FISH_TILE


def skin_st(index: int, s: float, t: float) -> tuple:
    """Where a point of a fish's skin (`s` along it, `t` from back to belly, each 0 to 1)
    is in the fish atlas."""
    x, y = tile_of(index)
    span = FISH_TILE - 2 * TILE_PAD
    return ((x + TILE_PAD + s * span) / FISH_ATLAS,
            (y + TILE_PAD + t * (SKIN_ROWS - 2 * TILE_PAD)) / FISH_ATLAS)


def fin_st(index: int, s: float, t: float) -> tuple:
    """Where a point of a fish's fins (`s` along the base, `t` out to the edge) is."""
    x, y = tile_of(index)
    span = FISH_TILE - 2 * TILE_PAD
    return ((x + TILE_PAD + s * span) / FISH_ATLAS,
            (y + SKIN_ROWS + TILE_PAD + t * (FISH_TILE - SKIN_ROWS - 2 * TILE_PAD))
            / FISH_ATLAS)


def noise(x: float, y: float, seed: int) -> float:
    """Smooth value noise, 0 to 1, deterministic."""

    def corner(i: int, j: int) -> float:
        h = (i * 374761393 + j * 668265263 + seed * 2147483647) & 0xFFFFFFFF
        h = ((h ^ (h >> 13)) * 1274126177) & 0xFFFFFFFF
        return ((h ^ (h >> 16)) & 0xFFFF) / 0xFFFF

    i, j = math.floor(x), math.floor(y)
    fx, fy = x - i, y - j
    fx, fy = fx * fx * (3 - 2 * fx), fy * fy * (3 - 2 * fy)
    top = corner(i, j) + (corner(i + 1, j) - corner(i, j)) * fx
    bottom = corner(i, j + 1) + (corner(i + 1, j + 1) - corner(i, j + 1)) * fx
    return top + (bottom - top) * fy


def spots(x: float, y: float, seed: int) -> float:
    """The distance (in cells) from `(x, y)` to the nearest of points scattered one to a
    cell: round, organic spots where it is small."""
    i, j = math.floor(x), math.floor(y)
    best = 9.0
    for di in (-1, 0, 1):
        for dj in (-1, 0, 1):
            ci, cj = i + di, j + dj
            px = ci + 0.15 + 0.7 * noise(ci * 3.1 + 0.5, cj * 1.7 + 0.5, seed)
            py = cj + 0.15 + 0.7 * noise(ci * 2.3 + 0.5, cj * 4.1 + 0.5, seed + 11)
            best = min(best, math.hypot(x - px, y - py))
    return best


def mix(a, b, f: float) -> tuple:
    f = max(0.0, min(1.0, f))
    return tuple(a[k] + (b[k] - a[k]) * f for k in range(3))


def shade(c, f: float) -> tuple:
    return tuple(c[k] * f for k in range(3))


def smooth(edge0: float, edge1: float, x: float) -> float:
    f = max(0.0, min(1.0, (x - edge0) / (edge1 - edge0)))
    return f * f * (3 - 2 * f)


def paint_skin(spec: dict, index: int, s: float, t: float) -> tuple:
    """A fish's skin at `s` along it (0 the mouth) and `t` round it (0 the back, 0.5 the
    flank, 1 the belly)."""
    back, side, belly = spec["back"], spec["side"], spec["belly"]
    # Shaded dark over, light under, as fish are.
    colour = mix(back, side, smooth(0.05, 0.42, t))
    colour = mix(colour, belly, smooth(0.55, 0.85, t))
    pattern = spec["pattern"]
    seed = index * 7 + 3
    if pattern == "lateral":
        colour = mix(colour, (236, 240, 244), 1.0 - smooth(0.0, 0.035, abs(t - 0.46)))
        colour = mix(colour, (40, 56, 72), 0.7 * (1.0 - smooth(0.0, 0.015, abs(t - 0.41))))
        colour = mix(colour, (30, 40, 52), 0.5 * (1.0 - smooth(0.0, 0.05, abs(s - 0.2) + abs(t - 0.36))))
    elif pattern == "mottled":
        blot = spots(s * 7, t * 5, seed) + 0.25 * noise(s * 30, t * 20, seed + 1)
        colour = shade(colour, 1.0 - 0.4 * (1 - smooth(0.3, 0.45, blot)) * (1 - smooth(0.6, 0.9, t)))
    elif pattern == "spots":
        d = spots(s * 11, t * 7, seed)
        colour = shade(colour, 1.0 - 0.6 * (1 - smooth(0.22, 0.3, d)) * (1 - smooth(0.55, 0.8, t)))
        colour = mix(colour, (176, 186, 96), 0.25 * noise(s * 5, t * 4, seed + 2))
    elif pattern == "bars":
        bar = 0.5 + 0.5 * math.cos(2 * math.pi * (s * 7.0 - 0.2))
        bar *= smooth(0.15, 0.9, s * 1.4) * (1.0 - smooth(0.5, 0.72, t))
        colour = mix(colour, (52, 58, 66), 0.75 * smooth(0.55, 0.75, bar))
    elif pattern == "glow":
        band = 0.5 + 0.5 * math.cos(2 * math.pi * (s * 6.0 - 0.1))
        band *= smooth(0.12, 0.3, s) * (1.0 - smooth(0.45, 0.7, t))
        colour = mix(colour, (34, 52, 30), 0.7 * smooth(0.6, 0.8, band))
        for k in range(7):
            cs, ct = 0.2 + 0.11 * k, 0.48 + 0.04 * math.sin(k * 2.0)
            d = math.hypot((s - cs) * 1.6, t - ct)
            colour = mix(colour, (120, 255, 230), 1.0 - smooth(0.012, 0.03, d))
    elif pattern == "pike":
        cell_s, cell_t = s * 22, t * 9 + 0.5 * math.floor(s * 22)
        fs, ft = cell_s - math.floor(cell_s) - 0.5, cell_t - math.floor(cell_t) - 0.5
        bean = math.hypot(fs * 1.4, ft * 2.4)
        colour = mix(colour, (214, 212, 150),
                     0.85 * (1 - smooth(0.28, 0.36, bean)) * smooth(0.15, 0.3, t)
                     * (1 - smooth(0.62, 0.72, t)) * smooth(0.12, 0.2, s))
    elif pattern == "scales":
        pass
    elif pattern == "moons":
        for k in range(14):
            cs = 0.06 + 0.067 * k
            ct = 0.2 + 0.24 * noise(k * 1.7, 0.5, seed)
            d0 = math.hypot((s - cs) * 3.0, t - ct)
            d1 = math.hypot((s - cs - 0.01) * 3.0, t - ct + 0.025)
            crescent = (1 - smooth(0.05, 0.07, d0)) * smooth(0.04, 0.06, d1)
            colour = mix(colour, (226, 232, 246), crescent)
    elif pattern == "kyber":
        glow = 1.0 - smooth(0.0, 0.05, abs(t - 0.45))
        colour = mix(colour, (140, 240, 255), glow)
        colour = mix(colour, (255, 255, 255), 0.6 * (1.0 - smooth(0.0, 0.015, abs(t - 0.45))))
    elif pattern == "ghost":
        d = spots(s * 9, t * 10, seed)
        colour = mix(colour, (236, 242, 250), 0.7 * (1 - smooth(0.16, 0.24, d)) * (1 - smooth(0.4, 0.5, t)))
        colour = mix(colour, (120, 136, 170), smooth(0.38, 0.5, t) * (1 - smooth(0.5, 0.6, t)))
    elif pattern == "koi":
        patch = noise(s * 4.5, t * 3.0, seed) * 0.7 + noise(s * 11, t * 7, seed + 1) * 0.3
        red = smooth(0.48, 0.53, patch) * (1 - smooth(0.42, 0.6, t)) * smooth(0.06, 0.12, s)
        colour = mix(colour, (228, 74, 28), red)
        cap = 1 - smooth(0.06, 0.1, math.hypot((s - 0.08) * 1.2, t * 0.6))
        colour = mix(colour, (228, 74, 28), cap)
        ink = smooth(0.8, 0.84, noise(s * 13, t * 9, seed + 5)) * (1 - smooth(0.4, 0.5, t))
        colour = mix(colour, (30, 28, 30), ink * smooth(0.3, 0.4, s))
    elif pattern == "catfish":
        d = spots(s * 16, t * 9, seed) + 0.3 * noise(s * 40, t * 25, seed + 1)
        colour = shade(colour, 1.0 - 0.35 * (1 - smooth(0.3, 0.5, d)) * (1 - smooth(0.5, 0.75, t)))
    if not spec.get("ray"):
        # Scales: rows of overlapping arcs, their edges darker; the head (s < 0.2) has none.
        rows, columns = (9, 26) if pattern != "scales" else (7, 16)
        sx, ty = s * columns, t * rows + 0.5 * math.floor(s * columns)
        fx, fy = sx - math.floor(sx), ty - math.floor(ty) - 0.5
        edge = smooth(0.32, 0.5, math.hypot(fx * 0.9, fy))
        depth = 0.22 if pattern == "scales" else (0.06 if pattern in ("moons", "catfish") else 0.11)
        colour = shade(colour, 1.0 - depth * edge * smooth(0.18, 0.24, s))
        if pattern == "scales":
            colour = mix(colour, (255, 236, 150), 0.35 * (1 - edge) * smooth(0.2, 0.5, t)
                         * (1 - smooth(0.6, 0.8, t)) * smooth(0.18, 0.24, s))
        # The gill cover's edge, behind the head.
        gill = 0.19 + 0.04 * math.cos(math.pi * (t - 0.5))
        colour = shade(colour, 1.0 - 0.3 * (1 - smooth(0.0, 0.012, abs(s - gill)))
                       * (1 - smooth(0.85, 0.95, t)))
    return colour


def paint_fin(spec: dict, s: float, t: float) -> tuple:
    """A fish's fins at `s` along a fin's base and `t` out to its edge: their colour,
    darker rays and a paler edge (the koi's streaked orange, the ray's tail plain)."""
    colour = spec["fin"]
    if spec["pattern"] == "koi":
        colour = mix(colour, (236, 120, 50), 0.4 * smooth(0.4, 0.6, noise(s * 6, t * 2, 4)))
    elif spec["pattern"] == "pike":
        dot = noise(s * 18, t * 6, 9)
        colour = shade(colour, 1.0 - 0.45 * smooth(0.65, 0.72, dot))
    elif spec["pattern"] == "glow":
        colour = mix(colour, (255, 190, 60), smooth(0.7, 0.95, t))
    ray = 0.5 + 0.5 * math.cos(2 * math.pi * s * 14)
    colour = shade(colour, 1.0 - 0.18 * smooth(0.6, 0.95, ray) * smooth(0.0, 0.2, t))
    colour = mix(colour, (250, 250, 250), 0.25 * smooth(0.75, 1.0, t))
    return shade(colour, 0.85 + 0.15 * t)


def fish_atlas(output: Path) -> None:
    """The fish atlas, 512x512: a 128x128 tile a fish, its skin above and its fins
    below."""
    pixels = bytearray(FISH_ATLAS * FISH_ATLAS * 4)
    span = FISH_TILE - 2 * TILE_PAD
    for index, spec in enumerate(FISH):
        x0, y0 = tile_of(index)
        for y in range(FISH_TILE):
            for x in range(FISH_TILE):
                s = max(0.0, min(1.0, (x - TILE_PAD + 0.5) / span))
                if y < SKIN_ROWS:
                    t = max(0.0, min(1.0, (y - TILE_PAD + 0.5) / (SKIN_ROWS - 2 * TILE_PAD)))
                    colour = paint_skin(spec, index, s, t)
                else:
                    rows = FISH_TILE - SKIN_ROWS - 2 * TILE_PAD
                    t = max(0.0, min(1.0, (y - SKIN_ROWS - TILE_PAD + 0.5) / rows))
                    colour = paint_fin(spec, s, t)
                i = ((y0 + y) * FISH_ATLAS + x0 + x) * 4
                pixels[i : i + 4] = bytes(
                    (*(max(0, min(255, round(c))) for c in colour), 255)
                )
    png(output / "fishing_fish.png", FISH_ATLAS, FISH_ATLAS, pixels)


def smooth_normals(mesh: Mesh, outward) -> None:
    """Each vertex's normal from the faces round it, turned to face `outward(p)`."""
    sums = [[0.0, 0.0, 0.0] for _ in mesh.positions]
    for a, b, c in mesh.triangles:
        pa, pb, pc = mesh.positions[a], mesh.positions[b], mesh.positions[c]
        u = [pb[k] - pa[k] for k in range(3)]
        v = [pc[k] - pa[k] for k in range(3)]
        n = (u[1] * v[2] - u[2] * v[1], u[2] * v[0] - u[0] * v[2], u[0] * v[1] - u[1] * v[0])
        ref = outward(pa)
        if sum(n[k] * ref[k] for k in range(3)) < 0:
            n = tuple(-c for c in n)
        for i in (a, b, c):
            for k in range(3):
                sums[i][k] += n[k]
    mesh.normals = [tuple(n) if any(n) else (0.0, 0.0, 1.0) for n in sums]


def fin_strip(mesh: Mesh, index: int, base, edge, normal) -> None:
    """A fin between the points of its `base` and of its `edge` (as many), its fin skin
    across it, its normal `normal` (both sides are drawn)."""
    count = len(base)
    ids = []
    for i, (p, q) in enumerate(zip(base, edge)):
        s = i / (count - 1)
        ids.append((mesh.vertex_st(p, normal, fin_st(index, s, 0.0)),
                    mesh.vertex_st(q, normal, fin_st(index, s, 1.0))))
    for (a0, a1), (b0, b1) in zip(ids, ids[1:]):
        mesh.triangles += [(a0, b0, b1), (a0, b1, a1)]


def segment(mesh: Mesh, p0, p1, r0: float, r1: float, cell: str, sides: int = 5) -> None:
    """A tapered tube from `p0` to `p1` in the flat atlas."""
    axis = [p1[k] - p0[k] for k in range(3)]
    length = math.sqrt(sum(c * c for c in axis)) or 1.0
    axis = [c / length for c in axis]
    helper = (0.0, 0.0, 1.0) if abs(axis[2]) < 0.9 else (1.0, 0.0, 0.0)
    u = (axis[1] * helper[2] - axis[2] * helper[1], axis[2] * helper[0] - axis[0] * helper[2],
         axis[0] * helper[1] - axis[1] * helper[0])
    ul = math.sqrt(sum(c * c for c in u))
    u = [c / ul for c in u]
    v = (axis[1] * u[2] - axis[2] * u[1], axis[2] * u[0] - axis[0] * u[2],
         axis[0] * u[1] - axis[1] * u[0])
    rings = []
    for centre, radius in ((p0, r0), (p1, r1)):
        ring = []
        for i in range(sides):
            a = 2 * math.pi * i / sides
            n = [u[k] * math.cos(a) + v[k] * math.sin(a) for k in range(3)]
            ring.append(mesh.vertex([centre[k] + n[k] * radius for k in range(3)], n, cell))
        rings.append(ring)
    for i in range(sides):
        j = (i + 1) % sides
        mesh.triangles += [(rings[0][i], rings[1][i], rings[1][j]),
                           (rings[0][i], rings[1][j], rings[0][j])]


def strand(mesh: Mesh, points, r0: float, r1: float, cell: str) -> None:
    """A thin tube through `points`, tapering from `r0` to `r1`."""
    last = len(points) - 1
    for i in range(last):
        segment(mesh, points[i], points[i + 1], r0 + (r1 - r0) * i / last,
                r0 + (r1 - r0) * (i + 1) / last, cell)


def pw(x: float, e: float) -> float:
    """`x` to the power `e`, `x` held at 0 or above (a sine at its end may dip below)."""
    return max(0.0, x) ** e


def profile(u: float, peak: float, blunt: float, peduncle: float, mouth: float) -> float:
    """The body's depth (0 to 1) at `u` along it: rising from the mouth to its deepest
    at `peak`, then narrowing to the root of the tail."""
    if u < peak:
        f = pw(math.sin(0.5 * math.pi * u / peak), blunt)
        return mouth + (1.0 - mouth) * f
    f = pw(math.cos(0.5 * math.pi * (u - peak) / (1.0 - peak)), 1.3)
    return peduncle + (1.0 - peduncle) * f


def tail_reach(shape: str, v: float) -> float:
    """How far back the tail fin's edge reaches (0 to 1) at `v` up it (-1 to 1)."""
    a = abs(v)
    if shape == "fork":
        return 0.45 + 0.55 * a ** 1.2
    if shape == "round":
        return math.sqrt(max(0.0, 1.0 - 0.75 * v * v))
    if shape == "flowing":
        return 0.7 + 0.3 * math.sin(math.pi * (0.5 + 0.5 * v)) - 0.15 * a
    return 0.88 + 0.12 * a


def fish_body(spec: dict, index: int) -> tuple:
    """A fish along -X from its mouth at the origin: its skin and fins (the fish atlas)
    and its eyes, spines, crystals and whiskers (the flat atlas)."""
    length = spec["length"]
    body_length = length * (1.0 - spec["tail"])
    half_h, half_w = spec["height"] / 2, spec["width"] / 2
    peak, blunt = spec["peak"], spec["blunt"]
    peduncle, mouth = spec["peduncle"], spec.get("mouth", 0.12)
    width_peak = spec.get("width_peak", peak)
    rings, sides = spec.get("rings", 18), 14

    def depth(u: float) -> float:
        return half_h * profile(u, peak, blunt, peduncle, mouth)

    def breadth(u: float) -> float:
        return half_w * profile(u, width_peak, blunt, peduncle * 0.8, mouth * 0.9)

    def centre(u: float) -> float:
        # The back arches more than the belly sags.
        return 0.12 * half_h * math.sin(math.pi * min(1.0, u / max(peak * 1.6, 0.01))) * (1 - u)

    skin = Mesh()
    grid = []
    for k in range(rings):
        u = (k / (rings - 1)) ** 1.15
        x = -u * body_length
        ring = []
        for j in range(sides):
            a = 2 * math.pi * j / sides
            y = breadth(u) * math.sin(a)
            z = centre(u) + depth(u) * math.cos(a)
            t = math.acos(max(-1.0, min(1.0, math.cos(a)))) / math.pi
            ring.append(skin.vertex_st((x, y, z), (0, 0, 1), skin_st(index, u, t)))
        grid.append(ring)
    for r0, r1 in zip(grid, grid[1:]):
        for j in range(sides):
            n = (j + 1) % sides
            skin.triangles += [(r0[j], r1[j], r1[n]), (r0[j], r1[n], r0[n])]
    nose = skin.vertex_st((0.25 * depth(0.0), 0.0, centre(0.0)), (1, 0, 0),
                          skin_st(index, 0.0, 0.5))
    for j in range(sides):
        skin.triangles.append((nose, grid[0][(j + 1) % sides], grid[0][j]))
    root = skin.vertex_st((-body_length - 0.05, 0.0, centre(1.0)), (-1, 0, 0),
                          skin_st(index, 1.0, 0.5))
    for j in range(sides):
        skin.triangles.append((root, grid[-1][j], grid[-1][(j + 1) % sides]))

    def outward(p):
        u = max(0.0, min(1.0, -p[0] / body_length))
        return (0.0 if -body_length < p[0] < 0 else p[0], p[1], p[2] - centre(u))

    smooth_normals(skin, outward)

    fins = Mesh()
    steps = 8
    for u0, u1, high in spec.get("dorsal", []):
        base, edge = [], []
        for i in range(steps):
            v = i / (steps - 1)
            u = u0 + (u1 - u0) * v
            top = centre(u) + depth(u) * 0.92
            h = high * spec["height"] * pw(math.sin(math.pi * min(1.0, 0.1 + v)), 0.6) * (1.25 - 0.5 * v)
            if u1 >= 1.0:
                h = high * spec["height"] * (0.6 + 0.4 * math.sin(math.pi * v * 0.5))
            base.append((-u * body_length, 0.0, top))
            edge.append((-u * body_length - 0.45 * h, 0.0, top + h))
        fin_strip(fins, index, base, edge, (0, 1, 0))
    if "anal" in spec:
        u0, u1, high = spec["anal"]
        base, edge = [], []
        for i in range(steps):
            v = i / (steps - 1)
            u = u0 + (u1 - u0) * v
            bottom = centre(u) - depth(u) * 0.92
            h = high * spec["height"] * pw(math.sin(math.pi * min(1.0, 0.1 + v)), 0.6)
            if u1 >= 1.0:
                h = high * spec["height"] * (0.6 + 0.4 * math.sin(math.pi * v * 0.5))
            base.append((-u * body_length, 0.0, bottom))
            edge.append((-u * body_length - 0.4 * h, 0.0, bottom - h))
        fin_strip(fins, index, base, edge, (0, 1, 0))
    if "adipose" in spec:
        u0, u1 = spec["adipose"]
        base, edge = [], []
        for i in range(steps):
            v = i / (steps - 1)
            u = u0 + (u1 - u0) * v
            top = centre(u) + depth(u) * 0.9
            h = 0.12 * spec["height"] * pw(math.sin(math.pi * v), 0.7)
            base.append((-u * body_length, 0.0, top))
            edge.append((-u * body_length - 0.3 * h, 0.0, top + h))
        fin_strip(fins, index, base, edge, (0, 1, 0))
    # The tail fin from the root of the tail, as deep as `tail_height` of the body.
    tail_steps = 11
    tail_length = length - body_length
    tail_half = spec["tail_height"] * half_h
    root_half = depth(1.0)
    base, edge = [], []
    for i in range(tail_steps):
        v = -1.0 + 2.0 * i / (tail_steps - 1)
        base.append((-body_length + 0.15, 0.0, centre(1.0) + v * root_half * 0.9))
        edge.append((-body_length - tail_length * tail_reach(spec["tail_shape"], v), 0.0,
                     centre(1.0) + v * tail_half))
    fin_strip(fins, index, base, edge, (0, 1, 0))
    # Pectoral fins behind the gills, low on each flank, swept back and out; pelvic fins
    # under the belly.
    for size, u, low, spread in ((spec.get("pectoral", 0.0), 0.24, -0.35, 0.55),
                                 (spec.get("pelvic", 0.0), 0.46, -0.85, 0.35)):
        if not size:
            continue
        for sign in (1.0, -1.0):
            base, edge = [], []
            for i in range(5):
                v = i / 4
                bu = u + 0.05 * v
                bz = centre(bu) + depth(bu) * (low + 0.15 * (1 - v))
                by = sign * breadth(bu) * math.sqrt(max(0.0, 1.0 - (low + 0.15 * (1 - v)) ** 2)) * 0.95
                reach = 1.6 * size * (0.55 + 0.45 * math.sin(math.pi * (0.25 + 0.6 * v)))
                base.append((-bu * body_length, by, bz))
                edge.append((-bu * body_length - reach * 0.85, by + sign * reach * spread,
                             bz - reach * 0.3))
            fin_strip(fins, index, base, edge, (0, sign, 0.3))

    extras = Mesh()
    # The eyes: a gold ring round a black pupil, on each side of the head.
    eye_u = spec.get("eye_u", 0.11)
    eye_r = spec["eye"] * spec["height"] * 0.5 + 0.12
    ez = centre(eye_u) + depth(eye_u) * spec.get("eye_up", 0.3)
    ey = breadth(eye_u) * math.sqrt(max(0.0, 1.0 - spec.get("eye_up", 0.3) ** 2))
    for sign in (1.0, -1.0):
        extras.tube(1, sign * (ey - 0.1), sign * (ey + 0.08), eye_r, eye_r * 0.9, "eye_gold",
                    sides=10, centre=(-eye_u * body_length, ez))
        extras.tube(1, sign * (ey + 0.05), sign * (ey + 0.14), eye_r * 0.55, eye_r * 0.45,
                    "pupil", sides=8, centre=(-eye_u * body_length, ez))
    if "gape" in spec:
        # An open mouth: a dark disc just inside the lips.
        r = spec["gape"] * 1.4 * min(depth(0.0), breadth(0.0))
        extras.tube(0, -0.3, 0.12, r, r * 0.9, "pupil", sides=10, centre=(0.0, centre(0.0)))
    if "spines" in spec:
        # Sharp rays standing out of the front dorsal fin.
        u0, u1, high = spec["spines"]
        for i in range(6):
            u = u0 + (u1 - u0) * i / 5
            top = centre(u) + depth(u) * 0.9
            h = high * spec["height"] * (1.1 - 0.4 * i / 5)
            segment(extras, (-u * body_length, 0.0, top),
                    (-u * body_length - 0.35 * h, 0.0, top + h), 0.09, 0.01, "spine", 4)
    if "crest" in spec:
        # A row of short spines down the back.
        u0, u1, count = spec["crest"]
        for i in range(count):
            u = u0 + (u1 - u0) * i / (count - 1)
            top = centre(u) + depth(u) * 0.95
            segment(extras, (-u * body_length, 0.0, top),
                    (-u * body_length - 0.5, 0.0, top + 0.6), 0.14, 0.01, "spine", 4)
    for i in range(spec.get("crystals", 0)):
        # Kyber crystals growing out of the back.
        u = 0.18 + 0.1 * i
        top = centre(u) + depth(u) * 0.8
        h = 0.55 + 0.35 * math.sin(i * 2.3) ** 2
        lean = 0.25 * math.sin(i * 1.7)
        segment(extras, (-u * body_length, 0.0, top),
                (-u * body_length - 0.15, lean, top + h), 0.16, 0.0, "kyber", 5)
    for reach, thick, droop in spec.get("barbels", []):
        # Whiskers from the corners of the mouth, curling out and back.
        for sign in (1.0, -1.0):
            points = []
            for i in range(7):
                v = i / 6
                points.append((-reach * 0.55 * v - 0.2, sign * (breadth(0.03) * 0.8 + reach * 0.45 * math.sin(v * 1.4)),
                               centre(0.0) + droop * depth(0.03) + droop * reach * 0.25 * v * v))
            strand(extras, points, thick, thick * 0.2, "barbel")

    wave = spec.get("wave", 0.0)
    if wave:
        # A swimming curve: the body bends side to side, more towards the tail.
        def bend(p, n):
            u = -p[0] / length
            return (p[0], p[1] + wave * math.sin(math.pi * 1.2 * u) * u, p[2]), n

        for mesh in (skin, fins, extras):
            moved = Mesh()
            moved.add(mesh, bend)
            mesh.positions, mesh.normals = moved.positions, moved.normals
    return skin, fins, extras


def ray_body(spec: dict, index: int) -> tuple:
    """The ghost ray: a flat diamond wing from its snout (the origin) back, eyes on top
    and a long thin tail."""
    length = spec["length"]
    body_length = length * 0.55
    half_w, thick = spec["width"] / 2, spec["height"] / 2
    rows, columns = 16, 15
    skin = Mesh()

    def outline(u: float) -> float:
        return half_w * pw(math.sin(math.pi * min(1.0, u ** 0.75)), 0.9)

    for face in (1.0, -1.0):
        grid = []
        for k in range(rows):
            u = k / (rows - 1)
            row = []
            for j in range(columns):
                v = -1.0 + 2.0 * j / (columns - 1)
                x = -u * body_length + 0.18 * body_length * (abs(v) ** 2) * math.sin(math.pi * u)
                y = v * outline(u)
                z = face * thick * math.sqrt(max(0.0, 1.0 - v * v)) * pw(math.sin(math.pi * min(1.0, 0.15 + u)), 0.5)
                if face > 0:
                    z += 0.6 * thick * math.cos(math.pi * v) * (1 - abs(v))
                t = 0.5 * abs(v) if face > 0 else 1.0 - 0.5 * abs(v)
                row.append(skin.vertex_st((x, y, z), (0, 0, face), skin_st(index, u, t)))
            grid.append(row)
        for r0, r1 in zip(grid, grid[1:]):
            for j in range(columns - 1):
                skin.triangles += [(r0[j], r1[j], r1[j + 1]), (r0[j], r1[j + 1], r0[j + 1])]
    smooth_normals(skin, lambda p: (0.0, p[1] * 0.3, p[2] if p[2] else 1.0))
    fins = Mesh()
    # Two small pelvic flaps at the back of the wing.
    for sign in (1.0, -1.0):
        base = [(-body_length * (0.86 + 0.04 * i), sign * 0.4, 0.0) for i in range(4)]
        edge = [(-body_length * (0.9 + 0.05 * i), sign * 1.6, -0.1) for i in range(4)]
        fin_strip(fins, index, base, edge, (0, 0, 1))
    extras = Mesh()
    for sign in (1.0, -1.0):
        at = (-body_length * 0.22, sign * 1.1)
        extras.tube(2, 0.2, thick * 1.5 + 0.3, 0.42, 0.36, "eye_gold", sides=8, centre=at)
        extras.tube(2, thick * 1.5 + 0.2, thick * 1.5 + 0.42, 0.22, 0.18, "pupil", sides=6,
                    centre=at)
    tail = [(-body_length - (length - body_length) * i / 8,
             0.25 * math.sin(i * 0.7), 0.15 * math.sin(i * 0.5)) for i in range(9)]
    tail[0] = (-body_length + 0.6, 0.0, 0.0)
    strand(extras, tail, 0.32, 0.03, "ray_tail")
    return skin, fins, extras


def fishes(output: Path) -> None:
    for index, spec in enumerate(FISH):
        skin, fins, extras = (ray_body if spec.get("ray") else fish_body)(spec, index)
        write_md3(output, fish_model(spec["id"]),
                  [(skin, FISH_SHADER), (fins, FISH_SHADER), (extras, SHADER)])


POND_RADIUS = 32.0
POND_WATER = 0.8


def rock(radius: float, height: float, sides: int, twist: float) -> Mesh:
    """A low stone: a squat, faceted lump, wider at the bottom, its texture wrapped
    round it."""
    mesh = Mesh()
    rings = ((0.0, radius), (height * 0.6, radius * 0.8), (height, radius * 0.3))
    loops = []
    for z, r in rings:
        loop = []
        for i in range(sides + 1):
            a = 2 * math.pi * i / sides + twist
            n = (math.cos(a), math.sin(a), 0.6 if z else 0.2)
            loop.append(mesh.vertex_st((r * math.cos(a), r * math.sin(a), z), n,
                                       (i / sides, z / 8.0)))
        loops.append(loop)
    for lower, upper in zip(loops, loops[1:]):
        for i in range(sides):
            mesh.triangles += [(lower[i], upper[i], upper[i + 1]),
                               (lower[i], upper[i + 1], lower[i + 1])]
    top = mesh.vertex_st((0.0, 0.0, height * 1.05), (0, 0, 1), (0.5, height / 8.0 + 0.1))
    last = loops[-1]
    for i in range(sides):
        mesh.triangles.append((top, last[i + 1], last[i]))
    return mesh


# The quarters of the game's leaf sheet (`textures/yavin/groundplants`) that the
# floating clumps and bank patches show: (s0, t0, s1, t1).
LEAF_PARTS = ((0.0, 0.0, 0.5, 0.5), (0.5, 0.0, 1.0, 0.5), (0.0, 0.5, 0.5, 1.0),
              (0.5, 0.5, 1.0, 1.0))


def leaf_patch(mesh: Mesh, x: float, y: float, z: float, size: float, turn: float,
               part) -> None:
    """A flat square of leaves (alpha-tested) centred on `(x, y)` at height `z`."""
    s0, t0, s1, t1 = part
    c, s = math.cos(turn), math.sin(turn)
    corners = []
    for dx, dy, st in ((-1, -1, (s0, t1)), (1, -1, (s1, t1)), (1, 1, (s1, t0)),
                       (-1, 1, (s0, t0))):
        px, py = dx * size / 2, dy * size / 2
        corners.append(mesh.vertex_st((x + px * c - py * s, y + px * s + py * c, z),
                                      (0, 0, 1), st))
    mesh.triangles += [(corners[0], corners[2], corners[1]),
                       (corners[0], corners[3], corners[2])]


def pond(output: Path) -> None:
    """The pond: its water, mapped flat so the game's ripples keep their size; a muddy
    bank from under the water's edge up and out to the ground; mossy stones round it;
    clumps of leaves floating and on the bank; and two clumps of reeds with cattails."""
    sides = 32

    def wobble(a: float) -> float:
        return 1.0 + 0.06 * math.sin(3 * a) + 0.04 * math.sin(7 * a + 1)

    water = Mesh()
    hub = water.vertex_st((0.0, 0.0, POND_WATER), (0, 0, 1), (0.0, 0.0))
    for i in range(sides):
        a = 2 * math.pi * i / sides
        r = POND_RADIUS * wobble(a)
        x, y = r * math.cos(a), r * math.sin(a)
        water.vertex_st((x, y, POND_WATER), (0, 0, 1), (x / 64.0, y / 64.0))
    for i in range(sides):
        water.triangles.append((hub, hub + 1 + (i + 1) % sides, hub + 1 + i))

    # The bank: three rings, under the water's edge, a low lip just above it, and the
    # ground, the mud mapped flat.
    bank = Mesh()
    rings = []
    for scale, extra, z in ((0.94, 0.0, 0.1), (1.0, 3.0, 2.0), (1.0, 10.0, 0.05)):
        ring = []
        for i in range(sides):
            a = 2 * math.pi * i / sides
            r = POND_RADIUS * wobble(a) * scale + extra
            x, y = r * math.cos(a), r * math.sin(a)
            n = (-0.3 * math.cos(a), -0.3 * math.sin(a), 1.0) if extra < 3 else (0, 0, 1)
            ring.append(bank.vertex_st((x, y, z), n, (x / 48.0, y / 48.0)))
        rings.append(ring)
    for inner, outer in zip(rings, rings[1:]):
        for i in range(sides):
            j = (i + 1) % sides
            bank.triangles += [(inner[i], inner[j], outer[j]), (inner[i], outer[j], outer[i])]

    stones = Mesh()
    count = 15
    for i in range(count):
        a = 2 * math.pi * (i + 0.3 * math.sin(i * 2.3)) / count
        r = POND_RADIUS * wobble(a) + 4.0 + 1.5 * math.sin(i * 1.7)
        size = 3.5 + 2.5 * (0.5 + 0.5 * math.sin(i * 3.1))
        stone = rock(size, size * 0.8, 6 + i % 3, i * 0.9)
        x, y = r * math.cos(a), r * math.sin(a)
        stones.add(stone, lambda p, n, x=x, y=y: ((p[0] + x, p[1] + y, p[2]), n))

    # Leaves: clumps floating near the edge of the water, patches on the bank.
    leaves = Mesh()
    for k, (a, r, size) in enumerate(((0.6, 0.7, 11.0), (2.0, 0.6, 9.0),
                                      (3.6, 0.72, 10.0), (4.9, 0.5, 8.0))):
        r *= POND_RADIUS
        leaf_patch(leaves, r * math.cos(a), r * math.sin(a), POND_WATER + 0.1, size,
                   a * 1.7, LEAF_PARTS[k % 4])
    for k in range(9):
        a = 2 * math.pi * (k + 0.5) / 9 + 0.2 * math.sin(k)
        r = POND_RADIUS * wobble(a) + 5.0
        leaf_patch(leaves, r * math.cos(a), r * math.sin(a), 0.4 + 0.02 * k, 8.0,
                   k * 2.1, LEAF_PARTS[(k + 1) % 4])

    # Two clumps of reeds on the bank, cattails on the tallest.
    reeds = Mesh()
    for a0 in (2.3, 5.4):
        for k in range(5):
            a = a0 + 0.12 * (k - 2)
            r = POND_RADIUS * wobble(a) + 1.5 + 0.8 * (k % 2)
            height = 13.0 + 3.0 * math.sin(k * 2.1 + a0)
            lean = 0.08 * (k - 2)
            stem = Mesh()
            stem.tube(2, 0.0, height, 0.28, 0.12, "reed", sides=4)
            if k % 2 == 0:
                stem.tube(2, height * 0.72, height * 0.88, 0.55, 0.55, "cattail", sides=6)
            x, y = r * math.cos(a), r * math.sin(a)
            reeds.add(stem, lambda p, n, x=x, y=y, lean=lean: ((p[0] + x + lean * p[2], p[1] + y, p[2]), n))
    write_md3(output, "fishing_pond", [
        (water, POND_SHADER), (bank, BANK_SHADER), (stones, STONE_SHADER),
        (leaves, LEAF_SHADER), (reeds, SHADER),
    ])


def icon(output: Path) -> None:
    """A rod arching over a bucket, drawn at four times the size and scaled down."""
    scale = 4
    size = ICON_SIZE * scale
    cover = [[0.0] * size for _ in range(size)]
    colour = [[(0, 0, 0)] * size for _ in range(size)]

    def paint(inside, rgb) -> None:
        for y in range(size):
            for x in range(size):
                if inside(x / scale, y / scale):
                    cover[y][x] = 1.0
                    colour[y][x] = rgb

    def segment(ax, ay, bx, by, width):
        def inside(x, y):
            dx, dy = bx - ax, by - ay
            t = max(0.0, min(1.0, ((x - ax) * dx + (y - ay) * dy) / (dx * dx + dy * dy)))
            return math.hypot(x - (ax + t * dx), y - (ay + t * dy)) <= width
        return inside

    # The bucket: a trapezoid with a rim and water.
    paint(lambda x, y: 82 <= y <= 118 and abs(x - 44) <= 18 + (y - 82) * -0.12 + 4, COLOURS["bucket"][1])
    paint(lambda x, y: 78 <= y <= 84 and abs(x - 44) <= 23, COLOURS["rim"][1])
    paint(lambda x, y: 84 < y <= 88 and abs(x - 44) <= 19, COLOURS["water"][1])
    # The rod from the lower right up to the upper left, the line down to the float.
    paint(segment(116, 120, 30, 14, 3.2), COLOURS["rod"][1])
    paint(segment(116, 120, 104, 105, 4.5), COLOURS["cork"][1])
    paint(segment(30, 14, 18, 60, 0.9), COLOURS["line"][1])
    paint(lambda x, y: math.hypot(x - 18, y - 64) <= 5 and y < 64, COLOURS["float_red"][1])
    paint(lambda x, y: math.hypot(x - 18, y - 64) <= 5 and y >= 64, COLOURS["float_white"][1])
    pixels = bytearray(ICON_SIZE * ICON_SIZE * 4)
    for y in range(ICON_SIZE):
        for x in range(ICON_SIZE):
            r = g = b = a = 0.0
            for sy in range(scale):
                for sx in range(scale):
                    c = cover[y * scale + sy][x * scale + sx]
                    rgb = colour[y * scale + sy][x * scale + sx]
                    r, g, b, a = r + rgb[0] * c, g + rgb[1] * c, b + rgb[2] * c, a + c
            i = (y * ICON_SIZE + x) * 4
            if a > 0:
                pixels[i : i + 4] = bytes(
                    (round(r / a), round(g / a), round(b / a), round(255 * a / scale ** 2))
                )
    png(output / "toy_fishing.png", ICON_SIZE, ICON_SIZE, pixels)


def main() -> None:
    if len(sys.argv) != 2:
        sys.exit(__doc__)
    output = Path(sys.argv[1])
    output.mkdir(parents=True, exist_ok=True)
    atlas(output)
    rod(output)
    bucket(output)
    float_model(output)
    line(output)
    fish_atlas(output)
    fishes(output)
    pond(output)
    icon(output)


if __name__ == "__main__":
    main()
