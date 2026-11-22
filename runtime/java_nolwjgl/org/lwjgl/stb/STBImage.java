package org.lwjgl.stb;

import org.lwjgl.system.MemoryUtil;

import java.io.ByteArrayOutputStream;
import java.nio.ByteBuffer;
import java.nio.IntBuffer;
import java.util.zip.Inflater;

/** Pure-Java/zlib PNG image decoder implementing LWJGL STBImage without native libstb. */
public final class STBImage {
    private static volatile boolean flipVertically = false;
    /** Like stb_image: null until a decode fails (NativeLibrariesBootstrap.loadSTB treats any non-null reason as a load error). */
    private static volatile String lastReason = null;

    private STBImage() {}

    public static void stbi_set_flip_vertically_on_load(boolean flag) {
        flipVertically = flag;
    }

    public static String stbi_failure_reason() {
        return lastReason;
    }

    public static void stbi_image_free(ByteBuffer retvalFromStbiLoad) {
        // DirectByteBuffer memory managed cleanly without manual free crash.
    }

    public static void nstbi_image_free(long retvalFromStbiLoad) {}

    public static boolean stbi_info_from_memory(ByteBuffer buffer, IntBuffer x, IntBuffer y, IntBuffer comp) {
        if (buffer == null || buffer.remaining() < 24) return false;
        int pos = buffer.position();
        if ((buffer.get(pos) & 0xFF) != 0x89 || buffer.get(pos + 1) != 'P'
                || buffer.get(pos + 2) != 'N' || buffer.get(pos + 3) != 'G') {
            lastReason = "unknown image type";
            return false;
        }
        int w = readInt(buffer, pos + 16);
        int h = readInt(buffer, pos + 20);
        int colorType = buffer.get(pos + 25) & 0xFF;
        int ch = channelsForColorType(colorType);
        if (x != null && x.remaining() > 0) x.put(x.position(), w);
        if (y != null && y.remaining() > 0) y.put(y.position(), h);
        if (comp != null && comp.remaining() > 0) comp.put(comp.position(), ch);
        return true;
    }

    public static ByteBuffer stbi_load_from_memory(
            ByteBuffer buffer, IntBuffer x, IntBuffer y, IntBuffer channelsInFile, int desiredChannels) {
        if (buffer == null || buffer.remaining() < 24) {
            lastReason = "truncated PNG";
            return null;
        }
        try {
            byte[] raw = new byte[buffer.remaining()];
            int savedPos = buffer.position();
            buffer.get(raw);
            buffer.position(savedPos);
            return decodePng(raw, x, y, channelsInFile, desiredChannels);
        } catch (Throwable t) {
            lastReason = t.getMessage() != null ? t.getMessage() : "PNG decode error";
            return null;
        }
    }

    private static final int[] ADAM7_XSTART = {0, 4, 0, 2, 0, 1, 0};
    private static final int[] ADAM7_YSTART = {0, 0, 4, 0, 2, 0, 1};
    private static final int[] ADAM7_XSTEP = {8, 8, 4, 4, 2, 2, 1};
    private static final int[] ADAM7_YSTEP = {8, 8, 8, 4, 4, 2, 2};

    /**
     * Complete PNG decoder: every colour type and bit depth of the PNG specification (1 to 16 bits), palettes, tRNS
     * transparency and Adam7 interlacing. 16-bit samples are reduced to their high byte like stb_image does.
     */
    private static ByteBuffer decodePng(
            byte[] src, IntBuffer xOut, IntBuffer yOut, IntBuffer cOut, int desiredCh) throws Exception {
        if (src.length < 8 || (src[0] & 0xFF) != 0x89 || src[1] != 'P' || src[2] != 'N' || src[3] != 'G') {
            throw new IllegalArgumentException("Not a PNG image");
        }
        int w = 0, h = 0, bitDepth = 0, colorType = 0, interlace = 0;
        boolean haveHeader = false;
        byte[] plte = null;
        byte[] trns = null;
        ByteArrayOutputStream idat = new ByteArrayOutputStream();

        int off = 8;
        while (off + 12 <= src.length) {
            int len = readInt(src, off);
            int tag = readInt(src, off + 4);
            int dataOff = off + 8;
            if (len < 0 || dataOff + len + 4 > src.length) break;
            if (tag == 0x49484452 && len >= 13) { // IHDR
                w = readInt(src, dataOff);
                h = readInt(src, dataOff + 4);
                bitDepth = src[dataOff + 8] & 0xFF;
                colorType = src[dataOff + 9] & 0xFF;
                interlace = src[dataOff + 12] & 0xFF;
                haveHeader = true;
            } else if (tag == 0x504C5445) { // PLTE
                plte = new byte[len];
                System.arraycopy(src, dataOff, plte, 0, len);
            } else if (tag == 0x74524E53) { // tRNS
                trns = new byte[len];
                System.arraycopy(src, dataOff, trns, 0, len);
            } else if (tag == 0x49444154) { // IDAT
                idat.write(src, dataOff, len);
            } else if (tag == 0x49454E44) { // IEND
                break;
            }
            off += 12 + len;
        }
        if (!haveHeader || w <= 0 || h <= 0 || (long) w * h > Integer.MAX_VALUE / 4 || interlace > 1) {
            throw new IllegalArgumentException("Unsupported PNG geometry or bit depth: " + bitDepth);
        }
        boolean validDepth;
        switch (colorType) {
            case 0: validDepth = bitDepth == 1 || bitDepth == 2 || bitDepth == 4 || bitDepth == 8 || bitDepth == 16; break;
            case 3: validDepth = bitDepth == 1 || bitDepth == 2 || bitDepth == 4 || bitDepth == 8; break;
            case 2: case 4: case 6: validDepth = bitDepth == 8 || bitDepth == 16; break;
            default: validDepth = false;
        }
        if (!validDepth) {
            throw new IllegalArgumentException("Unsupported PNG geometry or bit depth: " + bitDepth);
        }
        if (colorType == 3 && plte == null) {
            throw new IllegalArgumentException("Palette PNG without a PLTE chunk");
        }

        int samples = channelsForColorType(colorType); // samples per pixel (a palette image has one index)
        int bitsPerPixel = samples * bitDepth;
        int filterBpp = Math.max(1, bitsPerPixel >> 3);

        // all passes together: raw size of the decompressed stream
        int[] passW = new int[7], passH = new int[7];
        long expected = 0;
        int passes = interlace == 1 ? 7 : 1;
        for (int ps = 0; ps < passes; ps++) {
            if (interlace == 1) {
                passW[ps] = (w - ADAM7_XSTART[ps] + ADAM7_XSTEP[ps] - 1) / ADAM7_XSTEP[ps];
                passH[ps] = (h - ADAM7_YSTART[ps] + ADAM7_YSTEP[ps] - 1) / ADAM7_YSTEP[ps];
            } else {
                passW[ps] = w;
                passH[ps] = h;
            }
            if (passW[ps] > 0 && passH[ps] > 0) {
                expected += (long) passH[ps] * (1 + (((long) passW[ps] * bitsPerPixel + 7) >> 3));
            }
        }
        if (expected > Integer.MAX_VALUE - 16) throw new IllegalArgumentException("PNG too large");

        Inflater inf = new Inflater();
        inf.setInput(idat.toByteArray());
        byte[] scan = new byte[(int) expected];
        int total = 0;
        try {
            while (total < scan.length) {
                int n = inf.inflate(scan, total, scan.length - total);
                if (n == 0 && (inf.finished() || inf.needsInput() || inf.needsDictionary())) break;
                total += n;
            }
        } finally {
            inf.end();
        }
        if (total < scan.length) throw new IllegalArgumentException("Corrupt PNG: image data is truncated");

        // gray/rgb colour keys of tRNS (16-bit values, compared against the raw samples)
        int keyR = -1, keyG = -1, keyB = -1;
        if (trns != null && colorType == 0 && trns.length >= 2) {
            keyR = ((trns[0] & 0xFF) << 8) | (trns[1] & 0xFF);
        } else if (trns != null && colorType == 2 && trns.length >= 6) {
            keyR = ((trns[0] & 0xFF) << 8) | (trns[1] & 0xFF);
            keyG = ((trns[2] & 0xFF) << 8) | (trns[3] & 0xFF);
            keyB = ((trns[4] & 0xFF) << 8) | (trns[5] & 0xFF);
        }
        boolean hasTrns = trns != null && (colorType == 0 || colorType == 2 || colorType == 3);

        byte[] rgba = new byte[w * h * 4];
        int pos = 0;
        for (int ps = 0; ps < passes; ps++) {
            int pw = passW[ps], ph = passH[ps];
            if (pw <= 0 || ph <= 0) continue;
            int rowBytes = (int) (((long) pw * bitsPerPixel + 7) >> 3);
            byte[] prev = new byte[rowBytes];
            byte[] cur = new byte[rowBytes];
            for (int row = 0; row < ph; row++) {
                int filter = scan[pos] & 0xFF;
                if (filter > 4) throw new IllegalArgumentException("Corrupt PNG: bad filter type " + filter);
                System.arraycopy(scan, pos + 1, cur, 0, rowBytes);
                pos += 1 + rowBytes;
                unfilter(filter, cur, prev, rowBytes, filterBpp);
                int y = interlace == 1 ? ADAM7_YSTART[ps] + row * ADAM7_YSTEP[ps] : row;
                for (int col = 0; col < pw; col++) {
                    int x = interlace == 1 ? ADAM7_XSTART[ps] + col * ADAM7_XSTEP[ps] : col;
                    int r, g, b, a = 255;
                    switch (colorType) {
                        case 0: {
                            int v = sample(cur, col, bitDepth);
                            r = g = b = scale(v, bitDepth);
                            if (keyR >= 0 && v == keyR) a = 0;
                            break;
                        }
                        case 2: {
                            int rv = sample(cur, col * 3, bitDepth), gv = sample(cur, col * 3 + 1, bitDepth), bv = sample(cur, col * 3 + 2, bitDepth);
                            r = scale(rv, bitDepth); g = scale(gv, bitDepth); b = scale(bv, bitDepth);
                            if (keyR >= 0 && rv == keyR && gv == keyG && bv == keyB) a = 0;
                            break;
                        }
                        case 3: {
                            int idx = sample(cur, col, bitDepth);
                            if (idx * 3 + 2 < plte.length) {
                                r = plte[idx * 3] & 0xFF; g = plte[idx * 3 + 1] & 0xFF; b = plte[idx * 3 + 2] & 0xFF;
                            } else {
                                r = g = b = 0;
                            }
                            if (trns != null && idx < trns.length) a = trns[idx] & 0xFF;
                            break;
                        }
                        case 4: {
                            r = g = b = scale(sample(cur, col * 2, bitDepth), bitDepth);
                            a = scale(sample(cur, col * 2 + 1, bitDepth), bitDepth);
                            break;
                        }
                        default: { // 6
                            r = scale(sample(cur, col * 4, bitDepth), bitDepth);
                            g = scale(sample(cur, col * 4 + 1, bitDepth), bitDepth);
                            b = scale(sample(cur, col * 4 + 2, bitDepth), bitDepth);
                            a = scale(sample(cur, col * 4 + 3, bitDepth), bitDepth);
                        }
                    }
                    int di = (y * w + x) * 4;
                    rgba[di] = (byte) r; rgba[di + 1] = (byte) g; rgba[di + 2] = (byte) b; rgba[di + 3] = (byte) a;
                }
                byte[] tmp = prev; prev = cur; cur = tmp;
            }
        }

        // channels of the file as stb_image reports them: palette images are 3 (4 with tRNS), tRNS adds an alpha channel
        int fileCh;
        switch (colorType) {
            case 0: fileCh = hasTrns ? 2 : 1; break;
            case 2: fileCh = hasTrns ? 4 : 3; break;
            case 3: fileCh = hasTrns ? 4 : 3; break;
            case 4: fileCh = 2; break;
            default: fileCh = 4;
        }
        int outCh = (desiredCh > 0 && desiredCh <= 4) ? desiredCh : fileCh;
        ByteBuffer out = MemoryUtil.memAlloc(w * h * outCh);
        for (int row = 0; row < h; row++) {
            int dstRow = flipVertically ? (h - 1 - row) : row;
            int dstBase = dstRow * w * outCh;
            for (int col = 0; col < w; col++) {
                int si = (row * w + col) * 4;
                int r = rgba[si] & 0xFF, g = rgba[si + 1] & 0xFF, b = rgba[si + 2] & 0xFF, a = rgba[si + 3] & 0xFF;
                int dp = dstBase + col * outCh;
                if (outCh <= 2) {
                    out.put(dp, (byte) ((r * 77 + g * 150 + b * 29) >> 8)); // stb_image's luminance
                    if (outCh == 2) out.put(dp + 1, (byte) a);
                } else {
                    out.put(dp, (byte) r);
                    out.put(dp + 1, (byte) g);
                    out.put(dp + 2, (byte) b);
                    if (outCh == 4) out.put(dp + 3, (byte) a);
                }
            }
        }

        if (xOut != null && xOut.remaining() > 0) xOut.put(xOut.position(), w);
        if (yOut != null && yOut.remaining() > 0) yOut.put(yOut.position(), h);
        if (cOut != null && cOut.remaining() > 0) cOut.put(cOut.position(), fileCh);
        lastReason = null;
        return out;
    }

    /** The n-th sample (of {@code bitDepth} bits) of an unfiltered scanline. */
    private static int sample(byte[] row, int index, int bitDepth) {
        switch (bitDepth) {
            case 8:
                return row[index] & 0xFF;
            case 16:
                return ((row[index * 2] & 0xFF) << 8) | (row[index * 2 + 1] & 0xFF);
            default: {
                int perByte = 8 / bitDepth;
                int b = row[index / perByte] & 0xFF;
                int shift = 8 - bitDepth * (index % perByte + 1);
                return (b >> shift) & ((1 << bitDepth) - 1);
            }
        }
    }

    /** Sample value to 0..255 (low depths are stretched, 16-bit keeps the high byte). */
    private static int scale(int v, int bitDepth) {
        switch (bitDepth) {
            case 8: return v;
            case 16: return v >> 8;
            default: return v * 255 / ((1 << bitDepth) - 1);
        }
    }

    private static void unfilter(int filter, byte[] cur, byte[] prev, int stride, int bpp) {
        if (filter == 0) return;
        for (int i = 0; i < stride; i++) {
            int raw = cur[i] & 0xFF;
            int a = (i >= bpp) ? (cur[i - bpp] & 0xFF) : 0;
            int b = prev[i] & 0xFF;
            int c = (i >= bpp) ? (prev[i - bpp] & 0xFF) : 0;
            int val = raw;
            if (filter == 1) val = raw + a;
            else if (filter == 2) val = raw + b;
            else if (filter == 3) val = raw + ((a + b) >> 1);
            else if (filter == 4) val = raw + paeth(a, b, c);
            cur[i] = (byte) val;
        }
    }

    private static int paeth(int a, int b, int c) {
        int p = a + b - c;
        int pa = Math.abs(p - a);
        int pb = Math.abs(p - b);
        int pc = Math.abs(p - c);
        if (pa <= pb && pa <= pc) return a;
        return (pb <= pc) ? b : c;
    }

    private static int channelsForColorType(int colorType) {
        return switch (colorType) {
            case 0, 3 -> 1;
            case 2 -> 3;
            case 4 -> 2;
            default -> 4;
        };
    }

    private static int readInt(ByteBuffer b, int off) {
        return ((b.get(off) & 0xFF) << 24)
                | ((b.get(off + 1) & 0xFF) << 16)
                | ((b.get(off + 2) & 0xFF) << 8)
                | (b.get(off + 3) & 0xFF);
    }

    private static int readInt(byte[] b, int off) {
        return ((b[off] & 0xFF) << 24)
                | ((b[off + 1] & 0xFF) << 16)
                | ((b[off + 2] & 0xFF) << 8)
                | (b[off + 3] & 0xFF);
    }
}
