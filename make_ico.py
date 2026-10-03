# -*- coding: utf-8 -*-
"""从 app-icon.png 手写生成 BMP 条目的多尺寸 icon.ico
（传统 BMP 条目：兼容 LoadImage / ExtractAssociatedIcon / 旧 Shell API）"""
import struct
from PIL import Image

SRC = r"D:\image-edit\ai-image\src-tauri\icons\app-icon.png"
DST = r"D:\image-edit\ai-image\src-tauri\icons\icon.ico"
SIZES = [16, 24, 32, 48, 64, 128, 256]

img = Image.open(SRC).convert("RGBA")
w, h = img.size
side = min(w, h)
left = (w - side) // 2
top = (h - side) // 2
img = img.crop((left, top, left + side, top + side))


def encode_bmp_entry(im: Image.Image) -> bytes:
    """单条目：BITMAPINFOHEADER + BGRA 像素（自底向上）+ 全零 AND mask"""
    w, h = im.size
    px = im.load()
    data = bytearray()
    for y in range(h - 1, -1, -1):
        for x in range(w):
            r, g, b, a = px[x, y]
            data += struct.pack("BBBB", b, g, r, a)
    mask_row = ((w + 31) // 32) * 4
    data += b"\x00" * (mask_row * h)
    # BITMAPINFOHEADER：biHeight 为 2 倍高（XOR + AND）
    header = struct.pack("<IiiHHIIiiII", 40, w, h * 2, 1, 32, 0, len(data), 0, 0, 0, 0)
    return header + bytes(data)


entries = [(s, encode_bmp_entry(img.resize((s, s), Image.LANCZOS))) for s in SIZES]

count = len(entries)
# ICO 规范：目录区（N×16 字节）整体在前，图像数据区整体在后
header = struct.pack("<HHH", 0, 1, count)
dir_area = b""
data_area = b""
offset = 6 + 16 * count
for s, data in entries:
    wh = 0 if s >= 256 else s
    dir_area += struct.pack("<BBBBHHII", wh, wh, 0, 0, 1, 32, len(data), offset)
    data_area += data
    offset += len(data)
ico = header + dir_area + data_area

with open(DST, "wb") as f:
    f.write(ico)
print(f"OK: {DST}（{len(ico)} 字节，{count} 个 BMP 条目）")
