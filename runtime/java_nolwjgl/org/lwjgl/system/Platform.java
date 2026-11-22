package org.lwjgl.system;

import java.util.Locale;

public enum Platform {
    FREEBSD("FreeBSD"),
    LINUX("Linux"),
    MACOSX("macOS"),
    WINDOWS("Windows");

    public enum Architecture {
        X64,
        X86,
        ARM64,
        ARM32,
        PPC64LE,
        RISCV64
    }

    private final String name;

    Platform(String name) {
        this.name = name;
    }

    public String getName() {
        return name;
    }

    @Override
    public String toString() {
        return name;
    }

    public static Platform get() {
        String os = System.getProperty("os.name", "Linux").toLowerCase(Locale.ROOT);
        if (os.contains("win")) return WINDOWS;
        if (os.contains("mac") || os.contains("darwin")) return MACOSX;
        if (os.contains("freebsd")) return FREEBSD;
        return LINUX;
    }

    public static Architecture getArchitecture() {
        String arch = System.getProperty("os.arch", "aarch64").toLowerCase(Locale.ROOT);
        if (arch.contains("aarch64") || arch.contains("arm64")) return Architecture.ARM64;
        if (arch.contains("arm")) return Architecture.ARM32;
        if (arch.contains("64")) return Architecture.X64;
        return Architecture.X86;
    }
}
