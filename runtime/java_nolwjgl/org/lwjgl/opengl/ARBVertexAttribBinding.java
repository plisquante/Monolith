package org.lwjgl.opengl;

/**
 * Software emulation of ARB_vertex_attrib_binding on top of GL 3.3 Core / GLES 3.0
 * (`glBindBuffer` + `glVertexAttribPointer` / `glVertexAttribIPointer` / `glVertexAttribDivisor`).
 */
public final class ARBVertexAttribBinding {
    private static final int MAX_SLOTS = 32;
    private static final int[] bindingBuffer = new int[MAX_SLOTS];
    private static final long[] bindingOffset = new long[MAX_SLOTS];
    private static final int[] bindingStride = new int[MAX_SLOTS];
    private static final int[] attribBinding = new int[MAX_SLOTS];
    private static final int[] attribSize = new int[MAX_SLOTS];
    private static final int[] attribType = new int[MAX_SLOTS];
    private static final boolean[] attribNorm = new boolean[MAX_SLOTS];
    private static final int[] attribRelOff = new int[MAX_SLOTS];
    private static final boolean[] attribIsInt = new boolean[MAX_SLOTS];
    private static final boolean[] attribHasFmt = new boolean[MAX_SLOTS];

    static {
        for (int i = 0; i < MAX_SLOTS; i++) attribBinding[i] = i;
    }

    private ARBVertexAttribBinding() {}

    private static void applySlot(int attrib) {
        if (attrib < 0 || attrib >= MAX_SLOTS || !attribHasFmt[attrib]) return;
        int bind = attribBinding[attrib];
        if (bind < 0 || bind >= MAX_SLOTS) return;
        GL33C.glBindBuffer(GL15C.GL_ARRAY_BUFFER, bindingBuffer[bind]);
        long ptr = bindingOffset[bind] + (long) attribRelOff[attrib];
        int stride = bindingStride[bind];
        if (attribIsInt[attrib]) {
            GL30C.glVertexAttribIPointer(attrib, attribSize[attrib], attribType[attrib], stride, ptr);
        } else {
            GL20C.glVertexAttribPointer(attrib, attribSize[attrib], attribType[attrib], attribNorm[attrib], stride, ptr);
        }
    }

    public static void glBindVertexBuffer(int bindingindex, int buffer, long offset, int stride) {
        if (bindingindex < 0 || bindingindex >= MAX_SLOTS) return;
        bindingBuffer[bindingindex] = buffer;
        bindingOffset[bindingindex] = offset;
        bindingStride[bindingindex] = stride;
        for (int a = 0; a < MAX_SLOTS; a++) {
            if (attribBinding[a] == bindingindex && attribHasFmt[a]) applySlot(a);
        }
    }

    public static void glVertexAttribBinding(int attribindex, int bindingindex) {
        if (attribindex < 0 || attribindex >= MAX_SLOTS) return;
        attribBinding[attribindex] = bindingindex;
        applySlot(attribindex);
    }

    public static void glVertexAttribFormat(int attribindex, int size, int type, boolean normalized, int relativeoffset) {
        if (attribindex < 0 || attribindex >= MAX_SLOTS) return;
        attribSize[attribindex] = size;
        attribType[attribindex] = type;
        attribNorm[attribindex] = normalized;
        attribRelOff[attribindex] = relativeoffset;
        attribIsInt[attribindex] = false;
        attribHasFmt[attribindex] = true;
        applySlot(attribindex);
    }

    public static void glVertexAttribIFormat(int attribindex, int size, int type, int relativeoffset) {
        if (attribindex < 0 || attribindex >= MAX_SLOTS) return;
        attribSize[attribindex] = size;
        attribType[attribindex] = type;
        attribRelOff[attribindex] = relativeoffset;
        attribIsInt[attribindex] = true;
        attribHasFmt[attribindex] = true;
        applySlot(attribindex);
    }

    public static void glVertexBindingDivisor(int bindingindex, int divisor) {
        for (int a = 0; a < MAX_SLOTS; a++) {
            if (attribBinding[a] == bindingindex) {
                GL33C.glVertexAttribDivisor(a, divisor);
            }
        }
    }
}
