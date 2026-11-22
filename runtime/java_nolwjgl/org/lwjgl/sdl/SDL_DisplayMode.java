package org.lwjgl.sdl;

import org.lwjgl.system.MemoryStack;
import org.lwjgl.system.MemoryUtil;
import org.lwjgl.system.Struct;

import java.nio.ByteBuffer;

public class SDL_DisplayMode extends Struct<SDL_DisplayMode> {
    public static final int SIZEOF = 32;

    public SDL_DisplayMode(long address, ByteBuffer container) {
        super(address, container);
    }

    @Override
    public int sizeof() { return SIZEOF; }

    public static SDL_DisplayMode malloc(MemoryStack stack) {
        ByteBuffer b = stack.calloc(8, SIZEOF);
        return new SDL_DisplayMode(MemoryUtil.memAddress(b), b);
    }

    public static SDL_DisplayMode create(long address) {
        return address == 0L ? null : new SDL_DisplayMode(address, null);
    }

    public int format() { return MemoryUtil.memGetInt(address + 4); }
    public int w() { return MemoryUtil.memGetInt(address + 8); }
    public int h() { return MemoryUtil.memGetInt(address + 12); }
    public float refresh_rate() { return Float.intBitsToFloat(MemoryUtil.memGetInt(address + 20)); }

    public void set(int format, int w, int h, float refresh) {
        MemoryUtil.memPutInt(address + 4, format);
        MemoryUtil.memPutInt(address + 8, w);
        MemoryUtil.memPutInt(address + 12, h);
        MemoryUtil.memPutInt(address + 20, Float.floatToRawIntBits(refresh));
    }
}
