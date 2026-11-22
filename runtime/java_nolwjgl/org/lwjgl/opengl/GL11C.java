package org.lwjgl.opengl;

import j2z.rhi.NativeRhi;
import org.lwjgl.system.MemoryUtil;

import java.nio.ByteBuffer;

/** Zero-LWJGL OpenGL 3.3 Core -> NativeRhi / Android GLESv3 direct dispatch shim. */
public class GL11C {
    public static final int GL_COLOR_BUFFER_BIT = 0x00004000;
    public static final int GL_DEPTH_BUFFER_BIT = 0x00000100;
    public static final int GL_STENCIL_BUFFER_BIT = 0x00000400;
    public static final int GL_TRIANGLES = 0x0004;
    public static final int GL_TRIANGLE_STRIP = 0x0005;
    public static final int GL_LINES = 0x0001;
    public static final int GL_BYTE = 0x1400;
    public static final int GL_UNSIGNED_BYTE = 0x1401;
    public static final int GL_SHORT = 0x1402;
    public static final int GL_UNSIGNED_SHORT = 0x1403;
    public static final int GL_INT = 0x1404;
    public static final int GL_UNSIGNED_INT = 0x1405;
    public static final int GL_FLOAT = 0x1406;
    public static final int GL_DEPTH_TEST = 0x0B71;
    public static final int GL_BLEND = 0x0BE2;
    public static final int GL_CULL_FACE = 0x0B44;
    public static final int GL_SCISSOR_TEST = 0x0C11;
    public static final int GL_TEXTURE_2D = 0x0DE1;
    public static final int GL_TEXTURE_MIN_FILTER = 0x2801;
    public static final int GL_TEXTURE_MAG_FILTER = 0x2800;
    public static final int GL_TEXTURE_WRAP_S = 0x2802;
    public static final int GL_TEXTURE_WRAP_T = 0x2803;
    public static final int GL_NEAREST = 0x2600;
    public static final int GL_LINEAR = 0x2601;
    public static final int GL_RGBA = 0x1908;
    public static final int GL_RGBA8 = 0x8058;
    public static final int GL_RGB = 0x1907;
    public static final int GL_BACK = 0x0405;
    public static final int GL_FRONT = 0x0404;
    public static final int GL_CCW = 0x0901;
    public static final int GL_CW = 0x0900;
    public static final int GL_LEQUAL = 0x0203;
    public static final int GL_LESS = 0x0201;
    public static final int GL_SRC_ALPHA = 0x0302;
    public static final int GL_ONE_MINUS_SRC_ALPHA = 0x0303;
    public static final int GL_VENDOR = 0x1F00;
    public static final int GL_RENDERER = 0x1F01;
    public static final int GL_VERSION = 0x1F02;

    protected GL11C() {}

    public static void glViewport(int x, int y, int width, int height) { NativeRhi.glViewport(x, y, width, height); }
    public static void glClearColor(float red, float green, float blue, float alpha) { NativeRhi.glClearColor(red, green, blue, alpha); }
    public static void glClear(int mask) { NativeRhi.glClear(mask); }
    public static void glClearDepth(double depth) { NativeRhi.glClearDepth(depth); }
    public static void glDepthRange(double near, double far) { NativeRhi.glDepthRange(near, far); }
    public static void glEnable(int cap) { NativeRhi.glEnable(cap); }
    public static void glDisable(int cap) { NativeRhi.glDisable(cap); }
    public static void glBlendFunc(int sfactor, int dfactor) { NativeRhi.glBlendFunc(sfactor, dfactor); }
    public static void glDepthFunc(int func) { NativeRhi.glDepthFunc(func); }
    public static void glDepthMask(boolean flag) { NativeRhi.glDepthMask(flag); }
    public static void glColorMask(boolean r, boolean g, boolean b, boolean a) { NativeRhi.glColorMask(r, g, b, a); }
    public static void glCullFace(int mode) { NativeRhi.glCullFace(mode); }
    public static void glFrontFace(int mode) { NativeRhi.glFrontFace(mode); }
    public static void glScissor(int x, int y, int width, int height) { NativeRhi.glScissor(x, y, width, height); }
    public static void glPolygonMode(int face, int mode) { NativeRhi.glPolygonMode(face, mode); }

    public static int glGenTextures() { return NativeRhi.glGenTextures(); }
    public static void glDeleteTextures(int texture) { NativeRhi.glDeleteTextures(texture); }
    public static void glBindTexture(int target, int texture) { NativeRhi.glBindTexture(target, texture); }
    public static void glTexParameteri(int target, int pname, int param) { NativeRhi.glTexParameteri(target, pname, param); }

    public static void glTexImage2D(int target, int level, int internalformat, int width, int height,
                                    int border, int format, int type, ByteBuffer pixels) {
        NativeRhi.glTexImage2D(target, level, internalformat, width, height, border, format, type,
                MemoryUtil.memAddressSafe(pixels));
    }

    public static void glTexImage2D(int target, int level, int internalformat, int width, int height,
                                    int border, int format, int type, long pixels) {
        NativeRhi.glTexImage2D(target, level, internalformat, width, height, border, format, type, pixels);
    }

    public static void glTexSubImage2D(int target, int level, int xoffset, int yoffset, int width, int height,
                                       int format, int type, ByteBuffer pixels) {
        NativeRhi.glTexSubImage2D(target, level, xoffset, yoffset, width, height, format, type,
                MemoryUtil.memAddressSafe(pixels));
    }

    public static void glTexSubImage2D(int target, int level, int xoffset, int yoffset, int width, int height,
                                       int format, int type, long pixels) {
        NativeRhi.glTexSubImage2D(target, level, xoffset, yoffset, width, height, format, type, pixels);
    }

    public static void glLogicOp(int opcode) {}
    public static void glPixelStorei(int pname, int param) { NativeRhi.glPixelStorei(pname, param); }
    public static void glPolygonOffset(float factor, float units) { NativeRhi.glPolygonOffset(factor, units); }
    public static void glDrawBuffer(int buf) { NativeRhi.glDrawBuffers(new int[]{ buf }); }
    public static void glReadBuffer(int src) { NativeRhi.glReadBuffer(src); }
    public static void glReadPixels(int x, int y, int width, int height, int format, int type, long pixels) {
        NativeRhi.glReadPixels(x, y, width, height, format, type, pixels);
    }
    public static void glFinish() { NativeRhi.glFinish(); }
    public static float glGetFloat(int pname) { return NativeRhi.glGetFloat(pname); }
    public static int glGetTexLevelParameteri(int target, int level, int pname) {
        return NativeRhi.glGetTexLevelParameteri(target, level, pname);
    }

    public static void glDrawArrays(int mode, int first, int count) { NativeRhi.glDrawArrays(mode, first, count); }
    public static void glDrawElements(int mode, int count, int type, long indices) { NativeRhi.glDrawElements(mode, count, type, indices); }
    public static int glGetInteger(int pname) { return NativeRhi.glGetInteger(pname); }
    public static int glGetError() { return NativeRhi.glGetError(); }

    public static String glGetString(int name) {
        String drv = NativeRhi.glGetString(name);
        if (drv != null && !drv.isEmpty()) return drv;
        return switch (name) {
            case GL_VENDOR -> "J2ZIG";
            case GL_RENDERER -> "J2ZIG RHI (Vulkan 1.1+ / GLES 3.0+)";
            case GL_VERSION -> "3.3.0 J2ZIG RHI";
            default -> "";
        };
    }
}
