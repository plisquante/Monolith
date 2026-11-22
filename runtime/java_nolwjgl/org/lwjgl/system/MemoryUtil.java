package org.lwjgl.system;

import mono.rhi.NativeRhi;
import jdk.internal.misc.Unsafe;
import org.lwjgl.PointerBuffer;

import java.nio.Buffer;
import java.nio.ByteBuffer;
import java.nio.ByteOrder;
import java.nio.CharBuffer;
import java.nio.DoubleBuffer;
import java.nio.FloatBuffer;
import java.nio.IntBuffer;
import java.nio.LongBuffer;
import java.nio.ShortBuffer;
import java.nio.charset.StandardCharsets;

/** Zero-LWJGL MemoryUtil backed by Unsafe off-heap allocation and NativeRhi direct buffer wrapping. */
public final class MemoryUtil {
    public static final long NULL = 0L;
    private static final Unsafe UNSAFE = Unsafe.getUnsafe();

    public interface MemoryAllocator {
        long malloc(long size);
        long realloc(long ptr, long size);
        void free(long ptr);
    }

    private static final MemoryAllocator ALLOCATOR = new MemoryAllocator() {
        @Override public long malloc(long size) { return nmemAlloc(size); }
        @Override public long realloc(long ptr, long size) { return UNSAFE.reallocateMemory(ptr, size); }
        @Override public void free(long ptr) { nmemFree(ptr); }
    };

    /**
     * Minecraft's DebugMemoryUntracker looks up LWJGL's debug allocator by reflection: the lazily created allocator
     * is MemoryUtil$LazyInit.ALLOCATOR and is only a MemoryManage$DebugAllocator when LWJGL memory debugging is on.
     * Here it never is, so untracking is a no-op, exactly as in an LWJGL run without -Dorg.lwjgl.util.DebugAllocator.
     */
    private static final class LazyInit {
        private static final MemoryAllocator ALLOCATOR = MemoryUtil.ALLOCATOR;
    }

    private MemoryUtil() {}

    public static MemoryAllocator getAllocator(boolean tracked) {
        return ALLOCATOR;
    }

    public static ByteBuffer memAlloc(int size) {
        return ByteBuffer.allocateDirect(size).order(ByteOrder.nativeOrder());
    }

    public static ByteBuffer memCalloc(int size) {
        return memAlloc(size);
    }

    public static ByteBuffer memCalloc(int num, int size) {
        return memAlloc(num * size);
    }

    public static IntBuffer memAllocInt(int size) {
        return memAlloc(size << 2).asIntBuffer();
    }

    public static FloatBuffer memAllocFloat(int size) {
        return memAlloc(size << 2).asFloatBuffer();
    }

    public static PointerBuffer memAllocPointer(int size) {
        return PointerBuffer.allocateDirect(size);
    }

    public static ByteBuffer memByteBuffer(long address, int capacity) {
        if (address == 0L || capacity < 0) return memAlloc(0);
        try {
            ByteBuffer wrapped = NativeRhi.wrapDirectByteBuffer(address, capacity);
            if (wrapped != null) return wrapped.order(ByteOrder.nativeOrder());
        } catch (Throwable ignored) {
        }
        ByteBuffer b = memAlloc(0);
        UNSAFE.putLong(b, addressOffset(), address);
        UNSAFE.putInt(b, capacityOffset(), capacity);
        b.limit(capacity).position(0);
        return b;
    }

    public static ByteBuffer memByteBuffer(IntBuffer buffer) {
        if (buffer == null) return null;
        return memByteBuffer(memAddress(buffer), buffer.remaining() << 2);
    }

    public static IntBuffer memIntBuffer(long address, int capacity) {
        return memByteBuffer(address, capacity << 2).asIntBuffer();
    }

    public static ByteBuffer memSlice(ByteBuffer buffer, int offset, int capacity) {
        if (buffer == null) return null;
        return memByteBuffer(memAddress(buffer) + offset, capacity);
    }

    /**
     * LWJGL semantics: the contents of the whole old buffer (its capacity, not just what is left after the position)
     * are kept and the new buffer continues at the old position. Minecraft's TextureUtil.readResource grows its
     * read buffer with this call while reading a resource of more than 8 KiB.
     */
    public static ByteBuffer memRealloc(ByteBuffer ptr, int size) {
        ByteBuffer nb = memAlloc(size);
        if (ptr != null) {
            int copy = Math.min(ptr.capacity(), size);
            memCopy(memAddress(ptr) - ptr.position(), memAddress(nb), copy);
            nb.position(Math.min(ptr.position(), size));
        }
        return nb;
    }

    public static PointerBuffer memRealloc(PointerBuffer ptr, int size) {
        PointerBuffer nb = PointerBuffer.allocateDirect(size);
        if (ptr != null) {
            int copy = Math.min(ptr.remaining(), size);
            memCopy(ptr.address(), nb.address(), (long) copy * Pointer.POINTER_SIZE);
        }
        return nb;
    }

    public static void memFree(Buffer ptr) {
        // DirectByteBuffers managed cleanly; explicit off-heap memory frees via nmemFree.
    }

    public static void memFree(ByteBuffer ptr) {}
    public static void memFree(IntBuffer ptr) {}
    public static void memFree(CustomBuffer<?> ptr) {
        if (ptr != null) ptr.free();
    }

    public static void nmemFree(long ptr) {
        if (ptr != 0L) UNSAFE.freeMemory(ptr);
    }

    public static long nmemAlloc(long size) {
        return UNSAFE.allocateMemory(Math.max(1L, size));
    }

    public static long nmemCalloc(long num, long size) {
        long bytes = num * size;
        long p = UNSAFE.allocateMemory(Math.max(1L, bytes));
        if (bytes > 0L) UNSAFE.setMemory(p, bytes, (byte) 0);
        return p;
    }

    public static long memAddress(Buffer buffer) {
        if (buffer == null) return 0L;
        if (!buffer.isDirect()) throw new IllegalArgumentException("Buffer is not direct");
        return UNSAFE.getLong(buffer, addressOffset()) + (long) buffer.position() * elementSize(buffer);
    }

    public static long memAddress(ByteBuffer buffer) { return memAddress((Buffer) buffer); }
    public static long memAddress(IntBuffer buffer) { return memAddress((Buffer) buffer); }
    public static long memAddress(CustomBuffer<?> buffer) { return buffer == null ? 0L : buffer.address(); }

    public static long memAddressSafe(Buffer buffer) {
        return buffer == null ? 0L : memAddress(buffer);
    }

    public static void memCopy(long src, long dst, long bytes) {
        if (bytes > 0L && src != 0L && dst != 0L) UNSAFE.copyMemory(src, dst, bytes);
    }

    public static void memCopy(ByteBuffer src, ByteBuffer dst) {
        if (src == null || dst == null) return;
        int n = Math.min(src.remaining(), dst.remaining());
        if (src.isDirect() && dst.isDirect()) {
            memCopy(memAddress(src), memAddress(dst), n);
        } else {
            for (int i = 0; i < n; i++) dst.put(dst.position() + i, src.get(src.position() + i));
        }
    }

    public static void memSet(long ptr, int value, long bytes) {
        if (bytes > 0L && ptr != 0L) UNSAFE.setMemory(ptr, bytes, (byte) value);
    }

    public static void memSet(CustomBuffer<?> ptr, int value) {
        if (ptr != null) memSet(ptr.address(), value, (long) ptr.remaining() * ptr.sizeof());
    }

    public static byte memGetByte(long ptr) { return UNSAFE.getByte(ptr); }
    public static int memGetInt(long ptr) { return UNSAFE.getInt(ptr); }
    public static long memGetAddress(long ptr) { return UNSAFE.getLong(ptr); }
    public static void memPutByte(long ptr, byte value) { UNSAFE.putByte(ptr, value); }
    public static void memPutShort(long ptr, short value) { UNSAFE.putShort(ptr, value); }
    public static void memPutInt(long ptr, int value) { UNSAFE.putInt(ptr, value); }
    public static void memPutFloat(long ptr, float value) { UNSAFE.putFloat(ptr, value); }
    public static void memPutAddress(long ptr, long value) { UNSAFE.putLong(ptr, value); }

    public static String memASCII(long address) {
        return readCString(address, -1, StandardCharsets.US_ASCII);
    }

    public static String memASCII(long address, int length) {
        return readCString(address, length, StandardCharsets.US_ASCII);
    }

    public static String memUTF8(long address) {
        return readCString(address, -1, StandardCharsets.UTF_8);
    }

    public static ByteBuffer memUTF8(CharSequence text) {
        return memUTF8(text, true);
    }

    public static ByteBuffer memUTF8(CharSequence text, boolean nullTerminated) {
        byte[] bytes = (text == null ? "" : text.toString()).getBytes(StandardCharsets.UTF_8);
        ByteBuffer buf = memAlloc(bytes.length + (nullTerminated ? 1 : 0));
        buf.put(bytes);
        if (nullTerminated) buf.put((byte) 0);
        buf.flip();
        return buf;
    }

    private static String readCString(long addr, int len, java.nio.charset.Charset cs) {
        if (addr == 0L) return "";
        if (len >= 0) {
            byte[] b = new byte[len];
            for (int i = 0; i < len; i++) b[i] = UNSAFE.getByte(addr + i);
            return new String(b, cs);
        }
        int n = 0;
        while (UNSAFE.getByte(addr + n) != 0 && n < 65536) n++;
        byte[] b = new byte[n];
        for (int i = 0; i < n; i++) b[i] = UNSAFE.getByte(addr + i);
        return new String(b, cs);
    }

    private static long cachedAddrOffset = -1L;
    private static long cachedCapOffset = -1L;

    private static long addressOffset() {
        long off = cachedAddrOffset;
        if (off >= 0L) return off;
        try {
            off = UNSAFE.objectFieldOffset(Buffer.class.getDeclaredField("address"));
            cachedAddrOffset = off;
            return off;
        } catch (NoSuchFieldException e) {
            throw new InternalError(e);
        }
    }

    private static long capacityOffset() {
        long off = cachedCapOffset;
        if (off >= 0L) return off;
        try {
            off = UNSAFE.objectFieldOffset(Buffer.class.getDeclaredField("capacity"));
            cachedCapOffset = off;
            return off;
        } catch (NoSuchFieldException e) {
            throw new InternalError(e);
        }
    }

    private static int elementSize(Buffer b) {
        if (b instanceof ByteBuffer) return 1;
        if (b instanceof ShortBuffer || b instanceof CharBuffer) return 2;
        if (b instanceof IntBuffer || b instanceof FloatBuffer) return 4;
        if (b instanceof LongBuffer || b instanceof DoubleBuffer) return 8;
        return 1;
    }
}
