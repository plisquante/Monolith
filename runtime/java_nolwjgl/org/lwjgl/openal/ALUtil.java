package org.lwjgl.openal;

import java.util.Collections;
import java.util.List;

public final class ALUtil {
    private ALUtil() {}

    public static List<String> getStringList(long deviceHandle, int token) {
        return Collections.singletonList("MONOLITH Audio Output");
    }
}
