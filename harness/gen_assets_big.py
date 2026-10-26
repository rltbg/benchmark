#!/usr/bin/env python3 -I
"""Gros assets déterministes du banc v3 : textes de S Mo et images carrées SIDE x SIDE.
usage: gen_assets_big.py OUTDIR [--text-mb 10 50 100] [--big 4096 8192]"""
import sys, os, zlib, struct
import numpy as np
out = sys.argv[1]; args = sys.argv[2:]
def opt(name):
    if name not in args: return []
    i = args.index(name) + 1; v = []
    while i < len(args) and not args[i].startswith("--"): v.append(int(args[i])); i += 1
    return v
def fnv(i):
    h = 2166136261
    for x in int(i).to_bytes(4, "little"): h = ((h ^ x) * 16777619) & 0xFFFFFFFF
    return h
words = ["lorem","ipsum","dolor","sit","amet","日本語","テキスト","漢字","한국어","中文","emoji😀","🚀","👩‍👩‍👧‍👦","مرحبا","بالعالم","שלום","עולם","naïve","façade","Ωmega"]
for mb in opt("--text-mb"):
    path = f"{out}/text{mb}mb.txt"; target = mb * 1_000_000; size = 0; i = 0
    with open(path, "w", encoding="utf-8") as f:
        while size < target:
            n = 8 + fnv(i) % 8
            line = " ".join(words[fnv(i * 31 + j) % len(words)] for j in range(n)) + "\n"
            f.write(line); size += len(line.encode()); i += 1
    print(path, size, "octets,", i, "lignes")
def png(path, arr):
    h, w, _ = arr.shape; comp = zlib.compressobj(3); raw = bytearray()
    def ch(t, d): c = struct.pack(">I", len(d)) + t + d; return c + struct.pack(">I", zlib.crc32(t + d) & 0xFFFFFFFF)
    with open(path, "wb") as f:
        f.write(b"\x89PNG\r\n\x1a\n"); f.write(ch(b"IHDR", struct.pack(">IIBBBBB", w, h, 8, 2, 0, 0, 0)))
        buf = b""
        for y in range(h):
            buf += comp.compress(b"\x00" + arr[y].tobytes())
            if len(buf) > 1 << 22: f.write(ch(b"IDAT", buf)); buf = b""
        buf += comp.flush()
        if buf: f.write(ch(b"IDAT", buf))
        f.write(ch(b"IEND", b""))
for side in opt("--big"):
    rng = np.random.default_rng(12345)
    y = np.arange(side, dtype=np.uint32)[:, None]; x = np.arange(side, dtype=np.uint32)[None, :]
    img = np.empty((side, side, 3), dtype=np.uint8)
    img[..., 0] = ((x * 255) // side).astype(np.uint8) ^ ((y // 64) * 37).astype(np.uint8)
    img[..., 1] = ((y * 255) // side).astype(np.uint8) ^ ((x // 64) * 53).astype(np.uint8)
    img[..., 2] = (((x + y) * 255) // (2 * side)).astype(np.uint8)
    dots = rng.integers(0, side, size=(side * 4, 2)); img[dots[:, 0], dots[:, 1]] = 255   # points épars = détail pour le zoom
    png(f"{out}/big{side}.png", img); print(f"{out}/big{side}.png", os.path.getsize(f"{out}/big{side}.png"), "octets")
