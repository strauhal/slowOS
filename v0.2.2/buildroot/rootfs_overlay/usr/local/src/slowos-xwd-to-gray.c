/*
 * slowos-xwd-to-gray — stdin: XWD (ZPixmap, 32bpp, LSBFirst, std RGB888 masks)
 * stdout: magic "SLGW" + BE width + BE height + width*height uint8 luma (ITU-ish int).
 * exit 3: unsupported layout (caller falls back to Python decode).
 * SPDX-License-Identifier: MIT
 */
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>

#define XWD_HDR 100
#define MAGIC "SLGW"

static uint32_t be32(const unsigned char *p)
{
	return ((uint32_t)p[0] << 24) | ((uint32_t)p[1] << 16) | ((uint32_t)p[2] << 8) | (uint32_t)p[3];
}

static int read_all(unsigned char **out, size_t *outlen)
{
	size_t cap = 1u << 22;
	unsigned char *buf = (unsigned char *)malloc(cap);
	if (!buf)
		return -1;
	size_t n = 0;
	for (;;) {
		size_t r = fread(buf + n, 1, cap - n, stdin);
		if (r == 0)
			break;
		n += r;
		if (n >= cap) {
			cap *= 2;
			unsigned char *nb = (unsigned char *)realloc(buf, cap);
			if (!nb) {
				free(buf);
				return -1;
			}
			buf = nb;
		}
	}
	*out = buf;
	*outlen = n;
	return 0;
}

int main(void)
{
	unsigned char *buf = NULL;
	size_t n = 0;
	if (read_all(&buf, &n) != 0 || n < XWD_HDR) {
		free(buf);
		return 2;
	}
	if (be32(buf + 0) < XWD_HDR || be32(buf + 0) > n)
		goto unsup;
	if (be32(buf + 4) != 7)
		goto unsup; /* version */
	if (be32(buf + 8) != 2)
		goto unsup; /* ZPixmap */
	uint32_t depth = be32(buf + 12);
	uint32_t width = be32(buf + 16);
	uint32_t height = be32(buf + 20);
	uint32_t byte_order = be32(buf + 28);
	uint32_t bpp = be32(buf + 44);
	uint32_t bpl = be32(buf + 48);
	uint32_t visual = be32(buf + 52);
	uint32_t red_m = be32(buf + 56);
	uint32_t green_m = be32(buf + 60);
	uint32_t blue_m = be32(buf + 64);
	uint32_t ncolors = be32(buf + 76);
	(void)visual;
	/* xwd on some servers reports byte_order 0 for client-native LE; treat like LSBFirst. */
	if (byte_order != 0 && byte_order != 43)
		goto unsup;
	if (bpp != 32 || (depth != 24 && depth != 32))
		goto unsup;
	if (width == 0 || height == 0 || width > 16384 || height > 16384)
		goto unsup;
	if (bpl < width * 4u)
		goto unsup;
	uint32_t hdrsz = be32(buf + 0);
	uint32_t pix_off = hdrsz + ncolors * 12u;
	if (pix_off > n || pix_off + bpl * height > n)
		goto unsup;
	/* Standard TrueColor LE BGRX masks */
	if (((red_m & 0xffffffu) != 0xff0000u) || ((green_m & 0xffffffu) != 0xff00u) ||
	    ((blue_m & 0xffffffu) != 0xffu))
		goto unsup;

	size_t gray_n = (size_t)width * (size_t)height;
	unsigned char *gray = (unsigned char *)malloc(gray_n);
	if (!gray) {
		free(buf);
		return 2;
	}
	const unsigned char *pix = buf + pix_off;
	for (uint32_t y = 0; y < height; y++) {
		const unsigned char *row = pix + (size_t)y * bpl;
		unsigned char *dst = gray + (size_t)y * width;
		for (uint32_t x = 0; x < width; x++) {
			uint32_t wv = (uint32_t)row[x * 4] | ((uint32_t)row[x * 4 + 1] << 8) |
				       ((uint32_t)row[x * 4 + 2] << 16) | ((uint32_t)row[x * 4 + 3] << 24);
			unsigned b = wv & 0xffu;
			unsigned g = (wv >> 8) & 0xffu;
			unsigned r = (wv >> 16) & 0xffu;
			dst[x] = (unsigned char)((77u * r + 150u * g + 29u * b) >> 8);
		}
	}
	free(buf);
	buf = NULL;

	unsigned char head[12];
	memcpy(head, MAGIC, 4);
	head[4] = (unsigned char)((width >> 24) & 255);
	head[5] = (unsigned char)((width >> 16) & 255);
	head[6] = (unsigned char)((width >> 8) & 255);
	head[7] = (unsigned char)(width & 255);
	head[8] = (unsigned char)((height >> 24) & 255);
	head[9] = (unsigned char)((height >> 16) & 255);
	head[10] = (unsigned char)((height >> 8) & 255);
	head[11] = (unsigned char)(height & 255);
	if (fwrite(head, 1, 12, stdout) != 12 || fwrite(gray, 1, gray_n, stdout) != gray_n) {
		free(gray);
		return 2;
	}
	free(gray);
	return 0;
unsup:
	free(buf);
	return 3;
}
