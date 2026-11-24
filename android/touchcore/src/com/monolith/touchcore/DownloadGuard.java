package com.monolith.touchcore;

import java.io.File;
import java.io.IOException;
import java.net.URI;
import java.util.Locale;

/**
 * Checks for everything that the downloader takes from the version manifest, the version file and the asset index before it
 * turns it into a URL or a path: version ids, SHA-1 values, library paths and download hosts. The values arrive over HTTPS,
 * but a malformed or tampered one must not be able to write outside the game directories or fetch from another host.
 * Plain Java, no Android classes, so it can be tested on a desktop JDK.
 */
final class DownloadGuard {
    private DownloadGuard() {}

    /** A version id such as {@code 26.3}, {@code 1.21-pre1}, {@code 25w10a}; no path separators and no {@code ..}. */
    static boolean isSafeVersionId(String id) {
        if (id == null || id.isEmpty() || id.length() > 64) return false;
        if (id.equals(".") || id.contains("..")) return false;
        for (int i = 0; i < id.length(); i++) {
            char c = id.charAt(i);
            boolean ok = (c >= 'a' && c <= 'z') || (c >= 'A' && c <= 'Z') || (c >= '0' && c <= '9')
                    || c == '.' || c == '_' || c == '-' || c == '+' || c == ' ';
            if (!ok) return false;
        }
        return true;
    }

    /** 40 hexadecimal digits. */
    static boolean isSha1(String h) {
        if (h == null || h.length() != 40) return false;
        for (int i = 0; i < 40; i++) {
            char c = h.charAt(i);
            boolean hex = (c >= '0' && c <= '9') || (c >= 'a' && c <= 'f') || (c >= 'A' && c <= 'F');
            if (!hex) return false;
        }
        return true;
    }

    /** An https URL on a Mojang or Minecraft host: the only places the official manifest sends files from. */
    static boolean isTrustedUrl(String url) {
        if (url == null) return false;
        try {
            URI u = new URI(url);
            if (!"https".equalsIgnoreCase(u.getScheme()) || u.getUserInfo() != null || u.getHost() == null) return false;
            if (u.getPort() != -1 && u.getPort() != 443) return false;
            String h = u.getHost().toLowerCase(Locale.ROOT);
            return h.equals("mojang.com") || h.endsWith(".mojang.com") || h.equals("minecraft.net") || h.endsWith(".minecraft.net");
        } catch (Exception e) {
            return false;
        }
    }

    /** {@code root/relative}, refused when the resolved file is not inside {@code root} ({@code ..}, absolute paths, links). */
    static File under(File root, String relative) throws IOException {
        if (relative == null || relative.isEmpty()) throw new IOException("empty path");
        File canonRoot = root.getCanonicalFile();
        File f = new File(canonRoot, relative).getCanonicalFile();
        String prefix = canonRoot.getPath().endsWith(File.separator) ? canonRoot.getPath() : canonRoot.getPath() + File.separator;
        if (!f.getPath().startsWith(prefix)) throw new IOException("path leaves its directory: " + relative);
        return f;
    }
}
