# SPDX-License-Identifier: MIT
"""Decode X11 XWD (xwd -silent) without ImageMagick XWD delegate (Buildroot IM lacks MAGICKCORE_X11_DELEGATE)."""

import os
import struct
import subprocess
from PIL import Image

# X11/X.h
MSBFirst = 42
LSBFirst = 43


def _byte_order_is_lsb(byte_order):
    """Some xwd(1) emit byte_order 0 meaning client-native LE; treat as LSBFirst."""
    return byte_order in (0, LSBFirst)
ZPixmap = 2
TrueColor = 4
DirectColor = 5

_SZ_HDR = 100


def _mask_shift(mask):
    if mask == 0:
        return 0
    s = 0
    m = mask
    while m and (m & 1) == 0:
        s += 1
        m >>= 1
    return s


def _decode_rgb_slow_32(raw, width, height, bpl, endian, red_m, green_m, blue_m):
    rs, gs, bs = _mask_shift(red_m), _mask_shift(green_m), _mask_shift(blue_m)
    out = bytearray(width * height * 3)
    i = 0
    ew = '<' if endian == 'little' else '>'
    fmt = ew + str(width) + 'I'
    for y in range(height):
        row = raw[y * bpl : y * bpl + width * 4]
        for word in struct.unpack(fmt, row):
            out[i] = ((word & red_m) >> rs) & 0xFF
            out[i + 1] = ((word & green_m) >> gs) & 0xFF
            out[i + 2] = ((word & blue_m) >> bs) & 0xFF
            i += 3
    return Image.frombytes('RGB', (width, height), bytes(out))


def _decode_rgb_slow_24(raw, width, height, bpl, byte_order):
    out = bytearray(width * height * 3)
    i = 0
    for y in range(height):
        row = raw[y * bpl : y * bpl + width * 3]
        for x in range(width):
            if _byte_order_is_lsb(byte_order):
                b0, b1, b2 = row[x * 3], row[x * 3 + 1], row[x * 3 + 2]
            else:
                b0, b1, b2 = row[x * 3 + 2], row[x * 3 + 1], row[x * 3]
            out[i], out[i + 1], out[i + 2] = b0, b1, b2
            i += 3
    return Image.frombytes('RGB', (width, height), bytes(out))


def _std_rgb888_masks(rm, gm, bm):
    return (rm & 0xFFFFFF) == 0xFF0000 and (gm & 0xFFFFFF) == 0xFF00 and (bm & 0xFFFFFF) == 0xFF


def _try_pillow_packed_rgb(raw, width, height, bpl, byte_order, bpp, depth, red_m, green_m, blue_m, visual):
    """Return RGB Image or None if layout is not a packed BGR/RGB row Pillow understands."""
    if visual not in (TrueColor, DirectColor):
        return None
    try:
        blob = raw.tobytes() if hasattr(raw, 'tobytes') else bytes(raw)
        if bpp == 32 and depth in (24, 32) and bpl >= width * 4:
            if _byte_order_is_lsb(byte_order) and _std_rgb888_masks(red_m, green_m, blue_m):
                return Image.frombytes('RGB', (width, height), blob, 'raw', 'BGRX', bpl, 1)
        if bpp == 24 and depth == 24 and bpl >= width * 3:
            if _byte_order_is_lsb(byte_order) and _std_rgb888_masks(red_m, green_m, blue_m):
                return Image.frombytes('RGB', (width, height), blob, 'raw', 'BGR', bpl, 1)
            if byte_order == MSBFirst and _std_rgb888_masks(red_m, green_m, blue_m):
                return Image.frombytes('RGB', (width, height), blob, 'raw', 'RGB', bpl, 1)
    except (ValueError, TypeError, MemoryError, OSError):
        return None
    return None


_SLGW_MAGIC = b'SLGW'


def _native_gray_helper():
    p = os.environ.get('SLOWOS_XWD_TO_GRAY', '/usr/local/bin/slowos-xwd-to-gray')
    return p if p and os.path.isfile(p) else None


def xwd_bytes_to_l_image(data: bytes):
    """
    Return mode ``L`` Image from raw xwd(1) stdout.
    Uses ``slowos-xwd-to-gray`` (native) when present (override with ``SLOWOS_XWD_TO_GRAY``),
    else Pillow RGB decode + ``convert('L')``.
    """
    xwd_bytes_to_l_image._last_decode_backend = 'python'
    helper = _native_gray_helper()
    if helper and os.environ.get('SLOWOS_XWD_NATIVE', '1').strip().lower() in ('1', 'true', 'yes', 'on'):
        try:
            r = subprocess.run(
                [helper],
                input=data,
                capture_output=True,
                timeout=30,
            )
            if r.returncode == 0 and r.stdout and len(r.stdout) >= 12 and r.stdout[:4] == _SLGW_MAGIC:
                w, h = struct.unpack('>II', r.stdout[4:12])
                body = r.stdout[12:]
                if w > 0 and h > 0 and len(body) >= w * h:
                    xwd_bytes_to_l_image._last_decode_backend = 'native'
                    return Image.frombytes('L', (w, h), body[: w * h])
        except (subprocess.TimeoutExpired, OSError, ValueError, struct.error):
            pass
    return xwd_bytes_to_image(data).convert('L')


def xwd_bytes_to_image(data: bytes) -> Image.Image:
    """Return RGB Image from raw xwd(1) stdout. Raises ValueError on unsupported layout."""
    if len(data) < _SZ_HDR:
        raise ValueError('xwd too short')
    h = struct.unpack('>25I', data[:_SZ_HDR])
    header_size = h[0]
    version = h[1]
    pixfmt = h[2]
    depth = h[3]
    width = h[4]
    height = h[5]
    byte_order = h[7]
    bpp = h[11]
    bpl = h[12]
    visual = h[13]
    red_m, green_m, blue_m = h[14], h[15], h[16]
    ncolors = h[19]

    if version != 7:
        raise ValueError(f'unsupported xwd version {version}')
    if header_size < _SZ_HDR or header_size > len(data):
        raise ValueError('bad xwd header_size')
    if pixfmt != ZPixmap:
        raise ValueError(f'unsupported pixmap_format {pixfmt}')
    if width <= 0 or height <= 0 or width > 16384 or height > 16384:
        raise ValueError('bad dimensions')
    if bpl <= 0 or bpl > len(data):
        raise ValueError('bad bytes_per_line')

    pix_off = header_size + ncolors * 12
    pix_len = bpl * height
    if len(data) < pix_off + pix_len:
        raise ValueError('truncated xwd pixel buffer')

    raw = memoryview(data)[pix_off : pix_off + pix_len]
    endian = 'little' if _byte_order_is_lsb(byte_order) else 'big'

    fast = _try_pillow_packed_rgb(
        raw, width, height, bpl, byte_order, bpp, depth, red_m, green_m, blue_m, visual
    )
    if fast is not None:
        return fast

    if visual in (TrueColor, DirectColor) and bpp == 32 and depth in (24, 32):
        return _decode_rgb_slow_32(raw, width, height, bpl, endian, red_m, green_m, blue_m)

    if visual in (TrueColor, DirectColor) and bpp == 24 and depth == 24:
        return _decode_rgb_slow_24(raw, width, height, bpl, byte_order)

    raise ValueError(
        f'unsupported xwd visual={visual} depth={depth} bpp={bpp} ncolors={ncolors}'
    )
