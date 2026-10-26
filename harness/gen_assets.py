# Génère les assets déterministes communs à toutes les stacks.
import zlib, struct, os, sys
out = sys.argv[1]; os.makedirs(out + "/images", exist_ok=True)
def fnv(b):
    h = 2166136261
    for x in b: h = ((h ^ x) * 16777619) & 0xFFFFFFFF
    return h
def png(w, h, px):
    raw = b"".join(b"\x00" + bytes(px[y*w*3:(y+1)*w*3]) for y in range(h))
    def ch(t, d): c = struct.pack(">I", len(d)) + t + d; return c + struct.pack(">I", zlib.crc32(t + d) & 0xFFFFFFFF)
    return b"\x89PNG\r\n\x1a\n" + ch(b"IHDR", struct.pack(">IIBBBBB", w, h, 8, 2, 0, 0, 0)) + ch(b"IDAT", zlib.compress(raw, 6)) + ch(b"IEND", b"")
# 600 images 256x256 : dégradé + motif dépendant de l'index
for i in range(600):
    s = fnv(i.to_bytes(4, "little")); r0, g0, b0 = s & 255, (s >> 8) & 255, (s >> 16) & 255
    px = bytearray(256*256*3)
    for y in range(256):
        for x in range(256):
            o = (y*256 + x)*3
            k = ((x // 16) + (y // 16)) & 1
            px[o] = (r0 + x) & 255 if k else (r0 ^ y) & 255
            px[o+1] = (g0 + y) & 255 if k else (g0 ^ x) & 255
            px[o+2] = (b0 + x + y) & 255 if not k else (b0 ^ (x ^ y)) & 255
    open(f"{out}/images/img{i:03d}.png", "wb").write(png(256, 256, px))
# texte 1 Mo (UTF-8) : latin, CJK, emoji, arabe/hébreu (bidi), lignes de ~80 caractères
words = ["lorem","ipsum","dolor","sit","amet","日本語","テキスト","漢字","한국어","中文","emoji😀","🚀","👩‍👩‍👧‍👦","مرحبا","بالعالم","שלום","עולם","naïve","façade","Ωmega"]
lines, size, i = [], 0, 0
while size < 1_000_000:
    n = 8 + fnv(i.to_bytes(4, "little")) % 8
    line = " ".join(words[fnv((i*31 + j).to_bytes(4, "little")) % len(words)] for j in range(n)) + "\n"
    lines.append(line); size += len(line.encode()); i += 1
open(out + "/text1mb.txt", "w", encoding="utf-8").write("".join(lines))
print("ok", len(lines), "lignes", size, "octets")
