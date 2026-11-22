package com.monolith.touchcore;

import android.content.Context;
import android.content.pm.PackageInfo;
import android.system.Os;

import org.json.JSONObject;

import java.io.File;
import java.io.FileInputStream;
import java.io.FileOutputStream;
import java.io.IOException;
import java.io.InputStream;
import java.io.OutputStream;
import java.nio.charset.StandardCharsets;
import java.util.Enumeration;
import java.util.zip.ZipEntry;
import java.util.zip.ZipFile;
import java.util.zip.ZipInputStream;

/**
 * Prepares everything the native engine reads at startup besides the .so itself.
 *
 * The engine serves Class.getResource* from the directory MONO_RESOURCES: the non-class entries of the
 * Minecraft client jar (assets/, data/, version.json, pack.png, ...) plus the resource files of the Java
 * libraries it was built from (joptsimple message bundles, log4j configuration, ICU data, ...).
 * Without them Minecraft's Main dies while building its option parser, before anything is drawn.
 *
 * The client jar part is extracted from the jar the launcher downloaded; the library part ships inside this
 * APK as assets/engine_resources.zip (tools/build_touchcore_apk.py --engine-resources).
 */
final class EngineResources {
    static final String BUNDLE_ASSET = "engine_resources.zip";
    private static final String STAMP = ".source";
    private static final String OVERRIDE_PREFIX = "__override__/";
    /** The bundle's JDK files (conf/, lib/tzdb.dat) land in <resources>/__javahome__, the engine's java.home. */
    static final String JAVA_HOME_DIR = "__javahome__";

    interface Progress {
        void status(String message);
    }

    private EngineResources() {}

    /** Extracts the resource tree when it is missing or stale. Throws with a user-readable message when it cannot. */
    static void prepare(Context ctx, LauncherConfig cfg, Progress progress) throws IOException {
        File jar = new File(cfg.clientJarPath == null ? "" : cfg.clientJarPath);
        if (!jar.isFile()) {
            throw new IOException("client.jar bulunamadi (" + cfg.clientJarPath + ").\n"
                    + "Ayarlar sekmesinden Mojang surumunu indirin; motor, kaynaklarini bu jar'dan okur.");
        }
        File dir = new File(cfg.resourcesDir);
        String ident = jar.length() + ":" + jar.lastModified() + ":" + bundleIdent(ctx);
        File stamp = new File(dir, STAMP);
        if (stamp.isFile() && ident.equals(readText(stamp)) && new File(dir, "assets").isDirectory()) {
            return;
        }

        if (progress != null) progress.status("Eski kaynaklar temizleniyor...");
        deleteRecursively(dir);
        if (!dir.mkdirs() && !dir.isDirectory()) {
            throw new IOException("Kaynak dizini olusturulamadi: " + dir);
        }

        if (progress != null) progress.status("client.jar kaynaklari aciliyor (assets, data)...");
        int fromJar = extractJar(jar, dir, progress);
        if (progress != null) progress.status("Kutuphane kaynaklari aciliyor...");
        int fromLibs = extractBundle(ctx, dir);
        if (fromJar == 0) {
            throw new IOException("client.jar icinde kaynak dosyasi yok: " + jar);
        }
        if (fromLibs == 0) {
            throw new IOException("APK icinde " + BUNDLE_ASSET + " yok veya bos. Bu APK motor kaynaklari olmadan derlenmis;\n"
                    + "tools/build_touchcore_apk.py --engine-resources ile yeniden derleyin.");
        }
        if (!new File(dir, JAVA_HOME_DIR + "/conf/security/java.security").isFile()) {
            throw new IOException("APK icindeki " + BUNDLE_ASSET + " JDK dosyalarini (" + JAVA_HOME_DIR + "/conf) icermiyor;\n"
                    + "tools/build_touchcore_apk.py ile yeniden derleyin.");
        }
        writeText(stamp, ident);
    }

    /**
     * Process environment the engine reads: HOME and TMPDIR (an Android process has neither a writable home nor /tmp),
     * the game and asset directories, MC_VERSION and MC_ASSET_INDEX for the engine's argv.
     */
    /** Memory kept outside the Java heap budget: GL driver, textures, audio, engine code. */
    static final long NATIVE_RESERVE_MB = 1024;

    static void exportEnv(Context ctx, LauncherConfig cfg) throws IOException {
        File home = new File(ctx.getFilesDir(), "monolith/home");
        File tmp = new File(ctx.getCacheDir(), "mono-tmp");
        for (File d : new File[] {home, tmp, new File(cfg.gameDir), new File(cfg.assetsDir)}) {
            if (!d.isDirectory() && !d.mkdirs() && !d.isDirectory()) {
                throw new IOException("Dizin olusturulamadi: " + d);
            }
        }
        try {
            // the engine finds libmonoshader.so (shaderc, SPIRV-Cross) and other JNI libraries here
            Os.setenv("MONO_NATIVES", ctx.getApplicationInfo().nativeLibraryDir, true);
            // java.home of the engine (conf/security/java.security, lib/tzdb.dat): shipped in the bundle, see prepare()
            Os.setenv("MONO_JAVA_HOME", new File(cfg.resourcesDir, JAVA_HOME_DIR).getAbsolutePath(), true);
            Os.setenv("HOME", home.getAbsolutePath(), true);
            // debug setting: the native host names the player at the top of engine_log.txt
            if (cfg.debugMode && cfg.username != null) Os.setenv("MONO_DEBUG_USER", cfg.username, true);
            else Os.unsetenv("MONO_DEBUG_USER");
            // debug setting: the engine prints the top allocation sites with every collection (engine_log.txt)
            if (cfg.debugMode) Os.setenv("MONO_ALLOC_PROFILE", "1", true);
            else Os.unsetenv("MONO_ALLOC_PROFILE");
            // debug setting: live bytes by class every 3rd collection (what the 1 GB of live data is made of)
            if (cfg.debugMode) Os.setenv("MONO_HEAP_CENSUS", "3", true);
            else Os.unsetenv("MONO_HEAP_CENSUS");
            // Java heap ceiling of the engine's collector: the launcher's memory limit minus room for the
            // native side (GL driver, textures, audio, code). Past it the game gets OutOfMemoryError and writes
            // its own crash report instead of driving Android into killing other apps.
            android.app.ActivityManager am = (android.app.ActivityManager) ctx.getSystemService(Context.ACTIVITY_SERVICE);
            android.app.ActivityManager.MemoryInfo mem = new android.app.ActivityManager.MemoryInfo();
            am.getMemoryInfo(mem);
            long limitMb = (mem.totalMem >> 20) * Math.max(30, Math.min(80, cfg.memLimitPct)) / 100;
            long heapMb = Math.max(512, limitMb - NATIVE_RESERVE_MB);
            Os.setenv("MONO_HEAP_MAX_MB", Long.toString(heapMb), true);
            // Collect after max(256 MiB, live data) of allocation. A quarter of the live data (25) kept RAM low but the
            // game allocates that much in about half a second: in a world the collector ran twice a second and the
            // game spent 80% of its time stopped (FPS 0-6). Until the collector stops pausing the game, fewer
            // collections win over the RAM a smaller heap saves.
            Os.setenv("MONO_GC_MIN_MB", "256", true);
            // Let the garbage grow to three times the live data before a collection (each one stops the game ~0.7 s);
            // the runtime caps the allowance at 7/8 of the room below MONO_HEAP_MAX_MB.
            Os.setenv("MONO_GC_FACTOR", "300", true);
            // Whatever the live size, never let more than ~14% of the phone's RAM (768 MiB to 3 GiB) pile up as garbage
            // between collections: a heap that swells to the ceiling gets the whole app killed by Android's low-memory
            // killer (a 7 GB phone died at 4.3 GB PSS with no crash report).
            long garbageMb = Math.max(768, Math.min(3072, (mem.totalMem >> 20) * 14 / 100));
            Os.setenv("MONO_GC_MAX_GARBAGE_MB", Long.toString(garbageMb), true);
            // one line per collection in engine_log.txt (live heap, pause parts)
            Os.setenv("MONO_GC_VERBOSE", "1", true);
            Os.setenv("TMPDIR", tmp.getAbsolutePath(), true);
            File jar = new File(cfg.clientJarPath == null ? "" : cfg.clientJarPath);
            File verDir = jar.getParentFile();
            if (verDir == null || !jar.isFile()) return;
            String id = verDir.getName();
            Os.setenv("MC_VERSION", id, true);
            File meta = new File(verDir, id + ".json");
            if (meta.isFile()) {
                JSONObject idx = new JSONObject(readText(meta)).optJSONObject("assetIndex");
                if (idx != null && !idx.optString("id", "").isEmpty()) {
                    Os.setenv("MC_ASSET_INDEX", idx.getString("id"), true);
                }
            }
        } catch (android.system.ErrnoException | org.json.JSONException e) {
            throw new IOException("Motor ortami hazirlanamadi: " + e.getMessage());
        }
    }

    private static String bundleIdent(Context ctx) {
        try {
            PackageInfo pi = ctx.getPackageManager().getPackageInfo(ctx.getPackageName(), 0);
            return pi.lastUpdateTime + "/" + pi.versionCode;
        } catch (Throwable t) {
            return "?";
        }
    }

    private static boolean skipJarEntry(String n) {
        if (n.endsWith(".class") || n.startsWith("META-INF/versions/")) return true;
        String up = n.toUpperCase(java.util.Locale.ROOT);
        return up.startsWith("META-INF/") && (up.endsWith(".SF") || up.endsWith(".RSA") || up.endsWith(".DSA") || up.endsWith(".MF"));
    }

    private static int extractJar(File jar, File dest, Progress progress) throws IOException {
        int n = 0;
        try (ZipFile z = new ZipFile(jar)) {
            Enumeration<? extends ZipEntry> en = z.entries();
            while (en.hasMoreElements()) {
                ZipEntry e = en.nextElement();
                if (e.isDirectory() || skipJarEntry(e.getName())) continue;
                try (InputStream in = z.getInputStream(e)) {
                    writeEntry(dest, e.getName(), in, true);
                }
                n++;
                if (progress != null && n % 2000 == 0) progress.status("client.jar: " + n + " dosya acildi...");
            }
        }
        return n;
    }

    /** The client jar wins on name clashes, so library files never overwrite an existing file; only the explicit
     * branding overrides (entries under __override__/) replace the jar's own file. */
    private static int extractBundle(Context ctx, File dest) throws IOException {
        int n = 0;
        try (InputStream raw = ctx.getAssets().open(BUNDLE_ASSET); ZipInputStream zin = new ZipInputStream(raw)) {
            ZipEntry e;
            while ((e = zin.getNextEntry()) != null) {
                if (e.isDirectory()) continue;
                String name = e.getName();
                if (name.startsWith(OVERRIDE_PREFIX)) {
                    writeEntry(dest, name.substring(OVERRIDE_PREFIX.length()), zin, true); // branding replaces the jar's file
                } else {
                    writeEntry(dest, name, zin, false);
                }
                n++;
            }
        } catch (java.io.FileNotFoundException missing) {
            return 0;
        }
        return n;
    }

    private static void writeEntry(File root, String name, InputStream in, boolean overwrite) throws IOException {
        File out = new File(root, name);
        String rootPath = root.getCanonicalPath() + File.separator;
        if (!out.getCanonicalPath().startsWith(rootPath)) {
            throw new IOException("Gecersiz zip girdisi: " + name);
        }
        if (!overwrite && out.exists()) return;
        File parent = out.getParentFile();
        if (parent != null && !parent.isDirectory() && !parent.mkdirs() && !parent.isDirectory()) {
            throw new IOException("Dizin olusturulamadi: " + parent);
        }
        byte[] buf = new byte[64 * 1024];
        try (OutputStream os = new FileOutputStream(out)) {
            int r;
            while ((r = in.read(buf)) > 0) os.write(buf, 0, r);
        }
    }

    private static String readText(File f) throws IOException {
        try (InputStream in = new FileInputStream(f)) {
            java.io.ByteArrayOutputStream bos = new java.io.ByteArrayOutputStream();
            byte[] buf = new byte[8192];
            int r;
            while ((r = in.read(buf)) > 0) bos.write(buf, 0, r);
            return new String(bos.toByteArray(), StandardCharsets.UTF_8);
        }
    }

    private static void writeText(File f, String s) throws IOException {
        try (OutputStream os = new FileOutputStream(f)) {
            os.write(s.getBytes(StandardCharsets.UTF_8));
        }
    }

    private static void deleteRecursively(File f) {
        if (!f.exists()) return;
        File[] kids = f.listFiles();
        if (kids != null) {
            for (File k : kids) deleteRecursively(k);
        }
        // `delete` on a symlink removes the link, never its target.
        //noinspection ResultOfMethodCallIgnored
        f.delete();
    }
}
