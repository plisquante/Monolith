package org.lwjgl.openal;

import mono.rhi.NativeRhi;
import org.lwjgl.system.MemoryUtil;

import java.nio.IntBuffer;

public class ALC10 {
    public static final int ALC_TRUE = 1;
    public static final int ALC_FALSE = 0;

    protected ALC10() {}

    public static long alcOpenDevice(CharSequence deviceSpecifier) {
        return NativeRhi.alcOpenDevice(deviceSpecifier == null ? null : deviceSpecifier.toString());
    }
    public static boolean alcCloseDevice(long device) { return NativeRhi.alcCloseDevice(device); }
    public static long alcCreateContext(long device, IntBuffer attrList) {
        return NativeRhi.alcCreateContext(device, MemoryUtil.memAddressSafe(attrList));
    }
    public static boolean alcMakeContextCurrent(long context) { return NativeRhi.alcMakeContextCurrent(context); }
    public static void alcDestroyContext(long context) { NativeRhi.alcDestroyContext(context); }
    public static int alcGetError(long device) { return 0; }
    public static int alcGetInteger(long device, int param) {
        if (param == 0x1002) return 7; // ALC_ATTRIBUTES_SIZE: 3 key/value pairs of ALC_ALL_ATTRIBUTES + terminator
        return NativeRhi.alcGetInteger(device, param);
    }

    /**
     * ALC_ALL_ATTRIBUTES fills the whole buffer with key/value pairs ending in 0 (Minecraft scans it for
     * ALC_MONO_SOURCES); every other query is a single value.
     */
    public static void alcGetIntegerv(long device, int param, IntBuffer dest) {
        if (dest == null || dest.remaining() == 0) return;
        int p = dest.position();
        if (param == 0x1003) { // ALC_ALL_ATTRIBUTES: ALC_FREQUENCY, ALC_MONO_SOURCES, ALC_STEREO_SOURCES, 0
            if (dest.remaining() < 7) {
                // StringBuilder, not "+": a concatenation adds a synthetic class, which renumbers classes globally
                throw new IllegalArgumentException(new StringBuilder("ALC_ALL_ATTRIBUTES needs 7 ints, buffer has ")
                        .append(dest.remaining()).toString());
            }
            int key = 0x1007;
            for (int i = 0; i < 3; i++) {
                dest.put(p + 2 * i, key);
                dest.put(p + 2 * i + 1, NativeRhi.alcGetInteger(device, key));
                key = key == 0x1007 ? 0x1010 : 0x1011;
            }
            dest.put(p + 6, 0);
            return;
        }
        dest.put(p, alcGetInteger(device, param));
    }
    public static String alcGetString(long device, int param) { return NativeRhi.alcGetString(device, param); }
    public static boolean alcIsExtensionPresent(long device, CharSequence extName) { return false; }
}
