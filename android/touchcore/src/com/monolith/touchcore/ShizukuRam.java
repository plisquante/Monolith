package com.monolith.touchcore;

import android.content.pm.PackageManager;

import java.io.ByteArrayOutputStream;
import java.io.File;
import java.io.FileOutputStream;
import java.io.InputStream;
import java.lang.reflect.Method;
import java.nio.charset.StandardCharsets;

import rikka.shizuku.Shizuku;

/**
 * "Android RAM kısıtlamalarını gevşet": shell-level (adb) commands run through the Shizuku app, before the game
 * starts. Android's low-memory killer itself cannot be changed without root; what a shell user can do is free RAM
 * (stop cached background apps) and keep Android's background policies (doze, standby buckets, phantom process
 * limits) from restricting or stopping the game.
 */
final class ShizukuRam {
    static final int REQUEST_CODE = 4103;

    private static final String PKG = "com.monolith.touchcore";
    private static final String[] COMMANDS = {
        // stop every cached background process (what Android would kill first anyway): their RAM goes to the game
        "am kill-all",
        // no doze / app-standby restrictions for the game
        "dumpsys deviceidle whitelist +" + PKG,
        "cmd appops set " + PKG + " RUN_ANY_IN_BACKGROUND allow",
        "am set-standby-bucket " + PKG + " active",
        // Android 12+ phantom process monitor (kills child processes of apps over a global limit)
        "settings put global settings_enable_monitor_phantom_procs false",
    };

    private ShizukuRam() {}

    /** Shizuku app running (its service reachable from this app). */
    static boolean running() {
        try {
            return Shizuku.pingBinder();
        } catch (Throwable t) {
            return false;
        }
    }

    static boolean granted() {
        try {
            return running() && Shizuku.checkSelfPermission() == PackageManager.PERMISSION_GRANTED;
        } catch (Throwable t) {
            return false;
        }
    }

    /** Asks Shizuku for permission; the answer arrives at the listener. */
    static void request(Shizuku.OnRequestPermissionResultListener listener) {
        Shizuku.addRequestPermissionResultListener(listener);
        Shizuku.requestPermission(REQUEST_CODE);
    }

    /**
     * Runs the commands (blocking: call off the UI thread) and returns a log of each command's exit code and output.
     * The log is also written to {@code gameDir/shizuku.txt} for the crash report.
     */
    static String apply(String gameDir) {
        StringBuilder log = new StringBuilder("Shizuku RAM ayarı\n");
        if (!granted()) {
            log.append(running() ? "Shizuku izni verilmemiş: komutlar çalıştırılmadı\n"
                                 : "Shizuku çalışmıyor: komutlar çalıştırılmadı\n");
        } else {
            for (String cmd : COMMANDS) {
                log.append("$ ").append(cmd).append('\n');
                try {
                    Process p = newProcess(new String[] {"sh", "-c", cmd});
                    String out = readAll(p.getInputStream()) + readAll(p.getErrorStream());
                    int rc = p.waitFor();
                    if (!out.isEmpty()) log.append(out.trim()).append('\n');
                    log.append("-> çıkış kodu ").append(rc).append('\n');
                } catch (Throwable t) {
                    log.append("-> HATA: ").append(t).append('\n');
                }
            }
        }
        try (FileOutputStream os = new FileOutputStream(new File(gameDir, "shizuku.txt"))) {
            os.write(log.toString().getBytes(StandardCharsets.UTF_8));
        } catch (Exception ignored) {
            // the log is diagnostic only; the game starts either way
        }
        return log.toString();
    }

    /** Shizuku.newProcess is private in API 13 (the library steers to UserService); it is the documented shell path. */
    private static Process newProcess(String[] cmd) throws Exception {
        Method m = Shizuku.class.getDeclaredMethod("newProcess", String[].class, String[].class, String.class);
        m.setAccessible(true);
        return (Process) m.invoke(null, cmd, null, null);
    }

    private static String readAll(InputStream in) throws Exception {
        ByteArrayOutputStream bo = new ByteArrayOutputStream();
        byte[] buf = new byte[4096];
        int n;
        while ((n = in.read(buf)) > 0) bo.write(buf, 0, n);
        return bo.toString("UTF-8");
    }
}
