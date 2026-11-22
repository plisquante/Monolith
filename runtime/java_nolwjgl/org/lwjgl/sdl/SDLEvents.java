package org.lwjgl.sdl;

import j2z.rhi.NativeRhi;

public final class SDLEvents {
    public static final int SDL_EVENT_QUIT = 0x100;
    public static final int SDL_EVENT_WINDOW_RESIZED = 0x206;
    public static final int SDL_EVENT_WINDOW_PIXEL_SIZE_CHANGED = 0x207;
    public static final int SDL_EVENT_WINDOW_FOCUS_GAINED = 0x20E;
    public static final int SDL_EVENT_WINDOW_FOCUS_LOST = 0x20F;
    public static final int SDL_EVENT_WINDOW_CLOSE_REQUESTED = 0x210;
    public static final int SDL_EVENT_KEY_DOWN = 0x300;
    public static final int SDL_EVENT_KEY_UP = 0x301;
    public static final int SDL_EVENT_TEXT_EDITING = 0x302;
    public static final int SDL_EVENT_TEXT_INPUT = 0x303;
    public static final int SDL_EVENT_MOUSE_MOTION = 0x400;
    public static final int SDL_EVENT_MOUSE_BUTTON_DOWN = 0x401;
    public static final int SDL_EVENT_MOUSE_BUTTON_UP = 0x402;
    public static final int SDL_EVENT_MOUSE_WHEEL = 0x403;
    public static final int SDL_EVENT_DROP_FILE = 0x1000;

    private static final int MAX_QUEUE = 64;
    private static final int[] qType = new int[MAX_QUEUE];
    private static final int[] qI1 = new int[MAX_QUEUE];
    private static final int[] qI2 = new int[MAX_QUEUE];
    private static final float[] qF1 = new float[MAX_QUEUE];
    private static final float[] qF2 = new float[MAX_QUEUE];
    private static final float[] qF3 = new float[MAX_QUEUE];
    private static final float[] qF4 = new float[MAX_QUEUE];
    private static final String[] qText = new String[MAX_QUEUE];
    private static final int[] textBuf = new int[3 * 16];
    private static int qHead = 0;
    private static int qTail = 0;

    private static final int[] btns = new int[22];
    private static final int[] prevBtns = new int[22];
    private static final float[] axes = new float[4];

    // Map dst[0..16] excluding 7,8 (LMB, RMB) to SDL keycode & scancode.
    private static final int[] BTN_KEYCODE = {
        SDLKeycode.SDLK_W, SDLKeycode.SDLK_A, SDLKeycode.SDLK_S, SDLKeycode.SDLK_D,
        SDLKeycode.SDLK_SPACE, SDLKeycode.SDLK_LSHIFT, SDLKeycode.SDLK_LCTRL,
        0, 0, // LMB (7), RMB (8)
        SDLKeycode.SDLK_ESCAPE, SDLKeycode.SDLK_E, SDLKeycode.SDLK_Q,
        SDLKeycode.SDLK_F, SDLKeycode.SDLK_T, SDLKeycode.SDLK_F3,
        SDLKeycode.SDLK_F5, SDLKeycode.SDLK_TAB
    };
    private static final int[] BTN_SCANCODE = {
        SDLScancode.SDL_SCANCODE_W, SDLScancode.SDL_SCANCODE_A, SDLScancode.SDL_SCANCODE_S, SDLScancode.SDL_SCANCODE_D,
        SDLScancode.SDL_SCANCODE_SPACE, SDLScancode.SDL_SCANCODE_LSHIFT, SDLScancode.SDL_SCANCODE_LCTRL,
        0, 0,
        SDLScancode.SDL_SCANCODE_ESCAPE, SDLScancode.SDL_SCANCODE_E, SDLScancode.SDL_SCANCODE_Q,
        SDLScancode.SDL_SCANCODE_F, SDLScancode.SDL_SCANCODE_T, SDLScancode.SDL_SCANCODE_F3,
        SDLScancode.SDL_SCANCODE_F5, SDLScancode.SDL_SCANCODE_TAB
    };

    private SDLEvents() {}

    /** Debugging aid: J2Z_TRACE_SDL=<file> appends one line per SDL event (the game replaces System.err with its logger). */
    private static final String TRACE_FILE = System.getenv("J2Z_TRACE_SDL");
    private static final boolean TRACE = TRACE_FILE != null;

    private static void trace(String line) {
        try {
            java.nio.file.Files.writeString(java.nio.file.Path.of(TRACE_FILE), line + "\n",
                    java.nio.file.StandardOpenOption.CREATE, java.nio.file.StandardOpenOption.APPEND);
        } catch (java.io.IOException e) {
            throw new IllegalStateException("J2Z_TRACE_SDL: " + e.getMessage(), e);
        }
    }

    private static void enqueue(int type, int i1, int i2, float f1, float f2, float f3, float f4) {
        if (TRACE) trace("SDL event 0x" + Integer.toHexString(type) + " " + i1 + " " + i2 + " " + f1 + " " + f2);
        int next = (qTail + 1) % MAX_QUEUE;
        if (next == qHead) return;
        qType[qTail] = type;
        qI1[qTail] = i1;
        qI2[qTail] = i2;
        qF1[qTail] = f1;
        qF2[qTail] = f2;
        qF3[qTail] = f3;
        qF4[qTail] = f4;
        qText[qTail] = null;
        qTail = next;
    }

    private static void enqueueText(String s) {
        int next = (qTail + 1) % MAX_QUEUE;
        if (next == qHead) return;
        qType[qTail] = SDL_EVENT_TEXT_INPUT;
        qI1[qTail] = 0;
        qI2[qTail] = 0;
        qF1[qTail] = qF2[qTail] = qF3[qTail] = qF4[qTail] = 0f;
        qText[qTail] = s;
        qTail = next;
    }

    // editing keys that only arrive through the text queue (the others have their own entries in BTN_*)
    private static final int[] EDIT_KEYCODE = {8, 13, 127, 0x4000004F, 0x40000050, 0x40000051, 0x40000052, 0x4000004A, 0x4000004D};
    private static final int[] EDIT_SCANCODE = {42, 40, 76, 79, 80, 81, 82, 74, 77};

    private static int editScancode(int keycode) {
        for (int i = 0; i < EDIT_KEYCODE.length; i++) if (EDIT_KEYCODE[i] == keycode) return EDIT_SCANCODE[i];
        return 0;
    }

    public static void SDL_FlushEvents(int minType, int maxType) {
        // SDL semantics: only events with minType <= type <= maxType are dropped (the game flushes the keyboard and
        // mouse range, not the window events such as FOCUS_GAINED).
        int kept = 0;
        int[] idx = new int[MAX_QUEUE];
        for (int i = qHead; i != qTail; i = (i + 1) % MAX_QUEUE) {
            if (qType[i] < minType || qType[i] > maxType) idx[kept++] = i;
        }
        int[] t = new int[kept], a = new int[kept], b = new int[kept];
        String[] tx = new String[kept];
        float[] f1 = new float[kept], f2 = new float[kept], f3 = new float[kept], f4 = new float[kept];
        for (int k = 0; k < kept; k++) {
            int i = idx[k];
            t[k] = qType[i]; a[k] = qI1[i]; b[k] = qI2[i]; tx[k] = qText[i];
            f1[k] = qF1[i]; f2[k] = qF2[i]; f3[k] = qF3[i]; f4[k] = qF4[i];
        }
        for (int k = 0; k < kept; k++) {
            qType[k] = t[k]; qI1[k] = a[k]; qI2[k] = b[k]; qText[k] = tx[k];
            qF1[k] = f1[k]; qF2[k] = f2[k]; qF3[k] = f3[k]; qF4[k] = f4[k];
        }
        qHead = 0;
        qTail = kept;
    }

    public static long SDL_GetWindowFromEvent(SDL_Event event) {
        return event != null ? event.windowId : SDLVideo.activeWindow;
    }

    private static long focusedWindow = 0L;

    private static int pumpTraces;
    private static int sentW, sentH;

    public static void SDL_PumpEvents() {
        if (TRACE && pumpTraces < 5) {
            pumpTraces++;
            trace("SDL pump: activeWindow=" + SDLVideo.activeWindow + " hasOsSurface=" + NativeRhi.hasOsSurface());
        }
        if (SDLVideo.activeWindow == 0L || !NativeRhi.hasOsSurface()) return;
        if (focusedWindow != SDLVideo.activeWindow) {
            // The host window (the Android surface) is always the focused window. Without these events the game
            // believes it is in the background: it ignores the mouse/touch cursor and treats itself as paused.
            focusedWindow = SDLVideo.activeWindow;
            enqueue(SDL_EVENT_WINDOW_FOCUS_GAINED, 0, 0, 0f, 0f, 0f, 0f);
        }
        NativeRhi.pollOsInput(btns, axes);

        int newW = btns[20];
        int newH = btns[21];
        if (newW > 0 && newH > 0) {
            SDLVideo.winWidth = newW;
            SDLVideo.winHeight = newH;
        }
        // The game created its window with the size of its options (854x480) and keeps that as the logical window size
        // (mouse coordinates are scaled by it) until it is told the real one: send RESIZED once at start and on every change.
        if (SDLVideo.winWidth != sentW || SDLVideo.winHeight != sentH) {
            sentW = SDLVideo.winWidth;
            sentH = SDLVideo.winHeight;
            enqueue(SDL_EVENT_WINDOW_RESIZED, sentW, sentH, 0f, 0f, 0f, 0f);
            enqueue(SDL_EVENT_WINDOW_PIXEL_SIZE_CHANGED, sentW, sentH, 0f, 0f, 0f, 0f);
        }

        for (int i = 0; i < BTN_KEYCODE.length; i++) {
            int cur = btns[i];
            int prev = prevBtns[i];
            if (cur != prev) {
                prevBtns[i] = cur;
                if (i == 7 || i == 8) {
                    int sdlBtn = (i == 7) ? SDLMouse.SDL_BUTTON_LEFT : SDLMouse.SDL_BUTTON_RIGHT;
                    int bit = 1 << (sdlBtn - 1);
                    if (cur != 0) SDLMouse.mouseMask |= bit;
                    else SDLMouse.mouseMask &= ~bit;
                    enqueue(cur != 0 ? SDL_EVENT_MOUSE_BUTTON_DOWN : SDL_EVENT_MOUSE_BUTTON_UP, sdlBtn, 0, 0f, 0f, 0f, 0f);
                } else {
                    int sc = BTN_SCANCODE[i];
                    SDLKeyboard.setScancodeState(sc, cur != 0);
                    enqueue(cur != 0 ? SDL_EVENT_KEY_DOWN : SDL_EVENT_KEY_UP, BTN_KEYCODE[i], sc, 0f, 0f, 0f, 0f);
                }
            }
        }

        if (btns[19] != prevBtns[19]) {
            prevBtns[19] = btns[19];
            int slot = btns[19];
            if (slot >= 0 && slot < 9) {
                int kc = SDLKeycode.SDLK_1 + slot;
                int sc = SDLScancode.SDL_SCANCODE_1 + slot;
                enqueue(SDL_EVENT_KEY_DOWN, kc, sc, 0f, 0f, 0f, 0f);
                enqueue(SDL_EVENT_KEY_UP, kc, sc, 0f, 0f, 0f, 0f);
            }
        }

        // characters typed on the soft or hardware keyboard, and editing keys (backspace, enter, arrows...)
        int nt = NativeRhi.pollText(textBuf);
        for (int i = 0; i < nt; i++) {
            int kind = textBuf[3 * i], a = textBuf[3 * i + 1], b = textBuf[3 * i + 2];
            if (kind == 1) {
                if (a > 0 && Character.isValidCodePoint(a)) enqueueText(new String(Character.toChars(a)));
            } else if (kind == 2) {
                int sc = editScancode(a);
                if (sc == 0) continue;
                enqueue(b == 0 ? SDL_EVENT_KEY_UP : SDL_EVENT_KEY_DOWN, a, sc, b == 2 ? 1f : 0f, 0f, 0f, 0f);
            }
        }

        boolean cursorMode = btns[18] != 0;
        if (cursorMode) {
            float cx = axes[2] * SDLVideo.winWidth;
            float cy = axes[3] * SDLVideo.winHeight;
            float dx = cx - SDLMouse.cursorX;
            float dy = cy - SDLMouse.cursorY;
            if (Math.abs(dx) > 0.25f || Math.abs(dy) > 0.25f) {
                SDLMouse.cursorX = cx;
                SDLMouse.cursorY = cy;
                enqueue(SDL_EVENT_MOUSE_MOTION, 0, 0, cx, cy, dx, dy);
            }
        } else if (axes[0] != 0f || axes[1] != 0f) {
            SDLMouse.cursorX += axes[0];
            SDLMouse.cursorY += axes[1];
            enqueue(SDL_EVENT_MOUSE_MOTION, 0, 0, SDLMouse.cursorX, SDLMouse.cursorY, axes[0], axes[1]);
        }
    }

    public static boolean SDL_PollEvent(SDL_Event event) {
        if (qHead == qTail) {
            SDL_PumpEvents();
        }
        if (qHead == qTail) return false;
        int type = qType[qHead];
        int i1 = qI1[qHead];
        int i2 = qI2[qHead];
        float f1 = qF1[qHead];
        float f2 = qF2[qHead];
        float f3 = qF3[qHead];
        float f4 = qF4[qHead];
        String text = qText[qHead];
        qHead = (qHead + 1) % MAX_QUEUE;

        if (event != null) {
            event.type = type;
            event.windowId = SDLVideo.activeWindow;
            if (type >= 0x200 && type <= 0x2FF) { // every SDL_EVENT_WINDOW_* carries its window id (Window.handleEvent compares it)
                event.window().windowID = SDLVideo.activeWindow;
                event.window().data1 = i1;
                event.window().data2 = i2;
            } else if (type == SDL_EVENT_KEY_DOWN || type == SDL_EVENT_KEY_UP) {
                event.key().key = i1;
                event.key().scancode = i2;
                event.key().mod = 0;
                event.key().repeat = f1 != 0f;
            } else if (type == SDL_EVENT_TEXT_INPUT) {
                event.text().text = text != null ? text : "";
            } else if (type == SDL_EVENT_MOUSE_BUTTON_DOWN || type == SDL_EVENT_MOUSE_BUTTON_UP) {
                event.button().button = (byte) i1;
            } else if (type == SDL_EVENT_MOUSE_MOTION) {
                event.motion().x = f1;
                event.motion().y = f2;
                event.motion().xrel = f3;
                event.motion().yrel = f4;
            }
        }
        return true;
    }
}
