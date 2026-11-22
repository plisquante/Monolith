package org.lwjgl.openal;

import j2z.rhi.NativeRhi;
import org.lwjgl.system.MemoryUtil;

import java.nio.ByteBuffer;

/** OpenAL 1.0 dispatcher backed by NativeRhi dynamic libopenal probe + deterministic fallback. */
public class AL10 {
    public static final int AL_NO_ERROR = 0;
    public static final int AL_POSITION = 0x1004;
    public static final int AL_GAIN = 0x100A;

    protected AL10() {}

    public static void alEnable(int capability) {}

    public static int alGenBuffers() { return NativeRhi.alGenBuffer(); }
    public static void alGenBuffers(int[] buffers) {
        if (buffers == null) return;
        for (int i = 0; i < buffers.length; i++) buffers[i] = NativeRhi.alGenBuffer();
    }

    public static void alDeleteBuffers(int buffer) { NativeRhi.alDeleteBuffer(buffer); }
    public static void alDeleteBuffers(int[] buffers) {
        if (buffers == null) return;
        for (int b : buffers) NativeRhi.alDeleteBuffer(b);
    }

    public static void alBufferData(int buffer, int format, ByteBuffer data, int freq) {
        NativeRhi.alBufferData(buffer, format, MemoryUtil.memAddressSafe(data),
                data == null ? 0 : data.remaining(), freq);
    }

    public static int alGenSources() { return NativeRhi.alGenSource(); }
    public static void alGenSources(int[] sources) {
        if (sources == null) return;
        for (int i = 0; i < sources.length; i++) sources[i] = NativeRhi.alGenSource();
    }

    public static void alDeleteSources(int source) { NativeRhi.alDeleteSource(source); }
    public static void alDeleteSources(int[] sources) {
        if (sources == null) return;
        for (int s : sources) NativeRhi.alDeleteSource(s);
    }

    public static void alSourcePlay(int source) { NativeRhi.alSourcePlay(source); }
    public static void alSourcePause(int source) { NativeRhi.alSourcePause(source); }
    public static void alSourceStop(int source) { NativeRhi.alSourceStop(source); }

    public static void alSourceQueueBuffers(int source, int[] buffers) {
        if (buffers == null) return;
        for (int b : buffers) NativeRhi.alSourceQueueBuffer(source, b);
    }

    public static void alSourceUnqueueBuffers(int source, int[] buffers) {
        if (buffers == null) return;
        for (int i = 0; i < buffers.length; i++) buffers[i] = NativeRhi.alSourceUnqueueBuffer(source);
    }

    public static int alGetSourcei(int source, int param) { return NativeRhi.alGetSourcei(source, param); }
    public static void alSourcei(int source, int param, int value) { NativeRhi.alSourcei(source, param, value); }
    public static void alSourcef(int source, int param, float value) { NativeRhi.alSourcef(source, param, value); }
    public static void alSource3f(int source, int param, float x, float y, float z) { NativeRhi.alSource3f(source, param, x, y, z); }
    public static void alSourcefv(int source, int param, float[] values) {
        if (values != null && values.length >= 3) NativeRhi.alSource3f(source, param, values[0], values[1], values[2]);
    }
    public static void alListener3f(int param, float x, float y, float z) { NativeRhi.alListener3f(param, x, y, z); }
    public static void alListenerfv(int param, float[] values) {
        if (values != null && values.length >= 3) NativeRhi.alListener3f(param, values[0], values[1], values[2]);
    }
    public static int alGetError() { return NativeRhi.alGetError(); }
}
