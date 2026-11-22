package org.lwjgl.sdl;

import org.lwjgl.system.MemoryUtil;
import org.lwjgl.system.Struct;

import java.nio.ByteBuffer;

public class SDL_PixelFormatDetails extends Struct<SDL_PixelFormatDetails> {
    public SDL_PixelFormatDetails(long address, ByteBuffer container) {
        super(address, container);
    }

    @Override public int sizeof() { return 32; }
    public byte Rbits() { return 8; }
    public byte Gbits() { return 8; }
    public byte Bbits() { return 8; }
}
