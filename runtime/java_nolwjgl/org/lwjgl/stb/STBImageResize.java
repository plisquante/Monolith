package org.lwjgl.stb;

import org.lwjgl.system.MemoryUtil;

/** Pure-Java bilinear RGBA/uint8 image resizer matching LWJGL STBImageResize. */
public final class STBImageResize {
    private STBImageResize() {}

    public static long nstbir_resize_uint8_linear(
            long inputPixels, int inputW, int inputH, int inputStride,
            long outputPixels, int outputW, int outputH, int outputStride, int numChannels) {
        if (inputPixels == 0L || inputW <= 0 || inputH <= 0 || outputW <= 0 || outputH <= 0 || numChannels <= 0) {
            return outputPixels;
        }
        int ch = Math.min(numChannels, 4);
        int inS = inputStride > 0 ? inputStride : inputW * numChannels;
        int outS = outputStride > 0 ? outputStride : outputW * numChannels;
        long dst = outputPixels != 0L ? outputPixels : MemoryUtil.nmemAlloc((long) outS * outputH);
        for (int y = 0; y < outputH; y++) {
            float sy = (outputH > 1) ? ((y + 0.5f) * inputH / outputH - 0.5f) : 0f;
            int y0 = Math.max(0, Math.min(inputH - 1, (int) Math.floor(sy)));
            int y1 = Math.max(0, Math.min(inputH - 1, y0 + 1));
            float fy = Math.max(0f, Math.min(1f, sy - y0));
            for (int x = 0; x < outputW; x++) {
                float sx = (outputW > 1) ? ((x + 0.5f) * inputW / outputW - 0.5f) : 0f;
                int x0 = Math.max(0, Math.min(inputW - 1, (int) Math.floor(sx)));
                int x1 = Math.max(0, Math.min(inputW - 1, x0 + 1));
                float fx = Math.max(0f, Math.min(1f, sx - x0));
                long o00 = inputPixels + (long) y0 * inS + (long) x0 * numChannels;
                long o10 = inputPixels + (long) y0 * inS + (long) x1 * numChannels;
                long o01 = inputPixels + (long) y1 * inS + (long) x0 * numChannels;
                long o11 = inputPixels + (long) y1 * inS + (long) x1 * numChannels;
                long outOff = dst + (long) y * outS + (long) x * numChannels;
                for (int c = 0; c < ch; c++) {
                    int p00 = MemoryUtil.memGetByte(o00 + c) & 0xFF;
                    int p10 = MemoryUtil.memGetByte(o10 + c) & 0xFF;
                    int p01 = MemoryUtil.memGetByte(o01 + c) & 0xFF;
                    int p11 = MemoryUtil.memGetByte(o11 + c) & 0xFF;
                    float top = p00 + (p10 - p00) * fx;
                    float bot = p01 + (p11 - p01) * fx;
                    int val = Math.max(0, Math.min(255, Math.round(top + (bot - top) * fy)));
                    MemoryUtil.memPutByte(outOff + c, (byte) val);
                }
            }
        }
        return dst;
    }
}
