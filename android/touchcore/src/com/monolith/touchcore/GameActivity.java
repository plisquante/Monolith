package com.monolith.touchcore;

import android.app.Activity;
import android.app.AlertDialog;
import android.content.Context;
import android.content.DialogInterface;
import android.content.Intent;
import android.os.Build;
import android.os.Bundle;
import android.os.Handler;
import android.os.Looper;
import android.util.DisplayMetrics;
import android.view.KeyEvent;
import android.view.SurfaceHolder;
import android.view.SurfaceView;
import android.view.View;
import android.view.ViewGroup;
import android.view.WindowInsets;
import android.view.WindowInsetsController;
import android.view.WindowManager;
import android.text.InputType;
import android.view.inputmethod.BaseInputConnection;
import android.view.inputmethod.EditorInfo;
import android.view.inputmethod.InputConnection;
import android.view.inputmethod.InputMethodManager;
import android.widget.EditText;
import android.widget.FrameLayout;
import android.widget.Toast;

import java.util.List;

public final class GameActivity extends Activity implements SurfaceHolder.Callback {
    private LauncherConfig config;
    private ControlProfile activeProfile;
    private SurfaceView surfaceView;
    private TouchOverlayView overlayView;
    private ImeCatcher hiddenImeInput;
    private boolean engineInitialized = false;
    private int initStatus = 0;

    private final Handler mainHandler = new Handler(Looper.getMainLooper());
    private final long[] telemetryBuf = new long[16];
    private final Runnable telemetryTick = new Runnable() {
        @Override
        public void run() {
            if (!isFinishing() && NativeBridge.isLoaded()) {
                try {
                    NativeBridge.nativePollTelemetry(telemetryBuf);
                    if (overlayView != null) {
                        overlayView.updateTelemetry(telemetryBuf);
                    }
                } catch (Throwable ignored) {}
                mainHandler.postDelayed(this, 250);
            }
        }
    };

    @Override
    protected void onCreate(Bundle savedInstanceState) {
        super.onCreate(savedInstanceState);
        LauncherConfig.installCrashHandler(this);
        getWindow().addFlags(WindowManager.LayoutParams.FLAG_KEEP_SCREEN_ON);
        useWholeScreen();
        config = new LauncherConfig(this);
        activeProfile = config.loadActiveProfile();
        new java.io.File(config.gameDir, "display_info.txt").delete(); // one report per launch
        new java.io.File(config.gameDir, "memory_limit.txt").delete();
        new java.io.File(config.gameDir, "shizuku.txt").delete();
        new java.io.File(config.gameDir, "metrics.csv").delete();
        if (config.shizukuRam || config.debugMode) {
            // shell commands through Shizuku (RAM for the game, no background restrictions); the engine needs seconds
            // before it allocates much, so they run alongside its start-up
            final String gd = config.gameDir;
            new Thread(new Runnable() {
                @Override public void run() { ShizukuRam.apply(gd); }
            }, "shizuku-ram").start();
        }
        java.io.File[] oldShots = new java.io.File(config.gameDir).listFiles();
        if (oldShots != null) for (java.io.File f : oldShots) if (f.getName().startsWith("frame_") && f.getName().endsWith(".png")) f.delete();

        if (!NativeBridge.isLoaded()) {
            showFatalInitDialog("Native motor yüklenemedi",
                    "libtouchcore_mono.so yüklenirken hata oluştu:\n" + NativeBridge.getLoadError());
            return;
        }

        boolean extRequested = config.extLibPath != null && !config.extLibPath.trim().isEmpty();
        if (extRequested) {
            try {
                EngineResources.exportEnv(this, config);
            } catch (java.io.IOException e) {
                showFatalInitDialog("Motor ortami hazirlanamadi", String.valueOf(e.getMessage()));
                return;
            }
        }
        try {
            initStatus = NativeBridge.nativeInitEngine(
                    config.username,
                    config.extLibPath,
                    config.gameDir,
                    config.resourcesDir,
                    config.assetsDir,
                    config.backendMode,
                    config.powerProfile,
                    config.renderDistance,
                    config.targetFps,
                    config.fovDeg);
            engineInitialized = true;
        } catch (Throwable t) {
            showFatalInitDialog("Motor Başlatma Hatası", t.getClass().getSimpleName() + ": " + t.getMessage());
            return;
        }

        if (initStatus < 0 || (extRequested && initStatus != 1)) {
            // Do NOT silently fall back to Demo Mode when an external engine was explicitly selected.
            String detail = NativeBridge.safeGetLastLog();
            NativeBridge.nativeShutdown();
            engineInitialized = false;
            showFatalInitDialog(
                    "Harici MONOLITH Motoru Yüklenemedi",
                    "Seçilen kütüphane açılamadı veya 'mono_android_entry' başlatılamadı.\n"
                            + "Demo Moduna sessizce düşülmedi.\n\n"
                            + "Dosya: " + config.extLibPath + "\nHata kodu: " + initStatus + "\nDetay: " + detail);
            return;
        }

        NativeBridge.nativeUpdateRuntimeConfig(
                config.sensitivity,
                config.invertY,
                config.fovDeg,
                config.renderDistance,
                config.targetFps);

        FrameLayout root = new FrameLayout(this);
        root.setBackgroundColor(0xFF000000);

        hiddenImeInput = new ImeCatcher(this);
        hiddenImeInput.setAlpha(0f);
        root.addView(hiddenImeInput, new FrameLayout.LayoutParams(1, 1));

        surfaceView = new SurfaceView(this);
        SurfaceHolder holder = surfaceView.getHolder();
        holder.addCallback(this);

        if (config.resScalePct > 0 && config.resScalePct < 100) {
            // the whole physical screen (system bars and the camera cutout included), not the app area of DisplayMetrics
            int[] real = realScreenSize();
            int rawW = Math.max(real[0], real[1]);
            int rawH = Math.min(real[0], real[1]);
            int scaledW = Math.max(640, (rawW * config.resScalePct) / 100);
            int scaledH = Math.max(360, (rawH * config.resScalePct) / 100);
            holder.setFixedSize(scaledW, scaledH);
        }

        overlayView = new TouchOverlayView(this, config, activeProfile);
        if (initStatus == 1) {
            overlayView.setEngineModeLabel("MONOLITH Motor (libminecraft_mono.so)");
        } else {
            overlayView.setEngineModeLabel("DEMO MODU (Monolith RHI Voxel)");
        }
        overlayView.setHostCallbacks(new TouchOverlayView.HostCallbacks() {
            @Override
            public void onOpenQuickMenu() {
                showQuickMenu();
            }

            @Override
            public void onToggleSoftKeyboard() {
                toggleKeyboard();
            }
        });

        root.addView(surfaceView, new FrameLayout.LayoutParams(
                ViewGroup.LayoutParams.MATCH_PARENT, ViewGroup.LayoutParams.MATCH_PARENT));
        root.addView(overlayView, new FrameLayout.LayoutParams(
                ViewGroup.LayoutParams.MATCH_PARENT, ViewGroup.LayoutParams.MATCH_PARENT));

        setContentView(root);
    }

    private void showFatalInitDialog(String title, String message) {
        FrameLayout empty = new FrameLayout(this);
        empty.setBackgroundColor(0xFF0B1117);
        setContentView(empty);
        new AlertDialog.Builder(this, android.R.style.Theme_DeviceDefault_Dialog_Alert)
                .setTitle(title)
                .setMessage(message)
                .setCancelable(false)
                .setPositiveButton("Başlatıcıya Dön", new DialogInterface.OnClickListener() {
                    @Override public void onClick(DialogInterface dialog, int which) { finish(); }
                })
                .setNeutralButton("Demo Moduna Geç", new DialogInterface.OnClickListener() {
                    @Override public void onClick(DialogInterface dialog, int which) {
                        config.extLibPath = "";
                        config.save();
                        recreate();
                    }
                })
                .show();
    }

    @Override
    public void onWindowFocusChanged(boolean hasFocus) {
        super.onWindowFocusChanged(hasFocus);
        if (hasFocus) {
            enterFullscreenSafe();
        }
    }

    /**
     * The game surface covers the entire display: drawn under the (hidden) status and navigation bars and into the
     * camera cutout. Without this the window keeps the area of the navigation bar and the game renders at a smaller
     * width than the screen (2229 instead of 2340 pixels on a phone with on-screen buttons).
     */
    private void useWholeScreen() {
        if (Build.VERSION.SDK_INT >= 28) {
            WindowManager.LayoutParams lp = getWindow().getAttributes();
            lp.layoutInDisplayCutoutMode = Build.VERSION.SDK_INT >= 30
                    ? WindowManager.LayoutParams.LAYOUT_IN_DISPLAY_CUTOUT_MODE_ALWAYS
                    : WindowManager.LayoutParams.LAYOUT_IN_DISPLAY_CUTOUT_MODE_SHORT_EDGES;
            getWindow().setAttributes(lp);
        }
        getWindow().addFlags(WindowManager.LayoutParams.FLAG_LAYOUT_IN_SCREEN
                | WindowManager.LayoutParams.FLAG_LAYOUT_NO_LIMITS);
        if (Build.VERSION.SDK_INT >= 30) {
            getWindow().setDecorFitsSystemWindows(false);
        } else {
            getWindow().getDecorView().setSystemUiVisibility(
                    View.SYSTEM_UI_FLAG_LAYOUT_STABLE
                            | View.SYSTEM_UI_FLAG_LAYOUT_HIDE_NAVIGATION
                            | View.SYSTEM_UI_FLAG_LAYOUT_FULLSCREEN
                            | View.SYSTEM_UI_FLAG_FULLSCREEN
                            | View.SYSTEM_UI_FLAG_HIDE_NAVIGATION
                            | View.SYSTEM_UI_FLAG_IMMERSIVE_STICKY);
        }
        enterFullscreenSafe();
    }

    private int[] realScreenSize() {
        if (Build.VERSION.SDK_INT >= 30) {
            android.graphics.Rect b = getWindowManager().getCurrentWindowMetrics().getBounds();
            return new int[] {b.width(), b.height()};
        }
        DisplayMetrics dm = new DisplayMetrics();
        getWindowManager().getDefaultDisplay().getRealMetrics(dm);
        return new int[] {dm.widthPixels, dm.heightPixels};
    }

    private void enterFullscreenSafe() {
        try {
            if (Build.VERSION.SDK_INT >= 30) {
                WindowInsetsController c = getWindow().getInsetsController();
                if (c != null) {
                    c.hide(WindowInsets.Type.statusBars() | WindowInsets.Type.navigationBars());
                    c.setSystemBarsBehavior(WindowInsetsController.BEHAVIOR_SHOW_TRANSIENT_BARS_BY_SWIPE);
                }
            } else {
                getWindow().getDecorView().setSystemUiVisibility(
                        View.SYSTEM_UI_FLAG_LAYOUT_STABLE
                                | View.SYSTEM_UI_FLAG_LAYOUT_HIDE_NAVIGATION
                                | View.SYSTEM_UI_FLAG_LAYOUT_FULLSCREEN
                                | View.SYSTEM_UI_FLAG_FULLSCREEN
                                | View.SYSTEM_UI_FLAG_HIDE_NAVIGATION
                                | View.SYSTEM_UI_FLAG_IMMERSIVE_STICKY);
            }
        } catch (Throwable ignored) {}
    }

    private void toggleKeyboard() {
        if (hiddenImeInput == null) return;
        hiddenImeInput.requestFocus();
        InputMethodManager imm = (InputMethodManager) getSystemService(Context.INPUT_METHOD_SERVICE);
        if (imm != null) {
            imm.toggleSoftInput(InputMethodManager.SHOW_FORCED, 0);
        }
    }

    @Override
    protected void onResume() {
        super.onResume();
        if (!engineInitialized) return;
        config.load();
        activeProfile = config.loadActiveProfile();
        if (overlayView != null) {
            overlayView.setProfile(activeProfile);
        }
        NativeBridge.nativeUpdateRuntimeConfig(
                config.sensitivity,
                config.invertY,
                config.fovDeg,
                config.renderDistance,
                config.targetFps);
        mainHandler.removeCallbacks(telemetryTick);
        mainHandler.postDelayed(telemetryTick, 200);
    }

    @Override
    protected void onPause() {
        mainHandler.removeCallbacks(telemetryTick);
        if (overlayView != null) {
            overlayView.releaseAllInputs();
        }
        super.onPause();
    }

    @Override
    protected void onDestroy() {
        mainHandler.removeCallbacks(telemetryTick);
        if (engineInitialized && NativeBridge.isLoaded()) {
            NativeBridge.nativeShutdown();
            engineInitialized = false;
        }
        super.onDestroy();
    }

    @Override
    public void surfaceCreated(SurfaceHolder holder) {
        if (engineInitialized && NativeBridge.isLoaded()) {
            NativeBridge.nativeAttachSurface(holder.getSurface());
        }
    }

    @Override
    public void surfaceChanged(SurfaceHolder holder, int format, int width, int height) {
        if (engineInitialized && NativeBridge.isLoaded()) {
            NativeBridge.nativeResizeSurface(width, height);
        }
        writeDisplayInfo("surfaceChanged " + width + "x" + height);
        startSampler();
    }

    private boolean samplerRunning;
    private android.os.HandlerThread samplerThread;

    /**
     * Every 2 s: sizes (the EGL surface exists only after the engine's first swap) and memory of this process. If
     * Android kills the process (low memory), the last lines show how far memory had grown; a freeze shows as
     * unchanged values.
     */
    private void startSampler() {
        if (samplerRunning || surfaceView == null) return;
        samplerRunning = true;
        final long t0 = System.currentTimeMillis();
        // The sampler runs on its own thread: it used to run on the UI thread, where the two Debug.getMemoryInfo calls
        // per tick (full /proc/self/smaps walk plus the memtrack HAL) froze input and the cursor for a moment every 2 s.
        samplerThread = new android.os.HandlerThread("mono-sampler");
        samplerThread.start();
        final android.os.Handler samplerHandler = new android.os.Handler(samplerThread.getLooper());
        samplerHandler.post(new Runnable() {
            // previous sample, for the per-interval CPU, FPS figures of metrics.csv
            long lastWallMs = -1, lastCpuTicks = -1;
            int lastSwaps = -1;

            @Override public void run() {
                if (isFinishing() || surfaceView == null) { samplerRunning = false; if (samplerThread != null) samplerThread.quit(); return; }
                writeMetrics();
                long[] mem = readMemKb();
                android.app.ActivityManager.MemoryInfo sys = new android.app.ActivityManager.MemoryInfo();
                ((android.app.ActivityManager) getSystemService(Context.ACTIVITY_SERVICE)).getMemoryInfo(sys);
                long pssMb = mem[0] / 1024;
                long limitMb = (sys.totalMem >> 20) * Math.max(30, Math.min(80, config.memLimitPct)) / 100;
                String line = "+" + ((System.currentTimeMillis() - t0) / 1000) + " s: process memory " + pssMb
                        + " MB (native " + (mem[1] / 1024) + " MB), system free " + (sys.availMem >> 20)
                        + " MB, low-memory " + sys.lowMemory + ", limit " + limitMb + " MB";
                writeDisplayInfo(line);
                // With the Shizuku setting the cached apps were stopped for the game: Android's low-memory flag then
                // only means the game is using what was freed for it, so only the game's own limit stops it.
                if (pssMb > limitMb || (sys.lowMemory && !(config.shizukuRam || config.debugMode))) {
                    // Past the limit Android starts killing other apps and the system UI (widgets reload) before it
                    // kills the game. Stop the game first and leave the reason for the next start of the launcher.
                    try {
                        java.io.FileOutputStream os = new java.io.FileOutputStream(new java.io.File(config.gameDir, "memory_limit.txt"));
                        os.write(("Oyun durduruldu: bellek " + pssMb + " MB, sinir " + limitMb + " MB (Ayarlar > Bellek Sınırı), "
                                + "sistemde bos " + (sys.availMem >> 20) + " MB, low-memory " + sys.lowMemory + "\n" + line + "\n")
                                .getBytes(java.nio.charset.StandardCharsets.UTF_8));
                        os.close();
                    } catch (Throwable ignored) { }
                    android.os.Process.killProcess(android.os.Process.myPid());
                    return;
                }
                samplerHandler.postDelayed(this, 2000);
            }

            /** One row of <game dir>/metrics.csv per sample: memory, process CPU, GPU load, frames per second. */
            private void writeMetrics() {
                try {
                    long now = System.currentTimeMillis();
                    long[] mem = readMemKb();
                    android.app.ActivityManager.MemoryInfo s = new android.app.ActivityManager.MemoryInfo();
                    ((android.app.ActivityManager) getSystemService(Context.ACTIVITY_SERVICE)).getMemoryInfo(s);
                    long cpuTicks = processCpuTicks();
                    int[] z = NativeBridge.isLoaded() ? NativeBridge.nativeSurfaceSizes() : null;
                    int swaps = z != null && z.length > 4 ? z[4] : -1;
                    double cpuPct = -1, fps = -1;
                    if (lastWallMs > 0 && now > lastWallMs) {
                        double dt = (now - lastWallMs) / 1000.0;
                        long hz = android.system.Os.sysconf(android.system.OsConstants._SC_CLK_TCK);
                        int cores = Runtime.getRuntime().availableProcessors();
                        if (cpuTicks >= 0 && lastCpuTicks >= 0 && hz > 0)
                            cpuPct = 100.0 * (cpuTicks - lastCpuTicks) / hz / dt / cores; // share of the whole CPU
                        if (swaps >= 0 && lastSwaps >= 0) fps = (swaps - lastSwaps) / dt;
                    }
                    lastWallMs = now;
                    lastCpuTicks = cpuTicks;
                    lastSwaps = swaps;
                    java.io.File f = new java.io.File(config.gameDir, "metrics.csv");
                    boolean head = !f.isFile();
                    java.io.FileOutputStream os = new java.io.FileOutputStream(f, true);
                    StringBuilder sb = new StringBuilder();
                    if (head) {
                        sb.append("# device ").append(Build.MANUFACTURER).append(' ').append(Build.MODEL)
                          .append(" android ").append(Build.VERSION.SDK_INT).append(" ram_mb ").append(s.totalMem >> 20)
                          .append(" cores ").append(Runtime.getRuntime().availableProcessors());
                        if (config.debugMode) sb.append(" user ").append(config.username);
                        sb.append("\nt_s,pss_mb,native_mb,sys_free_mb,cpu_pct,gpu_pct,fps\n");
                    }
                    sb.append((now - t0) / 1000).append(',').append(mem[0] / 1024).append(',')
                      .append(mem[1] / 1024).append(',').append(s.availMem >> 20).append(',')
                      .append(String.format(java.util.Locale.ROOT, "%.1f", cpuPct)).append(',').append(gpuBusyPct()).append(',')
                      .append(String.format(java.util.Locale.ROOT, "%.1f", fps)).append('\n');
                    os.write(sb.toString().getBytes(java.nio.charset.StandardCharsets.UTF_8));
                    os.close();
                } catch (Throwable ignored) {
                    // diagnostics only: a failed sample never stops the game
                }
            }
        });
    }

    /** utime + stime of this process in clock ticks (/proc/self/stat fields 14, 15), -1 if unreadable. */
    /**
     * {pss, pssAnon} in kB from /proc/self/smaps_rollup: a cheap kernel summary. Debug.getMemoryInfo walks the whole
     * smaps (slow with thousands of mappings) and calls the memtrack HAL, which aborted the process on Android 17.
     * pss is the process PSS without GPU-driver memory; pssAnon is anonymous memory (the engine's heap lives there).
     */
    private static long[] readMemKb() {
        long pss = -1, anon = -1;
        try (java.io.BufferedReader r = new java.io.BufferedReader(new java.io.FileReader("/proc/self/smaps_rollup"))) {
            String line;
            while ((line = r.readLine()) != null) {
                if (line.startsWith("Pss:")) pss = parseKb(line);
                else if (line.startsWith("Pss_Anon:")) anon = parseKb(line);
            }
        } catch (Throwable ignored) { }
        if (pss < 0) pss = android.os.Debug.getPss(); // fallback: also a kernel summary, no HAL
        return new long[] {Math.max(pss, 0), Math.max(anon, 0)};
    }

    private static long parseKb(String line) {
        try { return Long.parseLong(line.substring(line.indexOf(':') + 1).replace("kB", "").trim()); } catch (Throwable t) { return -1; }
    }

    private static long processCpuTicks() {
        try {
            String st = new String(java.nio.file.Files.readAllBytes(java.nio.file.Paths.get("/proc/self/stat")),
                    java.nio.charset.StandardCharsets.US_ASCII);
            String[] f = st.substring(st.lastIndexOf(')') + 2).split(" "); // fields from 3 (state) on
            return Long.parseLong(f[11]) + Long.parseLong(f[12]);
        } catch (Throwable t) {
            return -1;
        }
    }

    /** GPU load in percent from the kernel's GPU driver node (Samsung Exynos/Xclipse, Mali), -1 if not readable. */
    private static int gpuBusyPct() {
        String[] nodes = {"/sys/kernel/gpu/gpu_busy", "/sys/class/misc/mali0/device/utilization", "/sys/class/kgsl/kgsl-3d0/gpu_busy_percentage"};
        for (String n : nodes) {
            try {
                String v = new String(java.nio.file.Files.readAllBytes(java.nio.file.Paths.get(n)),
                        java.nio.charset.StandardCharsets.US_ASCII).trim();
                StringBuilder d = new StringBuilder();
                for (char c : v.toCharArray()) { if (Character.isDigit(c)) d.append(c); else if (d.length() > 0) break; }
                if (d.length() > 0) return Math.min(100, Integer.parseInt(d.toString()));
            } catch (Throwable ignored) {
                // not present or not readable by apps on this device
            }
        }
        return -1;
    }

    /** <game dir>/display_info.txt: every size involved, to compare when the picture does not fit the phone. */
    private void writeDisplayInfo(String why) {
        try {
            StringBuilder sb = new StringBuilder();
            sb.append(why).append('\n');
            if (config.debugMode) sb.append("user ").append(config.username).append('\n');
            sb.append("device ").append(Build.MANUFACTURER).append(' ').append(Build.MODEL).append(" android ")
                    .append(Build.VERSION.SDK_INT).append('\n');
            DisplayMetrics dm = getResources().getDisplayMetrics();
            sb.append("app metrics ").append(dm.widthPixels).append('x').append(dm.heightPixels)
                    .append(" density ").append(dm.density).append(" dpi ").append(dm.densityDpi).append('\n');
            int[] real = realScreenSize();
            sb.append("window/real size ").append(real[0]).append('x').append(real[1]).append('\n');
            if (Build.VERSION.SDK_INT >= 30) {
                android.graphics.Rect max = getWindowManager().getMaximumWindowMetrics().getBounds();
                sb.append("maximum window ").append(max.width()).append('x').append(max.height()).append('\n');
            }
            if (surfaceView != null) {
                sb.append("surface view ").append(surfaceView.getWidth()).append('x').append(surfaceView.getHeight()).append('\n');
            }
            sb.append("resolution scale ").append(config.resScalePct).append("%\n");
            if (NativeBridge.isLoaded()) {
                try {
                    int[] z = NativeBridge.nativeSurfaceSizes();
                    if (z != null && z.length == 4) {
                        sb.append("engine drawable ").append(z[0]).append('x').append(z[1]).append(", EGL surface ")
                                .append(z[2]).append('x').append(z[3]).append('\n');
                    }
                } catch (Throwable ignored) { }
            }
            java.io.File f = new java.io.File(config.gameDir, "display_info.txt");
            java.io.FileOutputStream os = new java.io.FileOutputStream(f, true);
            os.write((sb.toString() + "\n").getBytes(java.nio.charset.StandardCharsets.UTF_8));
            os.close();
        } catch (Throwable ignored) { }
    }

    @Override
    public void surfaceDestroyed(SurfaceHolder holder) {
        if (overlayView != null) {
            overlayView.releaseAllInputs();
        }
        if (engineInitialized && NativeBridge.isLoaded()) {
            NativeBridge.nativeDetachSurface();
        }
    }

    @Override
    public boolean onKeyDown(int keyCode, KeyEvent event) {
        if (keyCode == KeyEvent.KEYCODE_BACK) {
            showQuickMenu();
            return true;
        }
        if (engineInitialized && NativeBridge.isLoaded()) {
            handleTextKey(event);
            int mapped = mapAndroidKeyToGlfw(keyCode);
            if (mapped != 0) {
                NativeBridge.nativeSetKey(mapped, true);
                NativeBridge.nativeSetKey(mapped, false);
                return true;
            }
        }
        return super.onKeyDown(keyCode, event);
    }

    @Override
    public boolean onKeyUp(int keyCode, KeyEvent event) {
        if (engineInitialized && NativeBridge.isLoaded() && editKeyFor(keyCode) != 0) {
            NativeBridge.nativeEditKey(editKeyFor(keyCode), 0);
            return true;
        }
        return super.onKeyUp(keyCode, event);
    }

    /** SDL keycode of an editing key (backspace, enter, delete, arrows, home, end), or 0. */
    static int editKeyFor(int keyCode) {
        switch (keyCode) {
            case KeyEvent.KEYCODE_DEL: return 8;
            case KeyEvent.KEYCODE_ENTER:
            case KeyEvent.KEYCODE_NUMPAD_ENTER: return 13;
            case KeyEvent.KEYCODE_FORWARD_DEL: return 127;
            case KeyEvent.KEYCODE_DPAD_RIGHT: return 0x4000004F;
            case KeyEvent.KEYCODE_DPAD_LEFT: return 0x40000050;
            case KeyEvent.KEYCODE_DPAD_DOWN: return 0x40000051;
            case KeyEvent.KEYCODE_DPAD_UP: return 0x40000052;
            case KeyEvent.KEYCODE_MOVE_HOME: return 0x4000004A;
            case KeyEvent.KEYCODE_MOVE_END: return 0x4000004D;
            default: return 0;
        }
    }

    /** Key down from the soft or a hardware keyboard: editing keys and the typed character reach the game as text input. */
    static void handleTextKey(KeyEvent e) {
        int ek = editKeyFor(e.getKeyCode());
        if (ek != 0) {
            NativeBridge.nativeEditKey(ek, e.getRepeatCount() > 0 ? 2 : 1);
            return;
        }
        int uc = e.getUnicodeChar();
        if (uc > 0 && !e.isCtrlPressed() && !e.isAltPressed() && Character.isValidCodePoint(uc) && !Character.isISOControl(uc)) {
            NativeBridge.nativeTypeChar(uc);
        }
    }

    /**
     * Invisible view that owns the soft keyboard. It is a plain editor with no text of its own: everything the
     * keyboard commits is forwarded to the game as characters and editing keys (an EditText kept the text to itself).
     */
    static final class ImeCatcher extends View {
        ImeCatcher(Context c) {
            super(c);
            setFocusable(true);
            setFocusableInTouchMode(true);
        }

        @Override public boolean onCheckIsTextEditor() { return true; }

        @Override
        public InputConnection onCreateInputConnection(EditorInfo ei) {
            // visible-password + no suggestions: keyboards commit each character at once instead of composing words
            ei.inputType = InputType.TYPE_CLASS_TEXT | InputType.TYPE_TEXT_FLAG_NO_SUGGESTIONS
                    | InputType.TYPE_TEXT_VARIATION_VISIBLE_PASSWORD;
            ei.imeOptions = EditorInfo.IME_FLAG_NO_FULLSCREEN | EditorInfo.IME_FLAG_NO_EXTRACT_UI | EditorInfo.IME_ACTION_NONE;
            return new BaseInputConnection(this, false) {
                private String composing = "";

                private void retype(String now) {
                    int p = 0;
                    while (p < composing.length() && p < now.length() && composing.charAt(p) == now.charAt(p)) p++;
                    for (int i = composing.length(); i > p; i--) editTap(8);
                    typeText(now.substring(p));
                    composing = now;
                }

                @Override public boolean setComposingText(CharSequence t, int pos) { retype(t.toString()); return true; }
                @Override public boolean commitText(CharSequence t, int pos) { retype(t.toString()); composing = ""; return true; }
                @Override public boolean finishComposingText() { composing = ""; return true; }
                @Override public boolean deleteSurroundingText(int before, int after) {
                    for (int i = 0; i < before; i++) editTap(8);
                    for (int i = 0; i < after; i++) editTap(127);
                    return true;
                }
                @Override public boolean sendKeyEvent(KeyEvent e) {
                    if (e.getAction() == KeyEvent.ACTION_DOWN) {
                        handleTextKey(e);
                    } else if (e.getAction() == KeyEvent.ACTION_UP && editKeyFor(e.getKeyCode()) != 0) {
                        NativeBridge.nativeEditKey(editKeyFor(e.getKeyCode()), 0);
                    }
                    return true;
                }
            };
        }

        private static void editTap(int sdlKeycode) {
            if (!NativeBridge.isLoaded()) return;
            NativeBridge.nativeEditKey(sdlKeycode, 1);
            NativeBridge.nativeEditKey(sdlKeycode, 0);
        }

        private static void typeText(String s) {
            if (!NativeBridge.isLoaded()) return;
            for (int i = 0; i < s.length(); ) {
                int cp = s.codePointAt(i);
                i += Character.charCount(cp);
                if (cp == '\n') editTap(13);
                else if (!Character.isISOControl(cp)) NativeBridge.nativeTypeChar(cp);
            }
        }
    }

    private static int mapAndroidKeyToGlfw(int keyCode) {
        if (keyCode >= KeyEvent.KEYCODE_A && keyCode <= KeyEvent.KEYCODE_Z) {
            return 'A' + (keyCode - KeyEvent.KEYCODE_A);
        }
        if (keyCode >= KeyEvent.KEYCODE_0 && keyCode <= KeyEvent.KEYCODE_9) {
            return '0' + (keyCode - KeyEvent.KEYCODE_0);
        }
        if (keyCode == KeyEvent.KEYCODE_SPACE) return GlfwKeys.KEY_SPACE;
        if (keyCode == KeyEvent.KEYCODE_ENTER) return GlfwKeys.KEY_ENTER;
        if (keyCode == KeyEvent.KEYCODE_DEL) return GlfwKeys.KEY_BACKSPACE;
        if (keyCode == KeyEvent.KEYCODE_ESCAPE) return GlfwKeys.KEY_ESCAPE;
        if (keyCode == KeyEvent.KEYCODE_TAB) return GlfwKeys.KEY_TAB;
        return 0;
    }

    private void showQuickMenu() {
        if (overlayView != null) overlayView.releaseAllInputs();
        String modeStr = (initStatus == 1) ? "Harici Motor (libminecraft_mono.so)" : "DEMO MODU (Monolith RHI)";
        String[] items = new String[] {
            (overlayView != null && overlayView.isShowF3Hud())
                    ? "F3 Telemetry Gizle" : "F3 Telemetry Göster",
            "Kontrol Profilini Değiştir (" + config.activeProfileName + ")",
            "Tuş Düzeni Editörünü Aç",
            "Sohbet / Komut Gönder",
            "Native Motor Günlüğü",
            "Başlatıcıya Dön (Çıkış)"
        };
        new AlertDialog.Builder(this, android.R.style.Theme_DeviceDefault_Dialog_Alert)
                .setTitle("TouchCore — " + config.username + " • " + modeStr)
                .setItems(items, new DialogInterface.OnClickListener() {
                    @Override public void onClick(DialogInterface dialog, int which) {
                        switch (which) {
                            case 0:
                                if (overlayView != null) overlayView.toggleF3Hud();
                                break;
                            case 1:
                                showProfileSwitcher();
                                break;
                            case 2:
                                Intent i = new Intent(GameActivity.this, ControlsEditorActivity.class);
                                i.putExtra("profile_name", config.activeProfileName);
                                startActivity(i);
                                break;
                            case 3:
                                showQuickCommandDialog();
                                break;
                            case 4:
                                new AlertDialog.Builder(GameActivity.this, android.R.style.Theme_DeviceDefault_Dialog_Alert)
                                        .setTitle("TouchCore Native Log")
                                        .setMessage(NativeBridge.safeGetLastLog())
                                        .setPositiveButton("Tamam", null)
                                        .show();
                                break;
                            case 5:
                                finish();
                                break;
                        }
                    }
                })
                .setNegativeButton("Devam Et", null)
                .show();
    }

    private void showProfileSwitcher() {
        List<String> names = config.listProfileNames();
        final String[] arr = names.toArray(new String[0]);
        new AlertDialog.Builder(this, android.R.style.Theme_DeviceDefault_Dialog_Alert)
                .setTitle("Kontrol Profili Seç")
                .setItems(arr, new DialogInterface.OnClickListener() {
                    @Override public void onClick(DialogInterface dialog, int which) {
                        config.activeProfileName = arr[which];
                        config.save();
                        activeProfile = config.loadActiveProfile();
                        if (overlayView != null) overlayView.setProfile(activeProfile);
                        Toast.makeText(GameActivity.this, "Profil: " + arr[which], Toast.LENGTH_SHORT).show();
                    }
                })
                .show();
    }

    private void showQuickCommandDialog() {
        final EditText input = new EditText(this);
        input.setHint("Ör: /gamemode creative veya mesaj");
        new AlertDialog.Builder(this, android.R.style.Theme_DeviceDefault_Dialog_Alert)
                .setTitle("Sohbet / Komut Gönder")
                .setView(input)
                .setPositiveButton("Gönder", new DialogInterface.OnClickListener() {
                    @Override public void onClick(DialogInterface dialog, int which) {
                        if (!NativeBridge.isLoaded()) return;
                        NativeBridge.nativeSetKey(GlfwKeys.KEY_T, true);
                        NativeBridge.nativeSetKey(GlfwKeys.KEY_T, false);
                        String msg = input.getText().toString();
                        for (int i = 0; i < msg.length(); i++) {
                            char ch = Character.toUpperCase(msg.charAt(i));
                            if ((ch >= 'A' && ch <= 'Z') || (ch >= '0' && ch <= '9')) {
                                NativeBridge.nativeSetKey(ch, true);
                                NativeBridge.nativeSetKey(ch, false);
                            } else if (ch == ' ') {
                                NativeBridge.nativeSetKey(GlfwKeys.KEY_SPACE, true);
                                NativeBridge.nativeSetKey(GlfwKeys.KEY_SPACE, false);
                            }
                        }
                        NativeBridge.nativeSetKey(GlfwKeys.KEY_ENTER, true);
                        NativeBridge.nativeSetKey(GlfwKeys.KEY_ENTER, false);
                    }
                })
                .setNegativeButton("İptal", null)
                .show();
    }
}
