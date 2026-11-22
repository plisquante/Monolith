package org.lwjgl.sdl;

import org.lwjgl.system.MemoryStack;
import org.lwjgl.system.MemoryUtil;
import org.lwjgl.system.Struct;

import java.nio.ByteBuffer;

public class SDL_MessageBoxData extends Struct<SDL_MessageBoxData> {
    public static final int SIZEOF = 64;

    public SDL_MessageBoxData(long address, ByteBuffer container) {
        super(address, container);
    }

    @Override public int sizeof() { return SIZEOF; }

    public static SDL_MessageBoxData calloc(MemoryStack stack) {
        ByteBuffer b = stack.calloc(8, SIZEOF);
        return new SDL_MessageBoxData(MemoryUtil.memAddress(b), b);
    }

    public SDL_MessageBoxData flags(int value) { return this; }
    public SDL_MessageBoxData title(ByteBuffer value) { return this; }
    public SDL_MessageBoxData message(ByteBuffer value) { return this; }
    public SDL_MessageBoxData buttons(SDL_MessageBoxButtonData.Buffer value) { return this; }
}
