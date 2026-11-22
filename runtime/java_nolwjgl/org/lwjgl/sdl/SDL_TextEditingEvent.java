package org.lwjgl.sdl;

public class SDL_TextEditingEvent {
    int start;
    int length;
    String text = "";

    public int length() { return length; }
    public int start() { return start; }
    public String textString() { return text; }
}
