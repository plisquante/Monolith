package org.lwjgl.opengl;

import j2z.rhi.NativeRhi;
import org.lwjgl.system.MemoryUtil;

import java.nio.FloatBuffer;

/** Zero-LWJGL OpenGL 2.0 Shader/Program/VertexAttrib shim -> NativeRhi / Android GLESv3. */
public class GL20C extends GL15C {
    public static final int GL_FRAGMENT_SHADER = 0x8B30;
    public static final int GL_VERTEX_SHADER = 0x8B31;
    public static final int GL_COMPILE_STATUS = 0x8B81;
    public static final int GL_LINK_STATUS = 0x8B82;
    public static final int GL_TEXTURE0 = 0x84C0;

    protected GL20C() {}

    public static void glActiveTexture(int texture) { NativeRhi.glActiveTexture(texture); }
    public static int glCreateShader(int type) { return NativeRhi.glCreateShader(type); }
    public static void glDeleteShader(int shader) { NativeRhi.glDeleteShader(shader); }
    public static void glShaderSource(int shader, CharSequence string) {
        NativeRhi.glShaderSource(shader, string == null ? "" : string.toString());
    }
    public static void glCompileShader(int shader) { NativeRhi.glCompileShader(shader); }
    public static int glGetShaderi(int shader, int pname) { return NativeRhi.glGetShaderi(shader, pname); }

    public static int glCreateProgram() { return NativeRhi.glCreateProgram(); }
    public static void glDeleteProgram(int program) { NativeRhi.glDeleteProgram(program); }
    public static void glAttachShader(int program, int shader) { NativeRhi.glAttachShader(program, shader); }
    public static void glLinkProgram(int program) { NativeRhi.glLinkProgram(program); }
    public static void glUseProgram(int program) { NativeRhi.glUseProgram(program); }
    public static int glGetProgrami(int program, int pname) { return NativeRhi.glGetProgrami(program, pname); }
    public static int glGetUniformLocation(int program, CharSequence name) {
        return NativeRhi.glGetUniformLocation(program, name == null ? "" : name.toString());
    }

    public static void glUniform1i(int location, int v0) { NativeRhi.glUniform1i(location, v0); }
    public static void glUniform1f(int location, float v0) { NativeRhi.glUniform1f(location, v0); }
    public static void glUniform2f(int location, float v0, float v1) { NativeRhi.glUniform2f(location, v0, v1); }
    public static void glUniform3f(int location, float v0, float v1, float v2) { NativeRhi.glUniform3f(location, v0, v1, v2); }
    public static void glUniform4f(int location, float v0, float v1, float v2, float v3) { NativeRhi.glUniform4f(location, v0, v1, v2, v3); }
    public static void glUniformMatrix4fv(int location, boolean transpose, FloatBuffer value) {
        NativeRhi.glUniformMatrix4fv(location, value == null ? 0 : (value.remaining() / 16), transpose, MemoryUtil.memAddressSafe(value));
    }

    public static void glEnableVertexAttribArray(int index) { NativeRhi.glEnableVertexAttribArray(index); }
    public static void glDisableVertexAttribArray(int index) { NativeRhi.glDisableVertexAttribArray(index); }
    public static void glVertexAttribPointer(int index, int size, int type, boolean normalized, int stride, long pointer) {
        NativeRhi.glVertexAttribPointer(index, size, type, normalized, stride, pointer);
    }
    public static void glBindAttribLocation(int program, int index, CharSequence name) {
        NativeRhi.glBindAttribLocation(program, index, name == null ? "" : name.toString());
    }
    public static String glGetShaderInfoLog(int shader, int maxLength) {
        return NativeRhi.glGetShaderInfoLog(shader);
    }
    public static String glGetProgramInfoLog(int program, int maxLength) {
        return NativeRhi.glGetProgramInfoLog(program);
    }
    public static void glBlendEquationSeparate(int modeRGB, int modeAlpha) {
        NativeRhi.glBlendEquationSeparate(modeRGB, modeAlpha);
    }
    public static void glBlendFuncSeparate(int sfactorRGB, int dfactorRGB, int sfactorAlpha, int dfactorAlpha) {
        NativeRhi.glBlendFuncSeparate(sfactorRGB, dfactorRGB, sfactorAlpha, dfactorAlpha);
    }
    public static void glDrawBuffers(int[] bufs) {
        NativeRhi.glDrawBuffers(bufs);
    }
    public static void nglShaderSource(int shader, int count, long strings, long length) {
        if (count <= 0 || strings == 0L) return;
        StringBuilder sb = new StringBuilder();
        for (int i = 0; i < count; i++) {
            long strPtr = MemoryUtil.memGetAddress(strings + (long) i * 8L);
            if (strPtr == 0L) continue;
            int len = length == 0L ? -1 : MemoryUtil.memGetInt(length + (long) i * 4L);
            sb.append(len < 0 ? MemoryUtil.memUTF8(strPtr) : MemoryUtil.memASCII(strPtr, len));
        }
        NativeRhi.glShaderSource(shader, sb.toString());
    }
}
