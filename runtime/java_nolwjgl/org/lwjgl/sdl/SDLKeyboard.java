package org.lwjgl.sdl;

import org.lwjgl.system.MemoryUtil;

import java.nio.ByteBuffer;

public final class SDLKeyboard {
    private static final int NUM_SCANCODES = 512;
    private static final ByteBuffer KEY_STATE = MemoryUtil.memCalloc(NUM_SCANCODES);

    private SDLKeyboard() {}

    static void setScancodeState(int scancode, boolean pressed) {
        if (scancode >= 0 && scancode < NUM_SCANCODES) {
            KEY_STATE.put(scancode, pressed ? (byte) 1 : (byte) 0);
        }
    }

    public static boolean SDL_ClearComposition(long window) { return true; }

    public static int SDL_GetKeyFromScancode(int scancode, short modstate, boolean key_event) {
        switch (scancode) {
            case SDLScancode.SDL_SCANCODE_W: return SDLKeycode.SDLK_W;
            case SDLScancode.SDL_SCANCODE_A: return SDLKeycode.SDLK_A;
            case SDLScancode.SDL_SCANCODE_S: return SDLKeycode.SDLK_S;
            case SDLScancode.SDL_SCANCODE_D: return SDLKeycode.SDLK_D;
            case SDLScancode.SDL_SCANCODE_SPACE: return SDLKeycode.SDLK_SPACE;
            case SDLScancode.SDL_SCANCODE_LSHIFT: return SDLKeycode.SDLK_LSHIFT;
            case SDLScancode.SDL_SCANCODE_LCTRL: return SDLKeycode.SDLK_LCTRL;
            case SDLScancode.SDL_SCANCODE_ESCAPE: return SDLKeycode.SDLK_ESCAPE;
            case SDLScancode.SDL_SCANCODE_E: return SDLKeycode.SDLK_E;
            case SDLScancode.SDL_SCANCODE_Q: return SDLKeycode.SDLK_Q;
            case SDLScancode.SDL_SCANCODE_F: return SDLKeycode.SDLK_F;
            case SDLScancode.SDL_SCANCODE_T: return SDLKeycode.SDLK_T;
            case SDLScancode.SDL_SCANCODE_F3: return SDLKeycode.SDLK_F3;
            case SDLScancode.SDL_SCANCODE_F5: return SDLKeycode.SDLK_F5;
            case SDLScancode.SDL_SCANCODE_TAB: return SDLKeycode.SDLK_TAB;
            default:
                if (scancode >= SDLScancode.SDL_SCANCODE_1 && scancode < SDLScancode.SDL_SCANCODE_1 + 9) {
                    return SDLKeycode.SDLK_1 + (scancode - SDLScancode.SDL_SCANCODE_1);
                }
                return scancode;
        }
    }

    public static String SDL_GetKeyName(int key) {
        if (key >= 33 && key <= 126) return String.valueOf((char) key);
        switch (key) {
            case SDLKeycode.SDLK_SPACE: return "Space";
            case SDLKeycode.SDLK_ESCAPE: return "Escape";
            case SDLKeycode.SDLK_TAB: return "Tab";
            case SDLKeycode.SDLK_LSHIFT: return "Left Shift";
            case SDLKeycode.SDLK_LCTRL: return "Left Ctrl";
            case SDLKeycode.SDLK_F3: return "F3";
            case SDLKeycode.SDLK_F5: return "F5";
            default: return "Key" + key;
        }
    }

    public static ByteBuffer SDL_GetKeyboardState() {
        return KEY_STATE.duplicate();
    }

    public static short SDL_GetModState() { return 0; }
    public static boolean SDL_SetTextInputArea(long window, SDL_Rect.Buffer rect, int cursor) { return true; }
    public static boolean SDL_StartTextInput(long window) { return true; }
    public static boolean SDL_StopTextInput(long window) { return true; }
}
