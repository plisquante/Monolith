package org.lwjgl.stb;

import org.lwjgl.system.Callback;
import org.lwjgl.system.MemoryUtil;

import java.io.ByteArrayOutputStream;
import java.util.zip.CRC32;
import java.util.zip.Deflater;

/** Pure-Java PNG encoder implementing LWJGL STBImageWrite.nstbi_write_png_to_func without native libstb. */
public final class STBImageWrite {
    private STBImageWrite() {}

    public static int nstbi_write_png_to_func(
            long func, long context, int w, int h, int comp, long data, int strideInBytes) {
        if (func == 0L || data == 0L || w <= 0 || h <= 0 || (comp != 1 && comp != 3 && comp != 4)) {
            return 0;
        }
        STBIWriteCallback cb = Callback.get(func);
        if (cb == null) return 0;
        int stride = strideInBytes > 0 ? strideInBytes : w * comp;
        int rowBytes = w * comp;
        byte[] raw = new byte[h * (1 + rowBytes)];
        for (int y = 0; y < h; y++) {
            int dstRow = y * (1 + rowBytes);
            raw[dstRow] = 0;
            long srcRow = data + (long) y * stride;
            for (int i = 0; i < rowBytes; i++) {
                raw[dstRow + 1 + i] = MemoryUtil.memGetByte(srcRow + i);
            }
        }
        try {
            byte[] png = encodePng(w, h, comp, raw);
            long tmp = MemoryUtil.nmemAlloc(png.length);
            try {
                for (int i = 0; i < png.length; i++) MemoryUtil.memPutByte(tmp + i, png[i]);
                cb.invoke(context, tmp, png.length);
            } finally {
                MemoryUtil.nmemFree(tmp);
            }
            return 1;
        } catch (Throwable t) {
            return 0;
        }
    }

    private static byte[] encodePng(int w, int h, int comp, byte[] rawScanlines) {
        byte colorType = (byte) (comp == 1 ? 0 : (comp == 3 ? 2 : 6));
        ByteArrayOutputStream ihdr = new ByteArrayOutputStream(13);
        putInt(ihdr, w);
        putInt(ihdr, h);
        ihdr.write(8);
        ihdr.write(colorType);
        ihdr.write(0);
        ihdr.write(0);
        ihdr.write(0);

        Deflater def = new Deflater();
        def.setInput(rawScanlines);
        def.finish();
        ByteArrayOutputStream idat = new ByteArrayOutputStream(rawScanlines.length / 2 + 64);
        byte[] buf = new byte[4096];
        while (!def.finished()) {
            int n = def.deflate(buf);
            if (n > 0) idat.write(buf, 0, n);
        }
        def.end();

        ByteArrayOutputStream out = new ByteArrayOutputStream(idat.size() + 64);
        out.write((byte) 0x89); out.write('P'); out.write('N'); out.write('G');
        out.write(13); out.write(10); out.write(26); out.write(10);
        writeChunk(out, "IHDR", ihdr.toByteArray());
        writeChunk(out, "IDAT", idat.toByteArray());
        writeChunk(out, "IEND", new byte[0]);
        return out.toByteArray();
    }

    private static void writeChunk(ByteArrayOutputStream out, String type, byte[] data) {
        putInt(out, data.length);
        byte[] t = type.getBytes(java.nio.charset.StandardCharsets.US_ASCII);
        out.write(t, 0, 4);
        out.write(data, 0, data.length);
        CRC32 crc = new CRC32();
        crc.update(t, 0, 4);
        crc.update(data, 0, data.length);
        putInt(out, (int) crc.getValue());
    }

    private static void putInt(ByteArrayOutputStream out, int v) {
        out.write((v >>> 24) & 0xFF);
        out.write((v >>> 16) & 0xFF);
        out.write((v >>> 8) & 0xFF);
        out.write(v & 0xFF);
    }
}
