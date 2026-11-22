package org.lwjgl.sdl;

public class SDL_WindowEvent {
    long windowID;
    int data1;
    int data2;

    public int data1() { return data1; }
    public int data2() { return data2; }
}
