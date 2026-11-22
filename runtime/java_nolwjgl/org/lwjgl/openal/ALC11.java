package org.lwjgl.openal;

public class ALC11 extends ALC10 {
    protected ALC11() {}

    public static int alcGetInteger(long device, int param) {
        return ALC10.alcGetInteger(device, param);
    }
}
