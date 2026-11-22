package org.lwjgl.glfw;

import mono.rhi.NativeRhi;

/**
 * Zero-LWJGL GLFW adapter bound to TouchCore's Android ANativeWindow + g_tc_input surface,
 * with RHI frame lifecycle management.
 */
public final class GLFW {
    public static final int GLFW_TRUE = 1;
    public static final int GLFW_FALSE = 0;
    public static final int GLFW_PRESS = 1;
    public static final int GLFW_RELEASE = 0;
    public static final int GLFW_CURSOR = 0x00033001;
    public static final int GLFW_CURSOR_NORMAL = 0x00034001;
    public static final int GLFW_CURSOR_DISABLED = 0x00034003;

    private static final long START_NS = System.nanoTime();
    private static boolean initialized = false;
    private static boolean shouldClose = false;
    private static long activeWindow = 0L;
    private static int winWidth = 1280;
    private static int winHeight = 720;

    private static GLFWKeyCallbackI keyCb;
    private static GLFWMouseButtonCallbackI mouseCb;
    private static GLFWCursorPosCallbackI cursorCb;
    private static GLFWFramebufferSizeCallbackI fbCb;
    private static GLFWErrorCallbackI errCb;
    private static GLFWCharCallbackI charCb;
    private static GLFWScrollCallbackI scrollCb;
    private static GLFWWindowFocusCallbackI focusCb;

    private static final int[] btns = new int[22];
    private static final int[] prevBtns = new int[22];
    private static final float[] axes = new float[4];
    private static final int[] BTN_GLFW_KEYS = {
        87, 65, 83, 68, 32, 340, 341, // W A S D SPACE LSHIFT LCTRL
        -1, -2,                       // LMB, RMB handled via mouseCb
        256, 69, 81, 70, 84, 292, 294, 258 // ESC E Q F T F3 F5 TAB
    };
    private static double virtCursorX = 640.0;
    private static double virtCursorY = 360.0;

    private GLFW() {}

    public static boolean glfwInit() {
        if (initialized) return true;
        String mode = System.getProperty("mono.graphics.backend", "auto");
        int b = NativeRhi.BACKEND_AUTO;
        if ("vulkan".equalsIgnoreCase(mode)) b = NativeRhi.BACKEND_VULKAN;
        else if ("opengl".equalsIgnoreCase(mode)) b = NativeRhi.BACKEND_OPENGL33;
        initialized = NativeRhi.init(b, NativeRhi.POWER_BALANCED) >= 0;
        return initialized;
    }

    public static void glfwWindowHint(int hint, int value) {}
    public static void glfwDefaultWindowHints() {}
    public static long glfwGetPrimaryMonitor() { return 1L; }
    public static GLFWVidMode glfwGetVideoMode(long monitor) {
        int w = NativeRhi.hasOsSurface() ? Math.max(NativeRhi.osSurfaceWidth(), 1) : winWidth;
        int h = NativeRhi.hasOsSurface() ? Math.max(NativeRhi.osSurfaceHeight(), 1) : winHeight;
        return new GLFWVidMode(w, h, 120);
    }
    public static void glfwShowWindow(long window) {}
    public static void glfwSetWindowTitle(long window, CharSequence title) {}

    public static long glfwCreateWindow(int width, int height, CharSequence title, long monitor, long share) {
        if (!initialized) glfwInit();
        boolean attached = NativeRhi.attachOsSurface();
        if (!attached) {
            if ("1".equals(System.getenv("MONO_HEADLESS_WINDOW"))) {
                activeWindow = 1L;
                winWidth = Math.max(width, 1);
                winHeight = Math.max(height, 1);
                return activeWindow;
            }
            throw new UnsupportedOperationException(
                    "monolith --no-lwjgl: native OS window creation requires an attached ANativeWindow/EGL surface (run inside TouchCore APK or X11/Wayland adapter)");
        }
        activeWindow = 1L;
        int osW = NativeRhi.osSurfaceWidth();
        int osH = NativeRhi.osSurfaceHeight();
        winWidth = osW > 0 ? osW : Math.max(width, 1);
        winHeight = osH > 0 ? osH : Math.max(height, 1);
        virtCursorX = winWidth * 0.5;
        virtCursorY = winHeight * 0.5;
        return activeWindow;
    }

    public static void glfwMakeContextCurrent(long window) {
        if (window != 0L) {
            NativeRhi.attachOsSurface();
            NativeRhi.beginFrame(winWidth, winHeight);
        }
    }

    public static void glfwSwapInterval(int interval) {}

    public static void glfwSwapBuffers(long window) {
        NativeRhi.swapOsSurface();
        NativeRhi.endFrame();
        NativeRhi.beginFrame(winWidth, winHeight);
    }

    public static void glfwPollEvents() {
        if (activeWindow == 0L || !NativeRhi.hasOsSurface()) return;
        NativeRhi.pollOsInput(btns, axes);

        int newW = btns[20];
        int newH = btns[21];
        if (newW > 0 && newH > 0 && (newW != winWidth || newH != winHeight)) {
            winWidth = newW;
            winHeight = newH;
            if (fbCb != null) fbCb.invoke(activeWindow, winWidth, winHeight);
        }

        for (int i = 0; i < BTN_GLFW_KEYS.length; i++) {
            int cur = btns[i];
            int prev = prevBtns[i];
            if (cur != prev) {
                prevBtns[i] = cur;
                int action = (cur != 0) ? GLFW_PRESS : GLFW_RELEASE;
                int code = BTN_GLFW_KEYS[i];
                if (code < 0) {
                    if (mouseCb != null) mouseCb.invoke(activeWindow, (-code) - 1, action, 0);
                } else {
                    if (keyCb != null) keyCb.invoke(activeWindow, code, 0, action, 0);
                }
            }
        }

        if (btns[19] != prevBtns[19]) {
            prevBtns[19] = btns[19];
            if (keyCb != null && btns[19] >= 0 && btns[19] < 9) {
                int slotKey = 49 + btns[19];
                keyCb.invoke(activeWindow, slotKey, 0, GLFW_PRESS, 0);
                keyCb.invoke(activeWindow, slotKey, 0, GLFW_RELEASE, 0);
            }
        }

        boolean cursorMode = btns[18] != 0;
        if (cursorMode) {
            double cx = axes[2] * winWidth;
            double cy = axes[3] * winHeight;
            if (Math.abs(cx - virtCursorX) > 0.25 || Math.abs(cy - virtCursorY) > 0.25) {
                virtCursorX = cx;
                virtCursorY = cy;
                if (cursorCb != null) cursorCb.invoke(activeWindow, virtCursorX, virtCursorY);
            }
        } else if (axes[0] != 0f || axes[1] != 0f) {
            virtCursorX += axes[0];
            virtCursorY += axes[1];
            if (cursorCb != null) cursorCb.invoke(activeWindow, virtCursorX, virtCursorY);
        }
    }

    public static void glfwGetFramebufferSize(long window, int[] w, int[] h) {
        if (w != null && w.length > 0) w[0] = winWidth;
        if (h != null && h.length > 0) h[0] = winHeight;
    }

    public static void glfwGetWindowSize(long window, int[] w, int[] h) {
        glfwGetFramebufferSize(window, w, h);
    }

    public static double glfwGetTime() {
        return (System.nanoTime() - START_NS) / 1_000_000_000.0;
    }

    public static boolean glfwWindowShouldClose(long window) {
        return shouldClose;
    }

    public static void glfwSetWindowShouldClose(long window, boolean value) {
        shouldClose = value;
    }

    public static void glfwSetInputMode(long window, int mode, int value) {}

    public static GLFWKeyCallbackI glfwSetKeyCallback(long window, GLFWKeyCallbackI cbfun) {
        GLFWKeyCallbackI old = keyCb;
        keyCb = cbfun;
        return old;
    }

    public static GLFWMouseButtonCallbackI glfwSetMouseButtonCallback(long window, GLFWMouseButtonCallbackI cbfun) {
        GLFWMouseButtonCallbackI old = mouseCb;
        mouseCb = cbfun;
        return old;
    }

    public static GLFWCursorPosCallbackI glfwSetCursorPosCallback(long window, GLFWCursorPosCallbackI cbfun) {
        GLFWCursorPosCallbackI old = cursorCb;
        cursorCb = cbfun;
        return old;
    }

    public static GLFWFramebufferSizeCallbackI glfwSetFramebufferSizeCallback(long window, GLFWFramebufferSizeCallbackI cbfun) {
        GLFWFramebufferSizeCallbackI old = fbCb;
        fbCb = cbfun;
        return old;
    }

    public static GLFWErrorCallbackI glfwSetErrorCallback(GLFWErrorCallbackI cbfun) {
        GLFWErrorCallbackI old = errCb;
        errCb = cbfun;
        return old;
    }

    public static GLFWCharCallbackI glfwSetCharCallback(long window, GLFWCharCallbackI cbfun) {
        GLFWCharCallbackI old = charCb;
        charCb = cbfun;
        return old;
    }

    public static GLFWScrollCallbackI glfwSetScrollCallback(long window, GLFWScrollCallbackI cbfun) {
        GLFWScrollCallbackI old = scrollCb;
        scrollCb = cbfun;
        return old;
    }

    public static GLFWWindowFocusCallbackI glfwSetWindowFocusCallback(long window, GLFWWindowFocusCallbackI cbfun) {
        GLFWWindowFocusCallbackI old = focusCb;
        focusCb = cbfun;
        return old;
    }

    public static void glfwDestroyWindow(long window) {
        activeWindow = 0L;
    }

    public static void glfwTerminate() {
        activeWindow = 0L;
        initialized = false;
    }
}
