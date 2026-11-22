package org.lwjgl.sdl;

import mono.rhi.NativeRhi;
import org.lwjgl.PointerBuffer;
import org.lwjgl.system.MemoryUtil;

import java.nio.ByteBuffer;
import java.nio.IntBuffer;

public final class SDLVideo {
    static long activeWindow = 0L;
    static long activeContext = 0L;
    static int winWidth = 1280;
    static int winHeight = 720;
    static boolean fullscreen = false;
    static boolean relativeMouse = false;
    private static final int[] glAttrs = new int[32];

    private static final ByteBuffer DEFAULT_MODE_BUF = MemoryUtil.memCalloc(SDL_DisplayMode.SIZEOF);
    private static final SDL_DisplayMode DEFAULT_MODE =
            new SDL_DisplayMode(MemoryUtil.memAddress(DEFAULT_MODE_BUF), DEFAULT_MODE_BUF);

    static {
        DEFAULT_MODE.set(0x16462004, winWidth, winHeight, 60.0f); // SDL_PIXELFORMAT_ABGR8888
    }

    private SDLVideo() {}

    static void resetAll() {
        activeContext = 0L;
        activeWindow = 0L;
    }

    private static void syncDisplayMode() {
        DEFAULT_MODE.set(0x16462004, winWidth, winHeight, 60.0f);
    }

    public static long SDL_CreateWindow(CharSequence title, int w, int h, long flags) {
        SDLInit.SDL_Init(SDLInit.SDL_INIT_VIDEO);
        boolean attached = NativeRhi.attachOsSurface();
        if (!attached && !"1".equals(System.getenv("MONO_HEADLESS_WINDOW"))) {
            SDLError.setError("monolith --no-lwjgl: native OS window creation requires an attached ANativeWindow/EGL surface");
            return 0L;
        }
        activeWindow = 1L;
        int osW = attached ? NativeRhi.osSurfaceWidth() : 0;
        int osH = attached ? NativeRhi.osSurfaceHeight() : 0;
        winWidth = osW > 0 ? osW : Math.max(w, 1);
        winHeight = osH > 0 ? osH : Math.max(h, 1);
        syncDisplayMode();
        SDLMouse.cursorX = winWidth * 0.5f;
        SDLMouse.cursorY = winHeight * 0.5f;
        return activeWindow;
    }

    public static void SDL_DestroyWindow(long window) {
        if (window == activeWindow) activeWindow = 0L;
    }

    public static long SDL_GL_CreateContext(long window) {
        if (window == 0L) return 0L;
        if (NativeRhi.hasOsSurface()) {
            NativeRhi.attachOsSurface();
        }
        NativeRhi.beginFrame(winWidth, winHeight);
        activeContext = 1L;
        return activeContext;
    }

    public static boolean SDL_GL_DestroyContext(long context) {
        if (context == activeContext) activeContext = 0L;
        return true;
    }

    public static boolean SDL_GL_GetAttribute(int attr, IntBuffer value) {
        if (value != null && value.remaining() > 0) {
            int idx = (attr >= 0 && attr < glAttrs.length) ? attr : 0;
            value.put(value.position(), glAttrs[idx]);
        }
        return true;
    }

    public static long SDL_GL_GetProcAddress(CharSequence proc) { return 1L; }
    public static boolean SDL_GL_LoadLibrary(CharSequence path) { return true; }

    public static boolean SDL_GL_MakeCurrent(long window, long context) {
        if (window != 0L && NativeRhi.hasOsSurface()) {
            NativeRhi.attachOsSurface();
        }
        return window != 0L && context != 0L;
    }

    public static boolean SDL_GL_SetAttribute(int attr, int value) {
        if (attr >= 0 && attr < glAttrs.length) glAttrs[attr] = value;
        return true;
    }

    public static boolean SDL_GL_SetSwapInterval(int interval) { return true; }

    public static boolean SDL_GL_SwapWindow(long window) {
        if (window == 0L) return false;
        NativeRhi.swapOsSurface();
        NativeRhi.endFrame();
        NativeRhi.beginFrame(winWidth, winHeight);
        return true;
    }

    public static void SDL_GL_UnloadLibrary() {}

    public static boolean SDL_GetClosestFullscreenDisplayMode(
            int displayID, int w, int h, float refresh_rate, boolean include_high_density_modes, SDL_DisplayMode closest) {
        syncDisplayMode();
        if (closest != null) closest.set(0x16462004, winWidth, winHeight, 60.0f);
        return true;
    }

    public static SDL_DisplayMode SDL_GetCurrentDisplayMode(int displayID) {
        syncDisplayMode();
        return DEFAULT_MODE;
    }

    public static String SDL_GetCurrentVideoDriver() {
        return NativeRhi.hasOsSurface() ? "android" : "headless";
    }

    public static SDL_DisplayMode SDL_GetDesktopDisplayMode(int displayID) {
        syncDisplayMode();
        return DEFAULT_MODE;
    }

    public static boolean SDL_GetDisplayBounds(int displayID, SDL_Rect rect) {
        if (rect != null) {
            rect.x(0).y(0).w(winWidth).h(winHeight);
        }
        return true;
    }

    public static int SDL_GetDisplayForWindow(long window) { return 1; }
    public static String SDL_GetDisplayName(int displayID) { return "TouchCore Primary Display"; }

    public static IntBuffer SDL_GetDisplays() {
        IntBuffer b = MemoryUtil.memAllocInt(1);
        b.put(0, 1);
        return b;
    }

    public static PointerBuffer SDL_GetFullscreenDisplayModes(int displayID) {
        syncDisplayMode();
        PointerBuffer pb = MemoryUtil.memAllocPointer(1);
        pb.put(0, DEFAULT_MODE.address());
        return pb;
    }

    public static int SDL_GetPrimaryDisplay() { return 1; }
    /** SDL_WINDOW_OPENGL | SDL_WINDOW_INPUT_FOCUS | SDL_WINDOW_MOUSE_FOCUS (+ fullscreen): the host window always has the focus. */
    public static long SDL_GetWindowFlags(long window) { return 0x00000002L | 0x00000200L | 0x00000400L | (fullscreen ? 0x00000001L : 0L); }
    public static SDL_DisplayMode SDL_GetWindowFullscreenMode(long window) { return DEFAULT_MODE; }
    public static float SDL_GetWindowPixelDensity(long window) { return 1.0f; }

    public static boolean SDL_GetWindowPosition(long window, IntBuffer x, IntBuffer y) {
        if (x != null && x.remaining() > 0) x.put(x.position(), 0);
        if (y != null && y.remaining() > 0) y.put(y.position(), 0);
        return true;
    }

    public static boolean SDL_GetWindowSizeInPixels(long window, IntBuffer w, IntBuffer h) {
        if (NativeRhi.hasOsSurface()) {
            int ow = NativeRhi.osSurfaceWidth();
            int oh = NativeRhi.osSurfaceHeight();
            if (ow > 0) winWidth = ow;
            if (oh > 0) winHeight = oh;
        }
        if (w != null && w.remaining() > 0) w.put(w.position(), winWidth);
        if (h != null && h.remaining() > 0) h.put(h.position(), winHeight);
        return true;
    }

    public static boolean SDL_RestoreWindow(long window) { return window != 0L; }
    public static boolean SDL_SetWindowBordered(long window, boolean bordered) { return true; }
    public static boolean SDL_SetWindowFullscreen(long window, boolean fs) { fullscreen = fs; return true; }
    public static boolean SDL_SetWindowFullscreenMode(long window, SDL_DisplayMode mode) { return true; }
    public static boolean SDL_SetWindowIcon(long window, SDL_Surface icon) { return true; }
    public static boolean SDL_SetWindowMaximumSize(long window, int max_w, int max_h) { return true; }
    public static boolean SDL_SetWindowMinimumSize(long window, int min_w, int min_h) { return true; }
    public static boolean SDL_SetWindowMouseGrab(long window, boolean grabbed) { relativeMouse = grabbed; mono.rhi.NativeRhi.setMouseGrabbed(grabbed); return true; }
    public static boolean SDL_SetWindowPosition(long window, int x, int y) { return true; }

    public static boolean SDL_SetWindowSize(long window, int w, int h) {
        if (w > 0 && h > 0 && !NativeRhi.hasOsSurface()) {
            winWidth = w;
            winHeight = h;
            syncDisplayMode();
        }
        return true;
    }

    public static boolean SDL_SetWindowTitle(long window, CharSequence title) { return true; }
    public static boolean SDL_SyncWindow(long window) { return true; }
}
