package org.lwjgl.system;

import org.lwjgl.PointerBuffer;

import java.nio.ByteBuffer;
import java.nio.ByteOrder;
import java.nio.FloatBuffer;
import java.nio.IntBuffer;
import java.nio.LongBuffer;
import java.nio.charset.StandardCharsets;

/** Zero-LWJGL thread-local stack allocator implementing full MemoryStack signatures. */
public class MemoryStack implements AutoCloseable {
    private static final int STACK_BYTES = 128 * 1024;
    private static final ThreadLocal<MemoryStack> TLS = ThreadLocal.withInitial(MemoryStack::new);

    private final ByteBuffer backing;
    private final int[] frames = new int[64];
    private int frameTop = 0;
    private int pointer = 0;

    private MemoryStack() {
        this.backing = ByteBuffer.allocateDirect(STACK_BYTES).order(ByteOrder.nativeOrder());
    }

    public static MemoryStack stackGet() {
        return TLS.get();
    }

    public static MemoryStack stackPush() {
        return TLS.get().push();
    }

    public static void stackPop() {
        TLS.get().pop();
    }

    public int getPointer() {
        return pointer;
    }

    public MemoryStack push() {
        if (frameTop < frames.length) {
            frames[frameTop++] = pointer;
        }
        return this;
    }

    public MemoryStack pop() {
        if (frameTop > 0) {
            pointer = frames[--frameTop];
        }
        return this;
    }

    @Override
    public void close() {
        pop();
    }

    public long ncalloc(int alignment, int num, int size) {
        ByteBuffer b = calloc(alignment, num * size);
        return MemoryUtil.memAddress(b);
    }

    public ByteBuffer malloc(int size) {
        return malloc(1, size);
    }

    public ByteBuffer calloc(int size) {
        return calloc(1, size);
    }

    public ByteBuffer calloc(int alignment, int size) {
        ByteBuffer b = malloc(alignment, size);
        for (int i = 0; i < size; i++) b.put(i, (byte) 0);
        return b;
    }

    public ByteBuffer malloc(int alignment, int size) {
        int a = Math.max(1, alignment);
        int aligned = (pointer + (a - 1)) & ~(a - 1);
        if (aligned + size > STACK_BYTES) {
            return ByteBuffer.allocateDirect(Math.max(size, 0)).order(ByteOrder.nativeOrder());
        }
        pointer = aligned + size;
        backing.limit(pointer);
        backing.position(aligned);
        ByteBuffer slice = backing.slice().order(ByteOrder.nativeOrder());
        backing.clear();
        return slice;
    }

    public IntBuffer mallocInt(int size) { return malloc(4, size << 2).asIntBuffer(); }
    public IntBuffer callocInt(int size) { return calloc(4, size << 2).asIntBuffer(); }
    public IntBuffer ints(int x) { IntBuffer b = mallocInt(1); b.put(0, x); return b; }
    public IntBuffer ints(int x, int y) { IntBuffer b = mallocInt(2); b.put(0, x).put(1, y); return b; }

    public FloatBuffer mallocFloat(int size) { return malloc(4, size << 2).asFloatBuffer(); }
    public FloatBuffer callocFloat(int size) { return calloc(4, size << 2).asFloatBuffer(); }
    public FloatBuffer floats(float x, float y, float z, float w) {
        FloatBuffer b = mallocFloat(4);
        b.put(0, x).put(1, y).put(2, z).put(3, w);
        return b;
    }

    public LongBuffer mallocLong(int size) { return malloc(8, size << 3).asLongBuffer(); }
    public LongBuffer callocLong(int size) { return calloc(8, size << 3).asLongBuffer(); }
    public LongBuffer longs(long x) { LongBuffer b = mallocLong(1); b.put(0, x); return b; }

    public PointerBuffer mallocPointer(int size) {
        ByteBuffer b = malloc(Pointer.POINTER_SIZE, size * Pointer.POINTER_SIZE);
        return PointerBuffer.create(b);
    }

    public PointerBuffer callocPointer(int size) {
        ByteBuffer b = calloc(Pointer.POINTER_SIZE, size * Pointer.POINTER_SIZE);
        return PointerBuffer.create(b);
    }

    public ByteBuffer ASCII(CharSequence text) {
        byte[] bytes = (text == null ? "" : text.toString()).getBytes(StandardCharsets.US_ASCII);
        ByteBuffer b = malloc(1, bytes.length + 1);
        b.put(bytes).put((byte) 0);
        b.flip();
        return b;
    }

    public ByteBuffer UTF8(CharSequence text) {
        return UTF8(text, true);
    }

    public ByteBuffer UTF8(CharSequence text, boolean nullTerminated) {
        byte[] bytes = (text == null ? "" : text.toString()).getBytes(StandardCharsets.UTF_8);
        ByteBuffer b = malloc(1, bytes.length + (nullTerminated ? 1 : 0));
        b.put(bytes);
        if (nullTerminated) b.put((byte) 0);
        b.flip();
        return b;
    }
}
