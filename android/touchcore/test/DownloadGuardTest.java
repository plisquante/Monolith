package com.monolith.touchcore;

import java.io.File;
import java.io.IOException;
import java.nio.file.Files;

/**
 * Tests of DownloadGuard on a desktop JDK:
 *   javac -d /tmp/dg android/touchcore/src/com/monolith/touchcore/DownloadGuard.java android/touchcore/test/DownloadGuardTest.java
 *   java -cp /tmp/dg com.monolith.touchcore.DownloadGuardTest
 */
public final class DownloadGuardTest {
    static int fails = 0;

    static void check(String name, boolean ok) {
        System.out.println((ok ? "PASS " : "FAIL ") + name);
        if (!ok) fails++;
    }

    public static void main(String[] args) throws Exception {
        for (String ok : new String[] {"26.3", "1.21-pre1", "25w10a", "1.20.5-rc1", "b1.7.3", "3D Shareware v1.34", "1.RV-Pre1"}) {
            check("version id accepted: " + ok, DownloadGuard.isSafeVersionId(ok));
        }
        for (String bad : new String[] {null, "", ".", "..", "../x", "a/b", "a\\b", "x..y", "/abs", "a\u0000b", "é", "x".repeat(65)}) {
            check("version id refused: " + (bad == null ? "null" : bad.replace("\u0000", "\\0")), !DownloadGuard.isSafeVersionId(bad));
        }

        check("sha1 accepted", DownloadGuard.isSha1("e877b6a07acd633fb3bb475002175cec036e7b87"));
        check("sha1 uppercase accepted", DownloadGuard.isSha1("E877B6A07ACD633FB3BB475002175CEC036E7B87"));
        check("sha1 too short", !DownloadGuard.isSha1("abc"));
        check("sha1 with a path in it", !DownloadGuard.isSha1("../../etc/passwd" + "0".repeat(24)));
        check("sha1 null", !DownloadGuard.isSha1(null));

        for (String ok : new String[] {
                "https://piston-meta.mojang.com/mc/game/version_manifest_v2.json",
                "https://piston-data.mojang.com/v1/objects/e877/client.jar",
                "https://libraries.minecraft.net/org/lwjgl/lwjgl/3.3.3/lwjgl-3.3.3.jar",
                "https://resources.download.minecraft.net/ab/abcdef",
                "HTTPS://launchermeta.mojang.com/x"}) {
            check("url trusted: " + ok, DownloadGuard.isTrustedUrl(ok));
        }
        for (String bad : new String[] {
                null, "http://piston-meta.mojang.com/x", "file:///data/data/x", "https://evil.example/x",
                "https://mojang.com.evil.example/x", "https://evilmojang.com/x", "https://user@piston-meta.mojang.com/x",
                "https://piston-meta.mojang.com:8443/x", "ftp://piston-meta.mojang.com/x", "not a url", "https:///x"}) {
            check("url refused: " + bad, !DownloadGuard.isTrustedUrl(bad));
        }

        File root = Files.createTempDirectory("dg").toFile();
        File inside = DownloadGuard.under(root, "org/lwjgl/lwjgl.jar");
        check("path inside the root", inside.getPath().startsWith(root.getCanonicalPath() + File.separator));
        // java.io.File(parent, child) treats an absolute child as relative to the parent: it cannot leave the root
        check("an absolute path stays under the root",
                DownloadGuard.under(root, "/etc/passwd").getPath().startsWith(root.getCanonicalPath() + File.separator));
        for (String bad : new String[] {"../escape", "a/../../escape", "", null}) {
            boolean refused;
            try {
                DownloadGuard.under(root, bad);
                refused = false;
            } catch (IOException e) {
                refused = true;
            }
            check("path refused: " + bad, refused);
        }
        check("path with .. that stays inside is fine", DownloadGuard.under(root, "a/../b.jar").getName().equals("b.jar"));
        System.out.println(fails == 0 ? "ALL PASSED" : fails + " FAILED");
        System.exit(fails == 0 ? 0 : 1);
    }
}
