package org.lwjgl.sdl;

import org.lwjgl.system.MemoryUtil;
import org.lwjgl.system.Struct;

import java.nio.ByteBuffer;

public class SDL_Surface extends Struct<SDL_Surface> {
    private int width;
    private int height;

    public SDL_Surface(long address, ByteBuffer container, int w, int h) {
        super(address, container);
        this.width = w;
        this.height = h;
    }

    @Override
    public int sizeof() { return 64; }
    public int w() { return width; }
    public int h() { return height; }
}
