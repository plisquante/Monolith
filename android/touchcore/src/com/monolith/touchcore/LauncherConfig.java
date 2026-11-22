package com.monolith.touchcore;

import android.content.Context;
import android.content.SharedPreferences;

import org.json.JSONArray;
import org.json.JSONObject;

import java.io.ByteArrayOutputStream;
import java.io.File;
import java.io.FileInputStream;
import java.io.FileOutputStream;
import java.io.PrintWriter;
import java.io.StringWriter;
import java.nio.charset.StandardCharsets;
import java.util.ArrayList;
import java.util.Collections;
import java.util.LinkedHashSet;
import java.util.List;
import java.util.Set;

public final class LauncherConfig {
    private static final String PREF_NAME = "touchcore_mono_prefs_v2";
    private static final String KEY_USERNAME = "username";
    private static final String KEY_RECENT_USERS = "recent_users";
    private static final String KEY_ACTIVE_PROFILE = "active_profile";
    private static final String KEY_PROFILES_JSON = "profiles_map_json";

    private static final String KEY_VERSION_NAME = "version_name";
    private static final String KEY_CLIENT_JAR = "client_jar_path";
    private static final String KEY_BACKEND_MODE = "backend_mode";
    private static final String KEY_POWER_PROFILE = "power_profile";
    private static final String KEY_RENDER_DIST = "render_dist";
    private static final String KEY_TARGET_FPS = "target_fps";
    private static final String KEY_RES_SCALE = "res_scale";
    private static final String KEY_MEM_LIMIT = "mem_limit_pct";
    private static final String KEY_SHIZUKU_RAM = "shizuku_ram";
    private static final String KEY_DEBUG_MODE = "debug_mode";
    private static final String KEY_FOV_DEG = "fov_deg";
    private static final String KEY_SENSITIVITY = "sensitivity";
    private static final String KEY_INVERT_Y = "invert_y";
    private static final String KEY_HAPTIC = "haptic_level";
    private static final String KEY_AUTO_F3 = "auto_f3";
    private static final String KEY_GAME_DIR = "game_dir";
    private static final String KEY_EXT_LIB = "ext_lib_path";
    private static final String KEY_RES_DIR = "resources_dir";
    private static final String KEY_ASSETS_DIR = "assets_dir";
    private static final String KEY_EXTRA_ENV = "extra_env";
    private static final String KEY_EXTRA_ARGS = "extra_args";

    public static final int BACKEND_AUTO = 0;
    public static final int BACKEND_VULKAN = 1;
    public static final int BACKEND_GLES = 2;

    public static final int POWER_ECO = 0;
    public static final int POWER_BALANCED = 1;
    public static final int POWER_TURBO = 2;

    private final Context appContext;
    private final SharedPreferences prefs;
    private final File defaultRootDir;

    public String username;
    public final List<String> recentUsernames = new ArrayList<String>();
    public String activeProfileName;
    public String selectedVersion;
    public String clientJarPath;

    public int backendMode;
    public int powerProfile;
    public int renderDistance;
    public int targetFps;
    public int resScalePct;
    /** Process memory limit as a percentage of the phone's RAM; the game is stopped when it passes it (see GameActivity). */
    public int memLimitPct;
    /** Run the Shizuku RAM commands (ShizukuRam) before each game start. */
    public boolean shizukuRam;
    /** Debug setting: the Shizuku RAM commands run, and reports and logs name the player (who sent which report). */
    public boolean debugMode;
    public float fovDeg;
    public float sensitivity;
    public boolean invertY;
    public int hapticLevel;
    public boolean autoF3;

    public String gameDir;
    public String extLibPath;
    public String resourcesDir;
    public String assetsDir;
    public String extraEnv;
    public String extraArgs;

    public LauncherConfig(Context ctx) {
        this.appContext = ctx.getApplicationContext();
        this.prefs = appContext.getSharedPreferences(PREF_NAME, Context.MODE_PRIVATE);
        this.defaultRootDir = new File(appContext.getFilesDir(), "monolith");
        ensureDirectories();
        load();
    }

    public void ensureDirectories() {
        new File(defaultRootDir, "game").mkdirs();
        new File(defaultRootDir, "resources").mkdirs();
        new File(defaultRootDir, "assets").mkdirs();
        new File(defaultRootDir, "mods").mkdirs();
        new File(defaultRootDir, "native").mkdirs();
        new File(defaultRootDir, "versions").mkdirs();
    }

    public void load() {
        username = prefs.getString(KEY_USERNAME, "Oyuncu");
        recentUsernames.clear();
        try {
            JSONArray arr = new JSONArray(prefs.getString(KEY_RECENT_USERS, "[]"));
            for (int i = 0; i < arr.length(); i++) {
                String u = arr.optString(i, "").trim();
                if (!u.isEmpty() && !recentUsernames.contains(u)) {
                    recentUsernames.add(u);
                }
            }
        } catch (Exception ignored) {}
        if (recentUsernames.isEmpty()) {
            recentUsernames.add(username);
        }

        selectedVersion = prefs.getString(KEY_VERSION_NAME, "MONOLITH 26.3 / Client Core v2");
        clientJarPath = prefs.getString(KEY_CLIENT_JAR, "");
        activeProfileName = prefs.getString(KEY_ACTIVE_PROFILE, ControlProfile.PRESET_POCKET);
        // one-time switch to the Pocket layout for installs that still use the old built-in default
        if (!prefs.getBoolean("ui_pocket_v1", false)) {
            if (ControlProfile.PRESET_DEFAULT.equals(activeProfileName)) activeProfileName = ControlProfile.PRESET_POCKET;
            prefs.edit().putBoolean("ui_pocket_v1", true).putString(KEY_ACTIVE_PROFILE, activeProfileName).apply();
        }
        backendMode = prefs.getInt(KEY_BACKEND_MODE, BACKEND_AUTO);
        powerProfile = prefs.getInt(KEY_POWER_PROFILE, POWER_BALANCED);
        renderDistance = prefs.getInt(KEY_RENDER_DIST, 10);
        targetFps = prefs.getInt(KEY_TARGET_FPS, 60);
        resScalePct = prefs.getInt(KEY_RES_SCALE, 100);
        memLimitPct = prefs.getInt(KEY_MEM_LIMIT, 60);
        shizukuRam = prefs.getBoolean(KEY_SHIZUKU_RAM, false);
        debugMode = prefs.getBoolean(KEY_DEBUG_MODE, false);
        fovDeg = prefs.getFloat(KEY_FOV_DEG, 75f);
        sensitivity = prefs.getFloat(KEY_SENSITIVITY, 1.0f);
        invertY = prefs.getBoolean(KEY_INVERT_Y, false);
        hapticLevel = prefs.getInt(KEY_HAPTIC, 1);
        autoF3 = prefs.getBoolean(KEY_AUTO_F3, false);

        gameDir = prefs.getString(KEY_GAME_DIR, new File(defaultRootDir, "game").getAbsolutePath());
        extLibPath = prefs.getString(KEY_EXT_LIB, "");
        resourcesDir = prefs.getString(KEY_RES_DIR, new File(defaultRootDir, "resources").getAbsolutePath());
        assetsDir = prefs.getString(KEY_ASSETS_DIR, new File(defaultRootDir, "assets").getAbsolutePath());
        extraEnv = prefs.getString(KEY_EXTRA_ENV, "");
        extraArgs = prefs.getString(KEY_EXTRA_ARGS, "");

        ensureDefaultProfilesSeeded();
    }

    public void save() {
        String cleanUser = sanitizeUsername(username);
        if (!cleanUser.isEmpty()) {
            username = cleanUser;
            recordRecentUsername(cleanUser);
        }

        JSONArray arr = new JSONArray();
        for (String u : recentUsernames) {
            arr.put(u);
        }

        prefs.edit()
                .putString(KEY_USERNAME, username)
                .putString(KEY_RECENT_USERS, arr.toString())
                .putString(KEY_ACTIVE_PROFILE, activeProfileName)
                .putString(KEY_VERSION_NAME, selectedVersion)
                .putString(KEY_CLIENT_JAR, clientJarPath)
                .putInt(KEY_BACKEND_MODE, backendMode)
                .putInt(KEY_POWER_PROFILE, powerProfile)
                .putInt(KEY_RENDER_DIST, renderDistance)
                .putInt(KEY_TARGET_FPS, targetFps)
                .putInt(KEY_RES_SCALE, resScalePct)
                .putInt(KEY_MEM_LIMIT, memLimitPct)
                .putBoolean(KEY_SHIZUKU_RAM, shizukuRam)
                .putBoolean(KEY_DEBUG_MODE, debugMode)
                .putFloat(KEY_FOV_DEG, fovDeg)
                .putFloat(KEY_SENSITIVITY, sensitivity)
                .putBoolean(KEY_INVERT_Y, invertY)
                .putInt(KEY_HAPTIC, hapticLevel)
                .putBoolean(KEY_AUTO_F3, autoF3)
                .putString(KEY_GAME_DIR, gameDir)
                .putString(KEY_EXT_LIB, extLibPath)
                .putString(KEY_RES_DIR, resourcesDir)
                .putString(KEY_ASSETS_DIR, assetsDir)
                .putString(KEY_EXTRA_ENV, extraEnv)
                .putString(KEY_EXTRA_ARGS, extraArgs)
                .apply();
    }

    public static String sanitizeUsername(String raw) {
        if (raw == null) return "";
        StringBuilder sb = new StringBuilder();
        for (int i = 0; i < raw.length(); i++) {
            char c = raw.charAt(i);
            if ((c >= 'a' && c <= 'z') || (c >= 'A' && c <= 'Z')
                    || (c >= '0' && c <= '9') || c == '_') {
                sb.append(c);
                if (sb.length() >= 16) break;
            }
        }
        return sb.toString();
    }

    public static boolean isValidUsername(String raw) {
        if (raw == null) return false;
        int len = raw.length();
        if (len < 3 || len > 16) return false;
        for (int i = 0; i < len; i++) {
            char c = raw.charAt(i);
            boolean ok = (c >= 'a' && c <= 'z') || (c >= 'A' && c <= 'Z')
                    || (c >= '0' && c <= '9') || c == '_';
            if (!ok) return false;
        }
        return true;
    }

    public void recordRecentUsername(String user) {
        if (user == null || user.isEmpty()) return;
        recentUsernames.remove(user);
        recentUsernames.add(0, user);
        while (recentUsernames.size() > 8) {
            recentUsernames.remove(recentUsernames.size() - 1);
        }
    }

    private JSONObject readProfileMap() {
        try {
            return new JSONObject(prefs.getString(KEY_PROFILES_JSON, "{}"));
        } catch (Exception e) {
            return new JSONObject();
        }
    }

    private void writeProfileMap(JSONObject map) {
        prefs.edit().putString(KEY_PROFILES_JSON, map.toString()).apply();
    }

    private void ensureDefaultProfilesSeeded() {
        JSONObject map = readProfileMap();
        boolean changed = false;
        for (String name : ControlProfile.builtInNames()) {
            if (!map.has(name)) {
                try {
                    map.put(name, ControlProfile.createPreset(name).toJson());
                    changed = true;
                } catch (Exception ignored) {}
            }
        }
        if (changed) {
            writeProfileMap(map);
        }
    }

    public List<String> listProfileNames() {
        JSONObject map = readProfileMap();
        Set<String> ordered = new LinkedHashSet<String>();
        for (String b : ControlProfile.builtInNames()) {
            if (map.has(b)) ordered.add(b);
        }
        JSONArray keys = map.names();
        if (keys != null) {
            List<String> custom = new ArrayList<String>();
            for (int i = 0; i < keys.length(); i++) {
                String k = keys.optString(i, "");
                if (!k.isEmpty() && !ordered.contains(k)) {
                    custom.add(k);
                }
            }
            Collections.sort(custom);
            ordered.addAll(custom);
        }
        return new ArrayList<String>(ordered);
    }

    public ControlProfile loadProfile(String profileName) {
        // the Pocket layout is rebuilt from code so layout fixes reach installs that saved an older copy
        if (ControlProfile.PRESET_POCKET.equals(profileName)) {
            return ControlProfile.createPreset(profileName);
        }
        JSONObject map = readProfileMap();
        JSONObject obj = map.optJSONObject(profileName);
        if (obj != null) {
            try {
                return ControlProfile.fromJson(obj);
            } catch (Exception ignored) {}
        }
        ControlProfile fallback = ControlProfile.createPreset(profileName);
        saveProfile(fallback);
        return fallback;
    }

    public ControlProfile loadActiveProfile() {
        return loadProfile(activeProfileName);
    }

    public void saveProfile(ControlProfile profile) {
        if (profile == null || profile.name == null || profile.name.trim().isEmpty()) return;
        JSONObject map = readProfileMap();
        try {
            map.put(profile.name.trim(), profile.toJson());
            writeProfileMap(map);
        } catch (Exception ignored) {}
    }

    public boolean deleteProfile(String profileName) {
        for (String builtin : ControlProfile.builtInNames()) {
            if (builtin.equals(profileName)) {
                return false;
            }
        }
        JSONObject map = readProfileMap();
        map.remove(profileName);
        writeProfileMap(map);
        if (profileName.equals(activeProfileName)) {
            activeProfileName = ControlProfile.PRESET_DEFAULT;
            save();
        }
        return true;
    }

    public ControlProfile resetProfileToDefault(String profileName) {
        ControlProfile fresh = ControlProfile.createPreset(profileName);
        fresh.name = profileName;
        saveProfile(fresh);
        return fresh;
    }

    public File getNativeImportTarget() {
        return new File(new File(defaultRootDir, "native"), "libminecraft_mono.so");
    }

    public File getJarImportTarget(String filename) {
        String safe = (filename == null || filename.trim().isEmpty()) ? "client.jar" : filename.replaceAll("[^A-Za-z0-9._-]", "_");
        return new File(new File(defaultRootDir, "versions"), safe);
    }

    public void resetAll() {
        prefs.edit().clear().apply();
        ensureDirectories();
        load();
    }

    public static void installCrashHandler(final Context ctx) {
        final Thread.UncaughtExceptionHandler prev = Thread.getDefaultUncaughtExceptionHandler();
        Thread.setDefaultUncaughtExceptionHandler(new Thread.UncaughtExceptionHandler() {
            @Override
            public void uncaughtException(Thread t, Throwable e) {
                try {
                    StringWriter sw = new StringWriter();
                    e.printStackTrace(new PrintWriter(sw));
                    File f = new File(ctx.getFilesDir(), "last_crash.txt");
                    FileOutputStream fos = new FileOutputStream(f);
                    fos.write(("Thread: " + t.getName() + "\n" + sw.toString()).getBytes(StandardCharsets.UTF_8));
                    fos.close();
                } catch (Throwable ignored) {}
                if (prev != null) {
                    prev.uncaughtException(t, e);
                }
            }
        });
    }

    /** Newest Minecraft crash report (<game dir>/crash-reports/crash-*.txt), or null. */
    public File newestCrashReport() {
        File[] fs = new File(gameDir == null ? "" : gameDir, "crash-reports").listFiles();
        File best = null;
        if (fs != null) for (File f : fs) {
            if (f.isFile() && f.getName().endsWith(".txt") && (best == null || f.lastModified() > best.lastModified())) best = f;
        }
        return best;
    }

    /** Every report file worth sending: Minecraft crash reports and logs, engine output, native and Java crashes. */
    public java.util.List<File> reportFiles() {
        java.util.List<File> out = new java.util.ArrayList<>();
        File gd = new File(gameDir == null ? "" : gameDir);
        for (String sub : new String[] {"crash-reports", "logs"}) {
            File[] fs = new File(gd, sub).listFiles();
            if (fs != null) for (File f : fs) if (f.isFile() && f.length() < 32L * 1024 * 1024) out.add(f);
        }
        File[] shots = gd.listFiles();
        if (shots != null) for (File f : shots) if (f.isFile() && f.getName().startsWith("frame_") && f.getName().endsWith(".png")) out.add(f);
        for (File f : new File[] {new File(gd, "engine_log.txt"), new File(gd, "native_crash.txt"), new File(gd, "display_info.txt"),
                new File(gd, "memory_limit.txt"),
                new File(gd, "shizuku.txt"),
                new File(gd, "metrics.csv"),
                new File(appContext.getFilesDir(), "exit_info.txt"),
                new File(appContext.getFilesDir(), "last_crash.txt")}) {
            if (f.isFile()) out.add(f);
        }
        return out;
    }

    /** Why Android ended the previous run (low-memory kill, crash, user swipe...): written to files/exit_info.txt. */
    public void recordExitInfo() {
        if (android.os.Build.VERSION.SDK_INT < 30) return;
        try {
            android.app.ActivityManager am = (android.app.ActivityManager) appContext.getSystemService(Context.ACTIVITY_SERVICE);
            java.util.List<android.app.ApplicationExitInfo> list = am.getHistoricalProcessExitReasons(null, 0, 3);
            StringBuilder sb = new StringBuilder();
            for (android.app.ApplicationExitInfo e : list) {
                sb.append(new java.util.Date(e.getTimestamp())).append("  ").append(exitReasonName(e.getReason()))
                  .append("  status=").append(e.getStatus())
                  .append("  pss=").append(e.getPss() / 1024).append("MB  rss=").append(e.getRss() / 1024).append("MB")
                  .append("  importance=").append(e.getImportance());
                if (e.getDescription() != null) sb.append("  ").append(e.getDescription());
                sb.append('\n');
            }
            File out = new File(appContext.getFilesDir(), "exit_info.txt");
            try (java.io.FileOutputStream fo = new java.io.FileOutputStream(out)) {
                fo.write(sb.toString().getBytes(java.nio.charset.StandardCharsets.UTF_8));
            }
        } catch (Throwable ignored) {}
    }

    private static String exitReasonName(int r) {
        switch (r) {
            case android.app.ApplicationExitInfo.REASON_ANR: return "ANR";
            case android.app.ApplicationExitInfo.REASON_CRASH: return "CRASH";
            case android.app.ApplicationExitInfo.REASON_CRASH_NATIVE: return "CRASH_NATIVE";
            case android.app.ApplicationExitInfo.REASON_DEPENDENCY_DIED: return "DEPENDENCY_DIED";
            case android.app.ApplicationExitInfo.REASON_EXCESSIVE_RESOURCE_USAGE: return "EXCESSIVE_RESOURCE_USAGE";
            case android.app.ApplicationExitInfo.REASON_EXIT_SELF: return "EXIT_SELF";
            case android.app.ApplicationExitInfo.REASON_INITIALIZATION_FAILURE: return "INITIALIZATION_FAILURE";
            case android.app.ApplicationExitInfo.REASON_LOW_MEMORY: return "LOW_MEMORY";
            case android.app.ApplicationExitInfo.REASON_OTHER: return "OTHER";
            case android.app.ApplicationExitInfo.REASON_PERMISSION_CHANGE: return "PERMISSION_CHANGE";
            case android.app.ApplicationExitInfo.REASON_SIGNALED: return "SIGNALED (killed by signal)";
            case android.app.ApplicationExitInfo.REASON_USER_REQUESTED: return "USER_REQUESTED";
            case android.app.ApplicationExitInfo.REASON_USER_STOPPED: return "USER_STOPPED";
            default: return "UNKNOWN(" + r + ")";
        }
    }

    /** Delete every file reportFiles() lists, so the next report only holds the next run. Returns the count removed. */
    public int clearLogs() {
        int n = 0;
        for (File f : reportFiles()) if (f.delete()) n++;
        return n;
    }

    public String readLastCrash() {
        StringBuilder sb = new StringBuilder();
        if (debugMode) sb.append("Kullanici: ").append(username).append(" (debug ayari acik)\n\n");
        // which engine library produced this report: size and import time tell the builds apart
        if (extLibPath != null && !extLibPath.isEmpty()) {
            File so = new File(extLibPath);
            sb.append("Motor: ").append(so.getName()).append(' ').append(so.length()).append(" bayt, ")
              .append(new java.util.Date(so.lastModified())).append("; profilleyici env=")
              .append(System.getenv("MONO_ALLOC_PROFILE")).append("\n\n");
        }
        File exit = new File(appContext.getFilesDir(), "exit_info.txt");
        if (exit.isFile() && exit.length() > 0) {
            sb.append("== Android'in son cikis nedenleri ==\n").append(readTail(exit, 2048)).append("\n");
        }
        File lim = new File(gameDir == null ? "" : gameDir, "memory_limit.txt");
        if (lim.isFile()) {
            sb.append("== Bellek siniri (").append(new java.util.Date(lim.lastModified())).append(") ==\n")
              .append(readTail(lim, 4096)).append("\n");
        }
        File mcr = newestCrashReport();
        if (mcr != null) {
            sb.append("== Minecraft cokme raporu: ").append(mcr.getName()).append(" ==\n")
              .append(readHead(mcr, 24 * 1024)).append("\n");
        }
        File nat = new File(gameDir == null ? "" : gameDir, "native_crash.txt");
        if (nat.isFile()) {
            sb.append("== Native cokme (").append(new java.util.Date(nat.lastModified())).append(") ==\n")
              .append(readTail(nat, 16 * 1024)).append("\n");
        }
        File f = new File(appContext.getFilesDir(), "last_crash.txt");
        if (f.isFile()) {
            sb.append("== Java cokme (").append(new java.util.Date(f.lastModified())).append(") ==\n")
              .append(readTail(f, 16 * 1024)).append("\n");
        }
        File log = new File(gameDir == null ? "" : gameDir, "engine_log.txt");
        if (log.isFile()) {
            sb.append("== Motor ciktisi, son 24 KB (").append(new java.util.Date(log.lastModified())).append(") ==\n")
              .append(readTail(log, 24 * 1024));
        }
        return sb.length() == 0 ? "Kayitli cokme yok." : sb.toString();
    }

    private static String readHead(File f, int max) {
        try (java.io.FileInputStream in = new java.io.FileInputStream(f)) {
            byte[] buf = new byte[(int) Math.min(max, f.length())];
            int off = 0, r;
            while (off < buf.length && (r = in.read(buf, off, buf.length - off)) > 0) off += r;
            return new String(buf, 0, off, StandardCharsets.UTF_8);
        } catch (Throwable t) {
            return "okunamadi: " + t.getMessage();
        }
    }

    private static String readTail(File f, int max) {
        try (java.io.RandomAccessFile raf = new java.io.RandomAccessFile(f, "r")) {
            long len = raf.length();
            long start = Math.max(0, len - max);
            byte[] buf = new byte[(int) (len - start)];
            raf.seek(start);
            raf.readFully(buf);
            return new String(buf, StandardCharsets.UTF_8);
        } catch (Throwable t) {
            return "okunamadi: " + t.getMessage();
        }
    }
}
