package org.lwjgl.sdl;

public class SDL_KeyboardEvent {
    int key;
    int scancode;
    short mod;
    boolean repeat;

    public int key() { return key; }
    public short mod() { return mod; }
    public boolean repeat() { return repeat; }
    public int scancode() { return scancode; }
}
