package org.lwjgl.sdl;

public class SDL_Event implements AutoCloseable {
    int type;
    long windowId;
    private final SDL_WindowEvent windowEv = new SDL_WindowEvent();
    private final SDL_KeyboardEvent keyEv = new SDL_KeyboardEvent();
    private final SDL_MouseMotionEvent motionEv = new SDL_MouseMotionEvent();
    private final SDL_MouseButtonEvent buttonEv = new SDL_MouseButtonEvent();
    private final SDL_MouseWheelEvent wheelEv = new SDL_MouseWheelEvent();
    private final SDL_TextInputEvent textEv = new SDL_TextInputEvent();
    private final SDL_TextEditingEvent editEv = new SDL_TextEditingEvent();
    private final SDL_DropEvent dropEv = new SDL_DropEvent();
    private final SDL_DisplayEvent displayEv = new SDL_DisplayEvent();

    public static SDL_Event malloc() { return new SDL_Event(); }

    @Override
    public void close() {}

    public int type() { return type; }
    public SDL_WindowEvent window() { return windowEv; }
    public SDL_KeyboardEvent key() { return keyEv; }
    public SDL_MouseMotionEvent motion() { return motionEv; }
    public SDL_MouseButtonEvent button() { return buttonEv; }
    public SDL_MouseWheelEvent wheel() { return wheelEv; }
    public SDL_TextInputEvent text() { return textEv; }
    public SDL_TextEditingEvent edit() { return editEv; }
    public SDL_DropEvent drop() { return dropEv; }
    public SDL_DisplayEvent display() { return displayEv; }
}
