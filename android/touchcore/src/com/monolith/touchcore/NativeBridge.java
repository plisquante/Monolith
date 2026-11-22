package com.monolith.touchcore;

import android.view.Surface;

public final class NativeBridge {
    private static volatile boolean sLoaded = false;
    private static volatile String sLoadError = null;

    static {
        try {
            System.loadLibrary("touchcore_mono");
            sLoaded = true;
        } catch (Throwable t) {
            sLoaded = false;
            sLoadError = t.getClass().getSimpleName() + ": " + t.getMessage();
        }
    }

    private NativeBridge() {}

    public static boolean isLoaded() {
        return sLoaded;
    }

    public static String getLoadError() {
        return sLoadError;
    }

    public static String safeProbeHardware() {
        if (!sLoaded) {
            return "Native kütüphane yüklenemedi: " + (sLoadError != null ? sLoadError : "Bilinmeyen hata");
        }
        try {
            return nativeProbeHardware();
        } catch (Throwable t) {
            return "Probe hatası: " + t.getMessage();
        }
    }

    public static String safeGetLastLog() {
        if (!sLoaded) {
            return "Native yükleme hatası: " + (sLoadError != null ? sLoadError : "-");
        }
        try {
            return nativeGetLastLog();
        } catch (Throwable t) {
            return "Log okunamadı: " + t.getMessage();
        }
    }

    public static native String nativeProbeHardware();

    public static native int nativeInitEngine(
            String username,
            String extLibPath,
            String gameDir,
            String resourcesDir,
            String assetsDir,
            int backendPref,
            int powerProfile,
            int renderDist,
            int targetFps,
            float fovDeg);

    public static native void nativeAttachSurface(Surface surface);

    public static native void nativeResizeSurface(int w, int h);
    /** {drawable width, height as the engine uses them, width, height read from the EGL surface (0 before the first swap)} */
    public static native int[] nativeSurfaceSizes();

    public static native void nativeDetachSurface();

    public static native void nativeShutdown();

    public static native void nativeSetKey(int glfwKey, boolean down);

    public static native void nativeSetMoveAxis(float x, float y);

    public static native void nativeAddLookDelta(float dx, float dy);

    public static native void nativeMouseButton(int button, boolean down);

    public static native void nativeSelectHotbar(int slot);

    public static native void nativeSetCursorMode(boolean cursorVisible, float normX, float normY);

    /** A character typed on the keyboard (Unicode code point), delivered to the game as text input. */
    public static native void nativeTypeChar(int codepoint);

    /** An editing key (SDL keycode: backspace, enter, delete, arrows, home, end); action 1 down, 0 up, 2 repeat. */
    public static native void nativeEditKey(int sdlKeycode, int action);

    /** True while the game holds the mouse (playing); false while a screen shows its cursor. */
    public static native boolean nativeIsMouseGrabbed();

    public static native void nativeUpdateRuntimeConfig(
            float sensitivity,
            boolean invertY,
            float fovDeg,
            int renderDist,
            int targetFps);

    public static native void nativePollTelemetry(long[] out16);

    public static native String nativeGetLastLog();
}
