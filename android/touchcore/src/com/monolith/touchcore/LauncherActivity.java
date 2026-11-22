package com.monolith.touchcore;

import android.app.Activity;
import android.app.AlertDialog;
import android.content.ClipData;
import android.content.ClipboardManager;
import android.content.Context;
import android.content.DialogInterface;
import android.content.Intent;
import android.graphics.Typeface;
import android.graphics.drawable.GradientDrawable;
import android.net.Uri;
import android.os.Bundle;
import android.text.Editable;
import android.text.TextWatcher;
import android.view.Gravity;
import android.view.View;
import android.view.ViewGroup;
import android.widget.Button;
import android.widget.CompoundButton;
import android.widget.EditText;
import android.widget.FrameLayout;
import android.widget.HorizontalScrollView;
import android.widget.ImageView;
import android.widget.LinearLayout;
import android.widget.ScrollView;
import android.widget.SeekBar;
import android.widget.Switch;
import android.widget.TextView;
import android.widget.Toast;

import java.io.File;
import java.io.FileOutputStream;
import java.io.InputStream;
import java.util.List;
import java.util.Locale;
import java.util.Random;

public final class LauncherActivity extends Activity {
    private static final int REQ_PICK_SO = 4001;
    private static final int REQ_EXPORT_REPORTS = 4002;

    private LauncherConfig config;
    private ControlProfile activeProfile;

    private FrameLayout contentHost;
    private final Button[] tabButtons = new Button[4];
    private int currentTab = 0;

    private SkinAvatarView avatarView;
    private EditText usernameInput;
    private TextView usernameStatus;
    private TextView engineModeBadge;
    private TextView playSummaryText;
    private LinearLayout recentUsersRow;

    private TextView extLibStatusText;
    private TextView probeOutputText;
    private TextView logOutputText;
    private TextView crashOutputText;

    // Material 3 dark colour scheme (tonal surfaces, green seed). Names kept so every screen picks up the tokens.
    private static final int COL_BG = 0xFF101412;       // surface
    private static final int COL_HEADER = 0xFF101412;   // top app bar: same as surface
    private static final int COL_CARD = 0xFF1C211E;     // surfaceContainer
    private static final int COL_SURFACE = 0xFF262B28;  // surfaceContainerHigh
    private static final int COL_BORDER = 0xFF404943;   // outlineVariant
    private static final int COL_ACCENT = 0xFF78DC9B;   // primary
    private static final int COL_ON_ACCENT = 0xFF003919;        // onPrimary
    private static final int COL_ACCENT_CONTAINER = 0xFF00522A; // primaryContainer
    private static final int COL_ON_ACCENT_CONTAINER = 0xFF94F8B5;
    private static final int COL_NAV = 0xFF0B0F0D;      // surfaceContainerLowest
    private static final int COL_BLUE = 0xFF82CFE6;     // tertiary
    private static final int COL_AMBER = 0xFFFBBF24;
    private static final int COL_TEXT = 0xFFE0E3DE;     // onSurface
    private static final int COL_MUTED = 0xFFC0C9C0;    // onSurfaceVariant

    @Override
    protected void onCreate(Bundle savedInstanceState) {
        super.onCreate(savedInstanceState);
        LauncherConfig.installCrashHandler(this);
        config = new LauncherConfig(this);
        config.recordExitInfo();
        activeProfile = config.loadActiveProfile();

        LinearLayout root = new LinearLayout(this);
        root.setOrientation(LinearLayout.VERTICAL);
        root.setBackgroundColor(COL_BG);
        root.setFitsSystemWindows(true);
        getWindow().setStatusBarColor(COL_BG);
        getWindow().setNavigationBarColor(COL_NAV);

        root.addView(buildHeader());
        root.addView(buildTabBar());

        contentHost = new FrameLayout(this);
        contentHost.setLayoutParams(new LinearLayout.LayoutParams(
                ViewGroup.LayoutParams.MATCH_PARENT, 0, 1f));
        root.addView(contentHost);

        setContentView(root);
        selectTab(0);
        showNewCrashReport();
    }

    /** After a crash, the next start shows the report once (native signal, Java exception, engine output) to share. */
    private void showNewCrashReport() {
        java.io.File mc = config.newestCrashReport();
        java.io.File[] files = {new java.io.File(config.gameDir == null ? "" : config.gameDir, "native_crash.txt"),
                new java.io.File(getFilesDir(), "last_crash.txt"), mc,
                new java.io.File(config.gameDir == null ? "" : config.gameDir, "memory_limit.txt")};
        long newest = 0;
        for (java.io.File f : files) if (f != null && f.isFile()) newest = Math.max(newest, f.lastModified());
        android.content.SharedPreferences sp = getSharedPreferences("crash_report", MODE_PRIVATE);
        if (newest == 0 || newest <= sp.getLong("shown", 0)) return;
        sp.edit().putLong("shown", newest).apply();
        final String report = config.readLastCrash();
        TextView tv = new TextView(this);
        tv.setText(report);
        tv.setTextIsSelectable(true);
        tv.setTextSize(11f);
        tv.setTypeface(android.graphics.Typeface.MONOSPACE);
        tv.setPadding(dp(12), dp(8), dp(12), dp(8));
        android.widget.ScrollView sv = new android.widget.ScrollView(this);
        sv.addView(tv);
        new AlertDialog.Builder(this, android.R.style.Theme_DeviceDefault_Dialog_Alert)
                .setTitle("Son calistirma coktu")
                .setView(sv)
                .setPositiveButton("Paylas", new android.content.DialogInterface.OnClickListener() {
                    @Override public void onClick(android.content.DialogInterface d, int w) {
                        Intent send = new Intent(Intent.ACTION_SEND);
                        send.setType("text/plain");
                        send.putExtra(Intent.EXTRA_TEXT, report);
                        startActivity(Intent.createChooser(send, "Cokme raporunu paylas"));
                    }
                })
                .setNeutralButton("Disa aktar", new android.content.DialogInterface.OnClickListener() {
                    @Override public void onClick(android.content.DialogInterface d, int w) { startReportExport(); }
                })
                .setNegativeButton("Kapat", null)
                .show();
    }

    private long totalRamMb() {
        android.app.ActivityManager.MemoryInfo mi = new android.app.ActivityManager.MemoryInfo();
        ((android.app.ActivityManager) getSystemService(ACTIVITY_SERVICE)).getMemoryInfo(mi);
        return mi.totalMem >> 20;
    }

    /** Lets the user pick where to save a zip of every report (system file picker: no storage permission needed). */
    private void startReportExport() {
        Intent intent = new Intent(Intent.ACTION_CREATE_DOCUMENT);
        intent.addCategory(Intent.CATEGORY_OPENABLE);
        intent.setType("application/zip");
        String stamp = new java.text.SimpleDateFormat("yyyyMMdd-HHmmss", java.util.Locale.US).format(new java.util.Date());
        intent.putExtra(Intent.EXTRA_TITLE, "monolith-rapor-" + stamp + ".zip");
        startActivityForResult(intent, REQ_EXPORT_REPORTS);
    }

    private void writeReportZip(Uri uri) {
        int n = 0;
        try (java.io.OutputStream os = getContentResolver().openOutputStream(uri);
             java.util.zip.ZipOutputStream zip = new java.util.zip.ZipOutputStream(os)) {
            if (os == null) throw new java.io.IOException("Hedef dosya acilamadi");
            for (java.io.File f : config.reportFiles()) {
                zip.putNextEntry(new java.util.zip.ZipEntry(f.getParentFile().getName() + "/" + f.getName()));
                try (java.io.FileInputStream in = new java.io.FileInputStream(f)) {
                    byte[] buf = new byte[16384];
                    int r;
                    while ((r = in.read(buf)) > 0) zip.write(buf, 0, r);
                }
                zip.closeEntry();
                n++;
            }
            zip.putNextEntry(new java.util.zip.ZipEntry("ozet.txt"));
            zip.write((NativeBridge.safeProbeHardware() + "\n\n" + config.readLastCrash()).getBytes(java.nio.charset.StandardCharsets.UTF_8));
            zip.closeEntry();
            Toast.makeText(this, n + " dosya disa aktarildi", Toast.LENGTH_LONG).show();
        } catch (Throwable t) {
            new AlertDialog.Builder(this, android.R.style.Theme_DeviceDefault_Dialog_Alert)
                    .setTitle("Disa aktarma basarisiz")
                    .setMessage(String.valueOf(t.getMessage()))
                    .setPositiveButton("Tamam", null)
                    .show();
        }
    }

    @Override
    protected void onResume() {
        super.onResume();
        config.load();
        activeProfile = config.loadActiveProfile();
        selectTab(currentTab);
    }

    private int dp(int v) {
        return Math.round(v * getResources().getDisplayMetrics().density);
    }

    private GradientDrawable roundedBox(int fill, int stroke, int radiusDp) {
        GradientDrawable d = new GradientDrawable();
        d.setColor(fill);
        d.setStroke(dp(1), stroke);
        d.setCornerRadius(dp(radiusDp));
        return d;
    }

    private View buildHeader() {
        LinearLayout header = new LinearLayout(this);
        header.setOrientation(LinearLayout.HORIZONTAL);
        header.setGravity(Gravity.CENTER_VERTICAL);
        header.setPadding(dp(16), dp(12), dp(16), dp(12));
        header.setBackgroundColor(COL_HEADER);

        ImageView logo = new ImageView(this);
        logo.setImageResource(R.drawable.monolith_logo);
        logo.setScaleType(ImageView.ScaleType.FIT_START);
        LinearLayout.LayoutParams logoLp = new LinearLayout.LayoutParams(dp(148), dp(37));
        logoLp.rightMargin = dp(10);
        header.addView(logo, logoLp);

        LinearLayout rightCol = new LinearLayout(this);
        rightCol.setOrientation(LinearLayout.VERTICAL);
        rightCol.setGravity(Gravity.END | Gravity.CENTER_VERTICAL);
        rightCol.setLayoutParams(new LinearLayout.LayoutParams(0, ViewGroup.LayoutParams.WRAP_CONTENT, 1f));

        TextView title = new TextView(this);
        title.setText("TouchCore Launcher");
        title.setTextColor(COL_TEXT);
        title.setTypeface(Typeface.DEFAULT_BOLD);
        title.setTextSize(16f);
        title.setGravity(Gravity.END);

        TextView badge = new TextView(this);
        badge.setText("MONOLITH ARM64 • v2.0");
        badge.setTextColor(COL_ACCENT);
        badge.setTextSize(10.5f);
        badge.setTypeface(Typeface.MONOSPACE);
        badge.setGravity(Gravity.END);

        rightCol.addView(title);
        rightCol.addView(badge);
        header.addView(rightCol);
        return header;
    }

    private View buildTabBar() {
        LinearLayout bar = new LinearLayout(this);
        bar.setOrientation(LinearLayout.HORIZONTAL);
        bar.setBackgroundColor(COL_NAV);
        bar.setPadding(dp(8), dp(8), dp(8), dp(10));

        String[] labels = {"Oyna", "Kontroller", "Ayarlar", "Sistem"};
        for (int i = 0; i < labels.length; i++) {
            final int tabIdx = i;
            Button b = new Button(this);
            b.setText(labels[i]);
            b.setTextSize(13f);
            b.setTypeface(Typeface.DEFAULT_BOLD);
            b.setAllCaps(false);
            b.setStateListAnimator(null);
            LinearLayout.LayoutParams lp = new LinearLayout.LayoutParams(0, dp(40), 1f);
            lp.setMargins(dp(3), 0, dp(3), 0);
            b.setLayoutParams(lp);
            b.setOnClickListener(new View.OnClickListener() {
                @Override public void onClick(View v) {
                    saveTextInputsIfPresent();
                    selectTab(tabIdx);
                }
            });
            tabButtons[i] = b;
            bar.addView(b);
        }
        return bar;
    }

    private void selectTab(int index) {
        currentTab = index;
        for (int i = 0; i < tabButtons.length; i++) {
            boolean active = (i == index);
            // M3 navigation: the selected destination sits in a primary-container pill, the rest are plain text
            int fill = active ? COL_ACCENT_CONTAINER : COL_NAV;
            tabButtons[i].setBackground(roundedBox(fill, fill, 20));
            tabButtons[i].setTextColor(active ? COL_ON_ACCENT_CONTAINER : COL_MUTED);
        }
        contentHost.removeAllViews();
        ScrollView scroll = new ScrollView(this);
        LinearLayout page = new LinearLayout(this);
        page.setOrientation(LinearLayout.VERTICAL);
        page.setPadding(dp(14), dp(10), dp(14), dp(24));

        if (index == 0) buildPlayTab(page);
        else if (index == 1) buildControlsTab(page);
        else if (index == 2) buildSettingsTab(page);
        else buildSystemTab(page);

        scroll.addView(page);
        contentHost.addView(scroll);
    }

    private LinearLayout makeCard(String heading) {
        LinearLayout card = new LinearLayout(this);
        card.setOrientation(LinearLayout.VERTICAL);
        card.setBackground(roundedBox(COL_CARD, COL_CARD, 16));
        card.setPadding(dp(16), dp(14), dp(16), dp(16));
        LinearLayout.LayoutParams lp = new LinearLayout.LayoutParams(
                ViewGroup.LayoutParams.MATCH_PARENT, ViewGroup.LayoutParams.WRAP_CONTENT);
        lp.bottomMargin = dp(12);
        card.setLayoutParams(lp);

        if (heading != null && !heading.isEmpty()) {
            TextView h = new TextView(this);
            h.setText(heading);
            h.setTextColor(COL_ACCENT);
            h.setTextSize(13f);
            h.setLetterSpacing(0.04f);
            h.setTypeface(Typeface.DEFAULT_BOLD);
            h.setPadding(0, 0, 0, dp(8));
            card.addView(h);
        }
        return card;
    }

    private Button makeStyledBtn(String text, int bg, int border) {
        Button b = new Button(this);
        b.setText(text);
        // Material 3 roles; every call site already names its role through the outline colour it passed
        int fill;
        int ink;
        if (border == COL_ACCENT) {            // filled (primary)
            fill = COL_ACCENT; ink = COL_ON_ACCENT;
        } else if (border == COL_BLUE || border == 0xFF2DD4BF) { // tonal (tertiary container)
            fill = 0xFF004E5D; ink = 0xFFB1ECFF;
        } else if (border == 0xFFEF4444) {     // destructive (error container)
            fill = 0xFF93000A; ink = 0xFFFFDAD6;
        } else {                               // tonal surface
            fill = COL_SURFACE; ink = COL_TEXT;
        }
        b.setTextColor(ink);
        b.setTextSize(13f);
        b.setAllCaps(false);
        b.setTypeface(Typeface.DEFAULT_BOLD);
        b.setStateListAnimator(null);
        b.setBackground(roundedBox(fill, fill, 20)); // M3 button: full pill, no outline
        b.setPadding(dp(18), dp(8), dp(18), dp(8));
        return b;
    }

    private void saveTextInputsIfPresent() {
        if (usernameInput != null) {
            String candidate = LauncherConfig.sanitizeUsername(usernameInput.getText().toString().trim());
            if (LauncherConfig.isValidUsername(candidate)) {
                config.username = candidate;
                config.save();
            }
        }
    }

    private boolean hasValidExtLib() {
        return config.extLibPath != null && !config.extLibPath.trim().isEmpty()
                && new File(config.extLibPath).isFile();
    }

    private void buildPlayTab(LinearLayout page) {
        LinearLayout userCard = makeCard("OYUNCU PROFİLİ & İSİM");
        LinearLayout userRow = new LinearLayout(this);
        userRow.setOrientation(LinearLayout.HORIZONTAL);
        userRow.setGravity(Gravity.CENTER_VERTICAL);

        avatarView = new SkinAvatarView(this);
        avatarView.setUsername(config.username);
        LinearLayout.LayoutParams avLp = new LinearLayout.LayoutParams(dp(64), dp(64));
        avLp.rightMargin = dp(12);
        userRow.addView(avatarView, avLp);

        LinearLayout inputCol = new LinearLayout(this);
        inputCol.setOrientation(LinearLayout.VERTICAL);
        inputCol.setLayoutParams(new LinearLayout.LayoutParams(0, ViewGroup.LayoutParams.WRAP_CONTENT, 1f));

        usernameInput = new EditText(this);
        usernameInput.setText(config.username);
        usernameInput.setHint("Oyuncu Adı (3-16 karakter)");
        usernameInput.setHintTextColor(COL_MUTED);
        usernameInput.setTextColor(COL_TEXT);
        usernameInput.setTextSize(14f);
        usernameInput.setSingleLine(true);
        usernameInput.setBackground(roundedBox(0xFF0A0F17, COL_BORDER, 9));
        usernameInput.setPadding(dp(10), dp(9), dp(10), dp(9));

        usernameStatus = new TextView(this);
        usernameStatus.setTextSize(11f);
        usernameStatus.setPadding(0, dp(5), 0, 0);

        inputCol.addView(usernameInput);
        inputCol.addView(usernameStatus);
        userRow.addView(inputCol);

        Button randBtn = makeStyledBtn("Zar", COL_SURFACE, COL_BORDER);
        LinearLayout.LayoutParams randLp = new LinearLayout.LayoutParams(dp(56), dp(42));
        randLp.leftMargin = dp(8);
        randBtn.setOnClickListener(new View.OnClickListener() {
            @Override public void onClick(View v) {
                usernameInput.setText("Mono_" + (100 + new Random().nextInt(8999)));
            }
        });
        userRow.addView(randBtn, randLp);
        userCard.addView(userRow);

        TextView recentLbl = new TextView(this);
        recentLbl.setText("Hızlı Profil Seçimi:");
        recentLbl.setTextColor(COL_MUTED);
        recentLbl.setTextSize(11f);
        recentLbl.setPadding(0, dp(10), 0, dp(5));
        userCard.addView(recentLbl);

        HorizontalScrollView chipsScroll = new HorizontalScrollView(this);
        recentUsersRow = new LinearLayout(this);
        recentUsersRow.setOrientation(LinearLayout.HORIZONTAL);
        rebuildRecentChips();
        chipsScroll.addView(recentUsersRow);
        userCard.addView(chipsScroll);

        usernameInput.addTextChangedListener(new TextWatcher() {
            @Override public void beforeTextChanged(CharSequence s, int start, int count, int after) {}
            @Override public void onTextChanged(CharSequence s, int start, int before, int count) {}
            @Override public void afterTextChanged(Editable s) {
                validateUsernameUi(s.toString().trim());
            }
        });
        page.addView(userCard);

        LinearLayout modeCard = makeCard("MOTOR & OYUN SÜRÜMÜ");
        engineModeBadge = new TextView(this);
        engineModeBadge.setTextSize(12.5f);
        engineModeBadge.setTypeface(Typeface.DEFAULT_BOLD);
        engineModeBadge.setPadding(dp(10), dp(8), dp(10), dp(8));
        modeCard.addView(engineModeBadge);

        LinearLayout modeBtns = new LinearLayout(this);
        modeBtns.setOrientation(LinearLayout.HORIZONTAL);
        modeBtns.setPadding(0, dp(8), 0, 0);

        Button pickLibBtn = makeStyledBtn(".so Motor Seç", 0xFF1E3A8A, COL_BLUE);
        pickLibBtn.setLayoutParams(new LinearLayout.LayoutParams(0, dp(40), 1f));
        pickLibBtn.setOnClickListener(new View.OnClickListener() {
            @Override public void onClick(View v) { launchSoPicker(); }
        });

        Button mojangBtn = makeStyledBtn("Mojang İndir", 0xFF0F766E, 0xFF2DD4BF);
        LinearLayout.LayoutParams mLp = new LinearLayout.LayoutParams(0, dp(40), 1f);
        mLp.leftMargin = dp(6);
        mojangBtn.setLayoutParams(mLp);
        mojangBtn.setOnClickListener(new View.OnClickListener() {
            @Override public void onClick(View v) { openMojangDownloader(); }
        });

        Button demoToggleBtn = makeStyledBtn("Demo Modu", COL_SURFACE, COL_BORDER);
        LinearLayout.LayoutParams dLp = new LinearLayout.LayoutParams(0, dp(40), 1f);
        dLp.leftMargin = dp(6);
        demoToggleBtn.setLayoutParams(dLp);
        demoToggleBtn.setOnClickListener(new View.OnClickListener() {
            @Override public void onClick(View v) {
                config.extLibPath = "";
                config.save();
                validateUsernameUi(usernameInput.getText().toString().trim());
                Toast.makeText(LauncherActivity.this, "DEMO MODU seçildi.", Toast.LENGTH_SHORT).show();
            }
        });
        modeBtns.addView(pickLibBtn);
        modeBtns.addView(mojangBtn);
        modeBtns.addView(demoToggleBtn);
        modeCard.addView(modeBtns);
        page.addView(modeCard);

        LinearLayout summaryCard = makeCard("KONTROL & GRAFİK ÖZETİ");
        playSummaryText = new TextView(this);
        playSummaryText.setTextColor(COL_TEXT);
        playSummaryText.setTextSize(12.5f);
        playSummaryText.setLineSpacing(dp(4), 1f);
        summaryCard.addView(playSummaryText);

        LinearLayout quickRow = new LinearLayout(this);
        quickRow.setOrientation(LinearLayout.HORIZONTAL);
        quickRow.setPadding(0, dp(10), 0, 0);

        Button editHudBtn = makeStyledBtn("MJLauncher HUD Düzenle", 0xFF1E3A8A, COL_BLUE);
        editHudBtn.setLayoutParams(new LinearLayout.LayoutParams(0, dp(42), 1f));
        editHudBtn.setOnClickListener(new View.OnClickListener() {
            @Override public void onClick(View v) {
                saveTextInputsIfPresent();
                Intent i = new Intent(LauncherActivity.this, ControlsEditorActivity.class);
                i.putExtra("profile_name", config.activeProfileName);
                startActivity(i);
            }
        });

        Button settingsBtn = makeStyledBtn("Grafik / FPS Ayarları", COL_SURFACE, COL_BORDER);
        LinearLayout.LayoutParams sLp = new LinearLayout.LayoutParams(0, dp(42), 1f);
        sLp.leftMargin = dp(8);
        settingsBtn.setLayoutParams(sLp);
        settingsBtn.setOnClickListener(new View.OnClickListener() {
            @Override public void onClick(View v) {
                saveTextInputsIfPresent();
                selectTab(2);
            }
        });

        quickRow.addView(editHudBtn);
        quickRow.addView(settingsBtn);
        summaryCard.addView(quickRow);
        page.addView(summaryCard);

        Button playBtn = makeStyledBtn("▶  OYUNU BAŞLAT", 0xFF16A34A, COL_ACCENT);
        playBtn.setTextSize(15.5f);
        LinearLayout.LayoutParams playLp = new LinearLayout.LayoutParams(
                ViewGroup.LayoutParams.MATCH_PARENT, dp(56));
        playLp.topMargin = dp(4);
        playBtn.setLayoutParams(playLp);
        playBtn.setOnClickListener(new View.OnClickListener() {
            @Override public void onClick(View v) {
                String raw = usernameInput.getText().toString().trim();
                if (!LauncherConfig.isValidUsername(raw)) {
                    Toast.makeText(LauncherActivity.this,
                            "Geçerli bir oyuncu adı girin (3-16 karakter: A-Z, 0-9, _).",
                            Toast.LENGTH_LONG).show();
                    return;
                }
                config.username = raw;
                config.save();
                if (config.extLibPath == null || config.extLibPath.trim().isEmpty()) {
                    startActivity(new Intent(LauncherActivity.this, GameActivity.class));
                } else {
                    prepareEngineThenLaunch();
                }
            }
        });
        page.addView(playBtn);

        validateUsernameUi(config.username);
    }

    private void rebuildRecentChips() {
        if (recentUsersRow == null) return;
        recentUsersRow.removeAllViews();
        for (final String u : config.recentUsernames) {
            Button chip = new Button(this);
            chip.setText(u);
            chip.setAllCaps(false);
            chip.setTextSize(11.5f);
            boolean sel = u.equals(config.username);
            chip.setTextColor(sel ? COL_ACCENT : COL_TEXT);
            chip.setBackground(roundedBox(COL_SURFACE, sel ? COL_ACCENT : COL_BORDER, 16));
            chip.setPadding(dp(12), dp(2), dp(12), dp(2));
            LinearLayout.LayoutParams lp = new LinearLayout.LayoutParams(
                    ViewGroup.LayoutParams.WRAP_CONTENT, dp(30));
            lp.rightMargin = dp(6);
            chip.setLayoutParams(lp);
            chip.setOnClickListener(new View.OnClickListener() {
                @Override public void onClick(View v) {
                    usernameInput.setText(u);
                    config.username = u;
                    config.save();
                    rebuildRecentChips();
                }
            });
            recentUsersRow.addView(chip);
        }
    }

    private void validateUsernameUi(String text) {
        boolean ok = LauncherConfig.isValidUsername(text);
        if (avatarView != null) {
            avatarView.setUsername(ok ? text : config.username);
        }
        if (usernameStatus != null) {
            if (ok) {
                usernameStatus.setText("Hazır: " + text);
                usernameStatus.setTextColor(COL_ACCENT);
                config.username = text;
            } else {
                usernameStatus.setText("3-16 karakter olmalı (A-Z, 0-9, _)");
                usernameStatus.setTextColor(0xFFFCA5A5);
            }
        }
        if (engineModeBadge != null) {
            boolean extSet = config.extLibPath != null && !config.extLibPath.trim().isEmpty();
            boolean hasJar = config.clientJarPath != null && !config.clientJarPath.isEmpty()
                    && new File(config.clientJarPath).isFile();
            String jarSuffix = hasJar ? ("\nİndirilen JAR: " + config.selectedVersion + " ("
                    + (new File(config.clientJarPath).length() / (1024 * 1024)) + " MB)") : "";
            if (extSet && new File(config.extLibPath).isFile()) {
                engineModeBadge.setText("GERÇEK MONOLITH MOTORU: " + new File(config.extLibPath).getName() + jarSuffix);
                engineModeBadge.setTextColor(COL_ACCENT);
                engineModeBadge.setBackground(roundedBox(0xFF0E291D, COL_ACCENT, 8));
            } else if (extSet) {
                engineModeBadge.setText("HATA: Seçilen .so bulunamadı (" + config.extLibPath + ")" + jarSuffix);
                engineModeBadge.setTextColor(0xFFFCA5A5);
                engineModeBadge.setBackground(roundedBox(0xFF331115, 0xFFEF4444, 8));
            } else {
                engineModeBadge.setText("DEMO MODU • Yerleşik Monolith RHI 3D Voxel (Harici libminecraft_mono.so yok)" + jarSuffix);
                engineModeBadge.setTextColor(COL_AMBER);
                engineModeBadge.setBackground(roundedBox(0xFF281E0B, COL_AMBER, 8));
            }
        }
        updatePlaySummary();
    }

    private void updatePlaySummary() {
        if (playSummaryText == null) return;
        String backend = config.backendMode == LauncherConfig.BACKEND_VULKAN ? "GLES + Vulkan RHI"
                : (config.backendMode == LauncherConfig.BACKEND_GLES ? "GLES 3.2" : "Oto (GLES)");
        String padMode = activeProfile.joystickMode == ControlProfile.STICK_DPAD ? "Çapraz (+) D-Pad" : "Analog Joystick";
        playSummaryText.setText(String.format(Locale.US,
                "Kontrol Profili: %s\nHareket: %s • %d MJLauncher Tuşu\n"
                        + "Backend: %s • %d Chunk • %s FPS • Ölçek %%%d",
                config.activeProfileName,
                padMode,
                activeProfile.buttons.size(),
                backend,
                config.renderDistance,
                config.targetFps == 0 ? "Max" : String.valueOf(config.targetFps),
                config.resScalePct));
    }

    private void buildControlsTab(LinearLayout page) {
        LinearLayout profCard = makeCard("MJLAUNCHER KONTROL PROFİLİ");

        Button pickProfileBtn = makeStyledBtn("Aktif Profil: " + config.activeProfileName + "  ▾", COL_SURFACE, COL_BLUE);
        pickProfileBtn.setLayoutParams(new LinearLayout.LayoutParams(
                ViewGroup.LayoutParams.MATCH_PARENT, dp(44)));
        pickProfileBtn.setOnClickListener(new View.OnClickListener() {
            @Override public void onClick(View v) {
                final List<String> names = config.listProfileNames();
                final String[] arr = names.toArray(new String[0]);
                new AlertDialog.Builder(LauncherActivity.this, android.R.style.Theme_DeviceDefault_Dialog_Alert)
                        .setTitle("Kontrol Profili Seç")
                        .setItems(arr, new DialogInterface.OnClickListener() {
                            @Override public void onClick(DialogInterface dialog, int which) {
                                config.activeProfileName = arr[which];
                                config.save();
                                activeProfile = config.loadActiveProfile();
                                selectTab(1);
                            }
                        })
                        .show();
            }
        });
        profCard.addView(pickProfileBtn);

        Button openEditorBtn = makeStyledBtn("GÖRSEL HUD EDİTÖRÜNÜ AÇ", 0xFF1D4ED8, COL_BLUE);
        LinearLayout.LayoutParams edLp = new LinearLayout.LayoutParams(
                ViewGroup.LayoutParams.MATCH_PARENT, dp(48));
        edLp.topMargin = dp(10);
        edLp.bottomMargin = dp(8);
        openEditorBtn.setLayoutParams(edLp);
        openEditorBtn.setOnClickListener(new View.OnClickListener() {
            @Override public void onClick(View v) {
                Intent i = new Intent(LauncherActivity.this, ControlsEditorActivity.class);
                i.putExtra("profile_name", config.activeProfileName);
                startActivity(i);
            }
        });
        profCard.addView(openEditorBtn);

        LinearLayout row1 = new LinearLayout(this);
        row1.setOrientation(LinearLayout.HORIZONTAL);
        Button dupBtn = makeStyledBtn("Yeni / Kopyala", COL_SURFACE, COL_BORDER);
        dupBtn.setLayoutParams(new LinearLayout.LayoutParams(0, dp(38), 1f));
        dupBtn.setOnClickListener(new View.OnClickListener() {
            @Override public void onClick(View v) { promptCloneProfile(); }
        });
        Button delBtn = makeStyledBtn("Sil", 0xFF450A0A, 0xFFEF4444);
        LinearLayout.LayoutParams delLp = new LinearLayout.LayoutParams(0, dp(38), 1f);
        delLp.leftMargin = dp(6);
        delBtn.setLayoutParams(delLp);
        delBtn.setOnClickListener(new View.OnClickListener() {
            @Override public void onClick(View v) {
                if (!config.deleteProfile(config.activeProfileName)) {
                    Toast.makeText(LauncherActivity.this,
                            "Yerleşik profiller silinemez; sıfırlayabilirsiniz.", Toast.LENGTH_SHORT).show();
                } else {
                    activeProfile = config.loadActiveProfile();
                    selectTab(1);
                }
            }
        });
        row1.addView(dupBtn);
        row1.addView(delBtn);
        profCard.addView(row1);

        LinearLayout row2 = new LinearLayout(this);
        row2.setOrientation(LinearLayout.HORIZONTAL);
        row2.setPadding(0, dp(6), 0, 0);
        Button exportBtn = makeStyledBtn("JSON Paylaş", COL_SURFACE, COL_BORDER);
        exportBtn.setLayoutParams(new LinearLayout.LayoutParams(0, dp(38), 1f));
        exportBtn.setOnClickListener(new View.OnClickListener() {
            @Override public void onClick(View v) { exportProfileJson(); }
        });
        Button importBtn = makeStyledBtn("JSON İçe Aktar", COL_SURFACE, COL_BORDER);
        LinearLayout.LayoutParams impLp = new LinearLayout.LayoutParams(0, dp(38), 1f);
        impLp.leftMargin = dp(6);
        importBtn.setLayoutParams(impLp);
        importBtn.setOnClickListener(new View.OnClickListener() {
            @Override public void onClick(View v) { importProfileJson(); }
        });
        Button resetBtn = makeStyledBtn("Sıfırla", COL_SURFACE, COL_BORDER);
        LinearLayout.LayoutParams rstLp = new LinearLayout.LayoutParams(0, dp(38), 1f);
        rstLp.leftMargin = dp(6);
        resetBtn.setLayoutParams(rstLp);
        resetBtn.setOnClickListener(new View.OnClickListener() {
            @Override public void onClick(View v) {
                activeProfile = config.resetProfileToDefault(config.activeProfileName);
                Toast.makeText(LauncherActivity.this, "Profil varsayılana döndü.", Toast.LENGTH_SHORT).show();
                selectTab(1);
            }
        });
        row2.addView(exportBtn);
        row2.addView(importBtn);
        row2.addView(resetBtn);
        profCard.addView(row2);
        page.addView(profCard);

        LinearLayout feelCard = makeCard("HAREKET TİPİ VE HASSASİYET");

        addOptionGroup(feelCard, "Hareket Kontrolü:",
                new String[]{"Çapraz (+) D-Pad", "Analog Joystick"},
                activeProfile.joystickMode == ControlProfile.STICK_DPAD ? 0 : 1,
                new OptionChosen() {
                    @Override public void onPick(int idx) {
                        activeProfile.joystickMode = (idx == 0) ? ControlProfile.STICK_DPAD : ControlProfile.STICK_ANALOG;
                        config.saveProfile(activeProfile);
                    }
                });

        addStyledSwitch(feelCard, "Dinamik Sol Dokunuş Joystick", activeProfile.dynamicJoystick,
                new CompoundButton.OnCheckedChangeListener() {
                    @Override public void onCheckedChanged(CompoundButton b, boolean c) {
                        activeProfile.dynamicJoystick = c;
                        config.saveProfile(activeProfile);
                    }
                });

        addStyledSwitch(feelCard, "Ters Y Ekseni (Kamera)", config.invertY,
                new CompoundButton.OnCheckedChangeListener() {
                    @Override public void onCheckedChanged(CompoundButton b, boolean c) {
                        config.invertY = c;
                        config.save();
                    }
                });

        addStyledSwitch(feelCard, "Dokunmatik Titreşim (Haptic)", config.hapticLevel > 0,
                new CompoundButton.OnCheckedChangeListener() {
                    @Override public void onCheckedChanged(CompoundButton b, boolean c) {
                        config.hapticLevel = c ? 1 : 0;
                        config.save();
                    }
                });

        addStyledSwitch(feelCard, "Oyun Başında F3 Telemetry Göster", config.autoF3,
                new CompoundButton.OnCheckedChangeListener() {
                    @Override public void onCheckedChanged(CompoundButton b, boolean c) {
                        config.autoF3 = c;
                        config.save();
                    }
                });

        addSliderRow(feelCard, "Kamera Hassasiyeti", 27,
                Math.round((config.sensitivity - 0.3f) * 10f),
                new SliderCallback() {
                    @Override public String formatValue(int progress) {
                        return String.format(Locale.US, "%.1fx", 0.3f + progress * 0.1f);
                    }
                    @Override public void onChanged(int progress) {
                        config.sensitivity = 0.3f + progress * 0.1f;
                        config.save();
                    }
                });

        page.addView(feelCard);
    }

    private interface OptionChosen {
        void onPick(int idx);
    }

    private void addOptionGroup(LinearLayout parent, String title, final String[] labels, int selectedIdx, final OptionChosen cb) {
        TextView lbl = new TextView(this);
        lbl.setText(title);
        lbl.setTextColor(COL_MUTED);
        lbl.setTextSize(11.5f);
        lbl.setPadding(0, dp(6), 0, dp(4));
        parent.addView(lbl);

        LinearLayout row = new LinearLayout(this);
        row.setOrientation(LinearLayout.HORIZONTAL);
        final Button[] btns = new Button[labels.length];
        for (int i = 0; i < labels.length; i++) {
            final int idx = i;
            Button b = new Button(this);
            b.setText(labels[i]);
            b.setAllCaps(false);
            b.setTextSize(11.5f);
            b.setTypeface(Typeface.DEFAULT_BOLD);
            LinearLayout.LayoutParams lp = new LinearLayout.LayoutParams(0, dp(36), 1f);
            if (i > 0) lp.leftMargin = dp(5);
            b.setLayoutParams(lp);
            btns[i] = b;
            b.setOnClickListener(new View.OnClickListener() {
                @Override public void onClick(View v) {
                    for (int k = 0; k < btns.length; k++) {
                        boolean act = (k == idx);
                        btns[k].setBackground(roundedBox(act ? 0xFF14532D : COL_SURFACE, act ? COL_ACCENT : COL_BORDER, 8));
                        btns[k].setTextColor(act ? 0xFFFFFFFF : COL_MUTED);
                    }
                    cb.onPick(idx);
                }
            });
            boolean act = (i == selectedIdx);
            b.setBackground(roundedBox(act ? 0xFF14532D : COL_SURFACE, act ? COL_ACCENT : COL_BORDER, 8));
            b.setTextColor(act ? 0xFFFFFFFF : COL_MUTED);
            row.addView(b);
        }
        parent.addView(row);
    }

    private void addStyledSwitch(LinearLayout parent, String text, boolean checked, CompoundButton.OnCheckedChangeListener l) {
        Switch sw = new Switch(this);
        sw.setText(text);
        sw.setTextColor(COL_TEXT);
        sw.setTextSize(12.5f);
        sw.setPadding(0, dp(6), 0, dp(6));
        sw.setChecked(checked);
        int[][] st = {{android.R.attr.state_checked}, {}};
        sw.setThumbTintList(new android.content.res.ColorStateList(st, new int[] {COL_ON_ACCENT, 0xFF8A938B}));
        sw.setTrackTintList(new android.content.res.ColorStateList(st, new int[] {COL_ACCENT, COL_SURFACE}));
        sw.setOnCheckedChangeListener(l);
        parent.addView(sw);
    }

    private interface SliderCallback {
        String formatValue(int progress);
        void onChanged(int progress);
    }

    private void addSliderRow(LinearLayout parent, final String title, int max, int initial, final SliderCallback cb) {
        final TextView lbl = new TextView(this);
        lbl.setText(title + ": " + cb.formatValue(initial));
        lbl.setTextColor(COL_TEXT);
        lbl.setTextSize(12f);
        lbl.setPadding(0, dp(10), 0, dp(2));
        parent.addView(lbl);

        SeekBar seek = new SeekBar(this);
        seek.setMax(max);
        seek.setProgress(Math.max(0, Math.min(max, initial)));
        seek.setProgressTintList(android.content.res.ColorStateList.valueOf(COL_ACCENT));
        seek.setThumbTintList(android.content.res.ColorStateList.valueOf(COL_ACCENT));
        seek.setOnSeekBarChangeListener(new SeekBar.OnSeekBarChangeListener() {
            @Override public void onProgressChanged(SeekBar s, int progress, boolean fromUser) {
                lbl.setText(title + ": " + cb.formatValue(progress));
                if (fromUser) cb.onChanged(progress);
            }
            @Override public void onStartTrackingTouch(SeekBar s) {}
            @Override public void onStopTrackingTouch(SeekBar s) {}
        });
        parent.addView(seek);
    }

    private void buildSettingsTab(LinearLayout page) {
        LinearLayout gfxCard = makeCard("GRAFİK, RHI BACKEND & FPS");

        addOptionGroup(gfxCard, "Grafik RHI Backend:",
                new String[]{"Otomatik (GLES ile cizer)", "GLES + Vulkan RHI eslikcisi", "GLES 3.2"},
                config.backendMode,
                new OptionChosen() {
                    @Override public void onPick(int idx) {
                        config.backendMode = idx;
                        config.save();
                    }
                });

        addOptionGroup(gfxCard, "Güç & İş Parçacığı Profili:",
                new String[]{"Eko", "Dengeli", "Turbo"},
                config.powerProfile,
                new OptionChosen() {
                    @Override public void onPick(int idx) {
                        config.powerProfile = idx;
                        config.save();
                    }
                });

        final int[] fpsValues = {30, 60, 90, 120, 0};
        int fpsIdx = 1;
        for (int i = 0; i < fpsValues.length; i++) {
            if (fpsValues[i] == config.targetFps) fpsIdx = i;
        }
        addOptionGroup(gfxCard, "Hedef FPS:",
                new String[]{"30", "60", "90", "120", "Max"},
                fpsIdx,
                new OptionChosen() {
                    @Override public void onPick(int idx) {
                        config.targetFps = fpsValues[idx];
                        config.save();
                    }
                });

        addSliderRow(gfxCard, "Görüş Mesafesi", 20, config.renderDistance - 4, new SliderCallback() {
            @Override public String formatValue(int progress) { return (4 + progress) + " Chunk"; }
            @Override public void onChanged(int progress) { config.renderDistance = 4 + progress; config.save(); }
        });

        addSliderRow(gfxCard, "Çözünürlük Ölçeği", 50, config.resScalePct - 50, new SliderCallback() {
            @Override public String formatValue(int progress) { return "%" + (50 + progress); }
            @Override public void onChanged(int progress) { config.resScalePct = 50 + progress; config.save(); }
        });

        final long totalMb = totalRamMb();
        addSliderRow(gfxCard, "Bellek Sınırı (oyun bunu aşınca durdurulur)", 50, config.memLimitPct - 30, new SliderCallback() {
            @Override public String formatValue(int progress) {
                return "%" + (30 + progress) + " (" + (totalMb * (30 + progress) / 100) + " MB / " + totalMb + " MB)";
            }
            @Override public void onChanged(int progress) { config.memLimitPct = 30 + progress; config.save(); }
        });

        addStyledSwitch(gfxCard, "Android RAM kısıtlamalarını gevşet (Shizuku)", config.shizukuRam,
                new CompoundButton.OnCheckedChangeListener() {
                    @Override public void onCheckedChanged(final CompoundButton b, boolean c) {
                        if (!c) {
                            config.shizukuRam = false;
                            config.save();
                            return;
                        }
                        if (!ShizukuRam.running()) {
                            b.setChecked(false);
                            Toast.makeText(LauncherActivity.this,
                                    "Shizuku çalışmıyor. Shizuku uygulamasını açıp servisi başlatın (kablosuz hata ayıklama veya adb).",
                                    Toast.LENGTH_LONG).show();
                            return;
                        }
                        if (ShizukuRam.granted()) {
                            config.shizukuRam = true;
                            config.save();
                            Toast.makeText(LauncherActivity.this,
                                    "Açık: oyun başlarken arka plan uygulamaları kapatılır, Android'in arka plan kısıtlamaları kaldırılır.",
                                    Toast.LENGTH_LONG).show();
                            return;
                        }
                        ShizukuRam.request(new rikka.shizuku.Shizuku.OnRequestPermissionResultListener() {
                            @Override public void onRequestPermissionResult(int requestCode, final int grantResult) {
                                if (requestCode != ShizukuRam.REQUEST_CODE) return;
                                rikka.shizuku.Shizuku.removeRequestPermissionResultListener(this);
                                runOnUiThread(new Runnable() {
                                    @Override public void run() {
                                        boolean ok = grantResult == android.content.pm.PackageManager.PERMISSION_GRANTED;
                                        config.shizukuRam = ok;
                                        config.save();
                                        b.setChecked(ok);
                                        Toast.makeText(LauncherActivity.this, ok ? "Shizuku izni verildi." : "Shizuku izni reddedildi.",
                                                Toast.LENGTH_LONG).show();
                                    }
                                });
                            }
                        });
                    }
                });

        addStyledSwitch(gfxCard, "Debug ayarı (raporlarda kullanıcı adı + Shizuku)", config.debugMode,
                new CompoundButton.OnCheckedChangeListener() {
                    @Override public void onCheckedChanged(CompoundButton b, boolean c) {
                        config.debugMode = c;
                        config.save();
                        if (!c) return;
                        if (!ShizukuRam.running()) {
                            Toast.makeText(LauncherActivity.this,
                                    "Debug açık: raporlara kullanıcı adı eklenecek. Shizuku çalışmıyor, onun komutları atlanacak.",
                                    Toast.LENGTH_LONG).show();
                        } else if (!ShizukuRam.granted()) {
                            ShizukuRam.request(new rikka.shizuku.Shizuku.OnRequestPermissionResultListener() {
                                @Override public void onRequestPermissionResult(int requestCode, final int grantResult) {
                                    if (requestCode != ShizukuRam.REQUEST_CODE) return;
                                    rikka.shizuku.Shizuku.removeRequestPermissionResultListener(this);
                                    runOnUiThread(new Runnable() {
                                        @Override public void run() {
                                            Toast.makeText(LauncherActivity.this,
                                                    grantResult == android.content.pm.PackageManager.PERMISSION_GRANTED
                                                            ? "Shizuku izni verildi." : "Shizuku izni reddedildi: onun komutları atlanacak.",
                                                    Toast.LENGTH_LONG).show();
                                        }
                                    });
                                }
                            });
                        } else {
                            Toast.makeText(LauncherActivity.this, "Debug açık: kullanıcı adı raporlarda, Shizuku komutları oyun başında.",
                                    Toast.LENGTH_LONG).show();
                        }
                    }
                });

        addSliderRow(gfxCard, "Görüş Açısı (FOV)", 60, Math.round(config.fovDeg - 50f), new SliderCallback() {
            @Override public String formatValue(int progress) { return (50 + progress) + "°"; }
            @Override public void onChanged(int progress) { config.fovDeg = 50f + progress; config.save(); }
        });
        page.addView(gfxCard);

        LinearLayout pathCard = makeCard("MONOLITH MOTOR (libminecraft_mono.so) & DİZİNLER");
        extLibStatusText = new TextView(this);
        extLibStatusText.setTextColor(COL_TEXT);
        extLibStatusText.setTextSize(12f);
        refreshExtLibLabel();
        pathCard.addView(extLibStatusText);

        LinearLayout soBtnRow = new LinearLayout(this);
        soBtnRow.setOrientation(LinearLayout.HORIZONTAL);
        soBtnRow.setPadding(0, dp(8), 0, dp(8));

        Button pickSoBtn = makeStyledBtn("libminecraft_mono.so İçe Aktar", 0xFF1E3A8A, COL_BLUE);
        pickSoBtn.setLayoutParams(new LinearLayout.LayoutParams(0, dp(40), 1f));
        pickSoBtn.setOnClickListener(new View.OnClickListener() {
            @Override public void onClick(View v) { launchSoPicker(); }
        });

        Button clearSoBtn = makeStyledBtn("Demo Moduna Dön", COL_SURFACE, COL_BORDER);
        LinearLayout.LayoutParams clp = new LinearLayout.LayoutParams(
                ViewGroup.LayoutParams.WRAP_CONTENT, dp(40));
        clp.leftMargin = dp(8);
        clearSoBtn.setLayoutParams(clp);
        clearSoBtn.setOnClickListener(new View.OnClickListener() {
            @Override public void onClick(View v) {
                config.extLibPath = "";
                config.save();
                refreshExtLibLabel();
            }
        });
        soBtnRow.addView(pickSoBtn);
        soBtnRow.addView(clearSoBtn);
        pathCard.addView(soBtnRow);

        final EditText gameDirEt = makePathField(pathCard, "Oyun Veri Dizini (MC_GAME_DIR)", config.gameDir);
        final EditText resDirEt = makePathField(pathCard, "Kaynak Dizini (MONO_RESOURCES)", config.resourcesDir);
        final EditText assetsDirEt = makePathField(pathCard, "Assets Dizini (MC_ASSETS_DIR)", config.assetsDir);
        final EditText envEt = makePathField(pathCard, "Ek Ortam Değişkenleri (KEY=VAL;...)", config.extraEnv);

        Button savePathsBtn = makeStyledBtn("Yolları Kaydet", 0xFF15803D, COL_ACCENT);
        LinearLayout.LayoutParams spLp = new LinearLayout.LayoutParams(
                ViewGroup.LayoutParams.MATCH_PARENT, dp(40));
        spLp.topMargin = dp(8);
        savePathsBtn.setLayoutParams(spLp);
        savePathsBtn.setOnClickListener(new View.OnClickListener() {
            @Override public void onClick(View v) {
                config.gameDir = gameDirEt.getText().toString().trim();
                config.resourcesDir = resDirEt.getText().toString().trim();
                config.assetsDir = assetsDirEt.getText().toString().trim();
                config.extraEnv = envEt.getText().toString().trim();
                config.save();
                Toast.makeText(LauncherActivity.this, "Dizin ayarları kaydedildi.", Toast.LENGTH_SHORT).show();
            }
        });
        pathCard.addView(savePathsBtn);
        page.addView(pathCard);
    }

    private void openMojangDownloader() {
        Toast.makeText(this, "Mojang sürüm listesi çekiliyor...", Toast.LENGTH_SHORT).show();
        new Thread(new Runnable() {
            @Override public void run() {
                try {
                    final List<MojangDownloader.VersionEntry> vers = MojangDownloader.fetchReleases(24);
                    runOnUiThread(new Runnable() {
                        @Override public void run() {
                            String[] labels = new String[vers.size()];
                            for (int i = 0; i < vers.size(); i++) labels[i] = vers.get(i).toString();
                            new AlertDialog.Builder(LauncherActivity.this, android.R.style.Theme_DeviceDefault_Dialog_Alert)
                                    .setTitle("Resmî Mojang Sürümü İndir")
                                    .setItems(labels, new DialogInterface.OnClickListener() {
                                        @Override public void onClick(DialogInterface dialog, int which) {
                                            downloadMojangVersion(vers.get(which));
                                        }
                                    })
                                    .setNegativeButton("İptal", null)
                                    .show();
                        }
                    });
                } catch (final Throwable t) {
                    runOnUiThread(new Runnable() {
                        @Override public void run() {
                            Toast.makeText(LauncherActivity.this,
                                    "Mojang listesi alınamadı: " + t.getMessage(), Toast.LENGTH_LONG).show();
                        }
                    });
                }
            }
        }).start();
    }

    /** Extracts the engine's resource tree (first run or after an update), then opens the game. */
    private void prepareEngineThenLaunch() {
        final AlertDialog dlg = new AlertDialog.Builder(this, android.R.style.Theme_DeviceDefault_Dialog_Alert)
                .setTitle("Motor hazirlaniyor")
                .setMessage("Kaynaklar kontrol ediliyor...")
                .setCancelable(false)
                .show();
        new Thread(new Runnable() {
            @Override public void run() {
                try {
                    EngineResources.prepare(LauncherActivity.this, config, new EngineResources.Progress() {
                        @Override public void status(final String message) {
                            runOnUiThread(new Runnable() {
                                @Override public void run() { dlg.setMessage(message); }
                            });
                        }
                    });
                    runOnUiThread(new Runnable() {
                        @Override public void run() {
                            dlg.dismiss();
                            startActivity(new Intent(LauncherActivity.this, GameActivity.class));
                        }
                    });
                } catch (final Throwable t) {
                    runOnUiThread(new Runnable() {
                        @Override public void run() {
                            dlg.dismiss();
                            new AlertDialog.Builder(LauncherActivity.this, android.R.style.Theme_DeviceDefault_Dialog_Alert)
                                    .setTitle("Oyun baslatilamadi")
                                    .setMessage(String.valueOf(t.getMessage()))
                                    .setPositiveButton("Tamam", null)
                                    .show();
                        }
                    });
                }
            }
        }).start();
    }

    private void downloadMojangVersion(final MojangDownloader.VersionEntry ver) {
        final AlertDialog progDlg = new AlertDialog.Builder(this, android.R.style.Theme_DeviceDefault_Dialog_Alert)
                .setTitle("İndiriliyor: " + ver.id)
                .setMessage("Mojang sunucularına bağlanılıyor...")
                .setCancelable(false)
                .show();
        new Thread(new Runnable() {
            @Override public void run() {
                try {
                    File versionsRoot = new File(getFilesDir(), "monolith/versions");
                    File assetsRoot = new File(config.assetsDir);
                    final File jar = MojangDownloader.downloadClientAndMetadata(
                            ver, versionsRoot, assetsRoot, new MojangDownloader.ProgressListener() {
                                @Override public void onStatus(final String msg) {
                                    runOnUiThread(new Runnable() {
                                        @Override public void run() { progDlg.setMessage(msg); }
                                    });
                                }
                            });
                    runOnUiThread(new Runnable() {
                        @Override public void run() {
                            progDlg.dismiss();
                            config.selectedVersion = ver.id;
                            config.clientJarPath = jar.getAbsolutePath();
                            config.save();
                            validateUsernameUi(config.username);
                            new AlertDialog.Builder(LauncherActivity.this, android.R.style.Theme_DeviceDefault_Dialog_Alert)
                                    .setTitle("İndirme Tamamlandı: " + ver.id)
                                    .setMessage("client.jar kaydedildi:\n" + jar.getAbsolutePath()
                                            + "\n\nNot: Android'de çalıştırmak için bu JAR'ı MONOLITH ile libminecraft_mono.so olarak derleyip '.so Motor Seç' üzerinden yükleyin:\n"
                                            + "python3 tools/mcbuild.py build-lib " + jar.getName() + " -dynamic --target aarch64-linux-android.26")
                                    .setPositiveButton("Tamam", null)
                                    .show();
                        }
                    });
                } catch (final Throwable t) {
                    runOnUiThread(new Runnable() {
                        @Override public void run() {
                            progDlg.dismiss();
                            Toast.makeText(LauncherActivity.this,
                                    "İndirme hatası: " + t.getMessage(), Toast.LENGTH_LONG).show();
                        }
                    });
                }
            }
        }).start();
    }

    private void launchSoPicker() {
        Intent intent = new Intent(Intent.ACTION_OPEN_DOCUMENT);
        intent.addCategory(Intent.CATEGORY_OPENABLE);
        intent.setType("*/*");
        startActivityForResult(intent, REQ_PICK_SO);
    }

    private EditText makePathField(LinearLayout parent, String label, String val) {
        TextView t = new TextView(this);
        t.setText(label);
        t.setTextColor(COL_MUTED);
        t.setTextSize(11f);
        t.setPadding(0, dp(6), 0, dp(2));
        parent.addView(t);

        EditText et = new EditText(this);
        et.setText(val);
        et.setTextColor(COL_TEXT);
        et.setTextSize(11.5f);
        et.setSingleLine(true);
        et.setBackground(roundedBox(COL_BG, COL_BORDER, 7));
        et.setPadding(dp(8), dp(7), dp(8), dp(7));
        parent.addView(et);
        return et;
    }

    private void refreshExtLibLabel() {
        if (extLibStatusText == null) return;
        if (config.extLibPath == null || config.extLibPath.isEmpty()) {
            extLibStatusText.setText("DEMO MODU aktif (libminecraft_mono.so seçilmedi).\n"
                    + "Üretmek için: python3 tools/mcbuild.py build-lib <client.jar> -dynamic --target aarch64-linux-android.26");
        } else {
            File f = new File(config.extLibPath);
            extLibStatusText.setText("Seçili: " + f.getAbsolutePath()
                    + (f.exists() ? (" (" + (f.length() / 1024) + " KB)") : " (BULUNAMADI — açılışta hata verecektir)"));
        }
    }

    private void buildSystemTab(LinearLayout page) {
        LinearLayout hwCard = makeCard("CİHAZ & ZİG RHI DONANIM TANILAMA");

        probeOutputText = new TextView(this);
        probeOutputText.setTextColor(COL_TEXT);
        probeOutputText.setTypeface(Typeface.MONOSPACE);
        probeOutputText.setTextSize(11.5f);
        probeOutputText.setText(NativeBridge.safeProbeHardware());
        hwCard.addView(probeOutputText);

        Button refreshBtn = makeStyledBtn("Donanımı & Logu Yenile", 0xFF1E3A8A, COL_BLUE);
        LinearLayout.LayoutParams rfLp = new LinearLayout.LayoutParams(
                ViewGroup.LayoutParams.MATCH_PARENT, dp(40));
        rfLp.topMargin = dp(8);
        refreshBtn.setLayoutParams(rfLp);
        refreshBtn.setOnClickListener(new View.OnClickListener() {
            @Override public void onClick(View v) {
                probeOutputText.setText(NativeBridge.safeProbeHardware());
                logOutputText.setText(NativeBridge.safeGetLastLog());
                crashOutputText.setText(config.readLastCrash());
            }
        });
        hwCard.addView(refreshBtn);
        page.addView(hwCard);

        LinearLayout logCard = makeCard("SON NATIVE MOTOR GÜNLÜĞÜ");
        logOutputText = new TextView(this);
        logOutputText.setTextColor(COL_MUTED);
        logOutputText.setTypeface(Typeface.MONOSPACE);
        logOutputText.setTextSize(11f);
        logOutputText.setText(NativeBridge.safeGetLastLog());
        logCard.addView(logOutputText);
        page.addView(logCard);

        LinearLayout crashCard = makeCard("SON JAVA ÇÖKME İZİ (CRASH LOG)");
        crashOutputText = new TextView(this);
        crashOutputText.setTextColor(0xFFFCA5A5);
        crashOutputText.setTypeface(Typeface.MONOSPACE);
        crashOutputText.setTextSize(10.5f);
        crashOutputText.setText(config.readLastCrash());
        crashCard.addView(crashOutputText);
        Button exportBtn = makeStyledBtn("Raporlari Disa Aktar (.zip)", 0xFF14532D, COL_ACCENT);
        LinearLayout.LayoutParams exLp = new LinearLayout.LayoutParams(ViewGroup.LayoutParams.MATCH_PARENT, dp(40));
        exLp.topMargin = dp(8);
        exportBtn.setLayoutParams(exLp);
        exportBtn.setOnClickListener(new View.OnClickListener() {
            @Override public void onClick(View v) { startReportExport(); }
        });
        crashCard.addView(exportBtn);
        Button clearLogBtn = makeStyledBtn("Logu Sifirla", 0xFF7F1D1D, 0xFFEF4444);
        LinearLayout.LayoutParams clLp = new LinearLayout.LayoutParams(ViewGroup.LayoutParams.MATCH_PARENT, dp(40));
        clLp.topMargin = dp(8);
        clearLogBtn.setLayoutParams(clLp);
        clearLogBtn.setOnClickListener(new View.OnClickListener() {
            @Override public void onClick(View v) {
                new AlertDialog.Builder(LauncherActivity.this, android.R.style.Theme_DeviceDefault_Dialog_Alert)
                        .setTitle("Log sifirlansin mi?")
                        .setMessage("Motor logu, olcumler, ekran goruntuleri ve cokme raporlari silinecek. Ayarlar ve dunyalar silinmez.")
                        .setPositiveButton("Sifirla", new DialogInterface.OnClickListener() {
                            @Override public void onClick(DialogInterface dialog, int which) {
                                int n = config.clearLogs();
                                crashOutputText.setText(config.readLastCrash());
                                Toast.makeText(LauncherActivity.this, n + " dosya silindi.", Toast.LENGTH_SHORT).show();
                            }
                        })
                        .setNegativeButton("Vazgec", null)
                        .show();
            }
        });
        crashCard.addView(clearLogBtn);
        page.addView(crashCard);

        Button factoryResetBtn = makeStyledBtn("Tüm Ayarları ve Profilleri Sıfırla", 0xFF7F1D1D, 0xFFEF4444);
        factoryResetBtn.setOnClickListener(new View.OnClickListener() {
            @Override public void onClick(View v) {
                new AlertDialog.Builder(LauncherActivity.this, android.R.style.Theme_DeviceDefault_Dialog_Alert)
                        .setTitle("Sıfırlama Onayı")
                        .setMessage("Tüm özel tuş profilleri ve başlatıcı ayarları varsayılana dönecek.")
                        .setPositiveButton("Sıfırla", new DialogInterface.OnClickListener() {
                            @Override public void onClick(DialogInterface dialog, int which) {
                                config.resetAll();
                                activeProfile = config.loadActiveProfile();
                                Toast.makeText(LauncherActivity.this, "Sıfırlandı.", Toast.LENGTH_SHORT).show();
                                selectTab(0);
                            }
                        })
                        .setNegativeButton("Vazgeç", null)
                        .show();
            }
        });
        page.addView(factoryResetBtn);
    }

    private void promptCloneProfile() {
        final EditText input = new EditText(this);
        input.setHint("Yeni profil adı");
        input.setText(config.activeProfileName + " Kopya");
        new AlertDialog.Builder(this, android.R.style.Theme_DeviceDefault_Dialog_Alert)
                .setTitle("Yeni Kontrol Profili")
                .setView(input)
                .setPositiveButton("Oluştur", new DialogInterface.OnClickListener() {
                    @Override public void onClick(DialogInterface dialog, int which) {
                        String name = input.getText().toString().trim();
                        if (name.isEmpty()) return;
                        ControlProfile cloned = activeProfile.copy(name);
                        config.saveProfile(cloned);
                        config.activeProfileName = name;
                        config.save();
                        activeProfile = cloned;
                        selectTab(1);
                    }
                })
                .setNegativeButton("İptal", null)
                .show();
    }

    private void exportProfileJson() {
        try {
            final String json = activeProfile.toPrettyJson();
            ClipboardManager cm = (ClipboardManager) getSystemService(Context.CLIPBOARD_SERVICE);
            if (cm != null) {
                cm.setPrimaryClip(ClipData.newPlainText("TouchCore Profile", json));
            }
            new AlertDialog.Builder(this, android.R.style.Theme_DeviceDefault_Dialog_Alert)
                    .setTitle("Profil JSON (Panoya Kopyalandı)")
                    .setMessage(json)
                    .setPositiveButton("Tamam", null)
                    .show();
        } catch (Exception e) {
            Toast.makeText(this, "JSON oluşturulamadı: " + e.getMessage(), Toast.LENGTH_LONG).show();
        }
    }

    private void importProfileJson() {
        final EditText input = new EditText(this);
        input.setHint("TouchCore Profil JSON yapıştırın...");
        input.setMinLines(4);
        new AlertDialog.Builder(this, android.R.style.Theme_DeviceDefault_Dialog_Alert)
                .setTitle("JSON Profil İçe Aktar")
                .setView(input)
                .setPositiveButton("İçe Aktar", new DialogInterface.OnClickListener() {
                    @Override public void onClick(DialogInterface dialog, int which) {
                        try {
                            ControlProfile imported = ControlProfile.fromJsonString(input.getText().toString().trim());
                            config.saveProfile(imported);
                            config.activeProfileName = imported.name;
                            config.save();
                            activeProfile = imported;
                            Toast.makeText(LauncherActivity.this,
                                    "İçe aktarıldı: " + imported.name, Toast.LENGTH_SHORT).show();
                            selectTab(1);
                        } catch (Exception e) {
                            Toast.makeText(LauncherActivity.this,
                                    "Geçersiz JSON: " + e.getMessage(), Toast.LENGTH_LONG).show();
                        }
                    }
                })
                .setNegativeButton("İptal", null)
                .show();
    }

    @Override
    protected void onActivityResult(int requestCode, int resultCode, Intent data) {
        super.onActivityResult(requestCode, resultCode, data);
        if (requestCode == REQ_EXPORT_REPORTS && resultCode == RESULT_OK && data != null && data.getData() != null) {
            writeReportZip(data.getData());
            return;
        }
        if (requestCode == REQ_PICK_SO && resultCode == RESULT_OK && data != null && data.getData() != null) {
            Uri uri = data.getData();
            File dst = config.getNativeImportTarget();
            try {
                InputStream in = getContentResolver().openInputStream(uri);
                if (in == null) throw new IllegalStateException("Dosya okunamadı");
                FileOutputStream out = new FileOutputStream(dst);
                byte[] buf = new byte[16384];
                int n;
                while ((n = in.read(buf)) > 0) {
                    out.write(buf, 0, n);
                }
                out.close();
                in.close();
                dst.setReadable(true, false);
                dst.setExecutable(true, false);
                config.extLibPath = dst.getAbsolutePath();
                config.save();
                refreshExtLibLabel();
                validateUsernameUi(config.username);
                Toast.makeText(this, "libminecraft_mono.so içe aktarıldı.", Toast.LENGTH_SHORT).show();
            } catch (Exception e) {
                Toast.makeText(this, "Dosya kopyalama hatası: " + e.getMessage(), Toast.LENGTH_LONG).show();
            }
        }
    }
}
