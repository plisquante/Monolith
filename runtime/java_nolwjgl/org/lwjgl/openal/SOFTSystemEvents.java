package org.lwjgl.openal;

public final class SOFTSystemEvents {
    private SOFTSystemEvents() {}

    public static int alcEventIsSupportedSOFT(int eventType, int deviceType) { return 0; }
    public static boolean alcEventControlSOFT(int[] events, boolean enable) { return false; }
    public static void alcEventCallbackSOFT(SOFTSystemEventProcI callback, long userParam) {}
}
