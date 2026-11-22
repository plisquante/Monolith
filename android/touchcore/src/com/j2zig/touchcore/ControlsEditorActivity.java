package com.j2zig.touchcore;

import android.app.Activity;
import android.app.AlertDialog;
import android.content.DialogInterface;
import android.graphics.Canvas;
import android.graphics.Paint;
import android.graphics.RectF;
import android.graphics.Typeface;
import android.graphics.drawable.GradientDrawable;
import android.os.Build;
import android.os.Bundle;
import android.view.Gravity;
import android.view.MotionEvent;
import android.view.View;
import android.view.ViewGroup;
import android.view.WindowInsets;
import android.view.WindowInsetsController;
import android.widget.Button;
import android.widget.EditText;
import android.widget.FrameLayout;
import android.widget.HorizontalScrollView;
import android.widget.LinearLayout;
import android.widget.SeekBar;
import android.widget.TextView;
import android.widget.Toast;

public final class ControlsEditorActivity extends Activity {
    private LauncherConfig config;
    private ControlProfile workingProfile;
    private String selectedButtonId = null;
    private boolean selectedJoystick = false;
    private boolean gridSnap = true;
    private boolean inspectorExpanded = true;
    private EditorCanvasView canvasView;
    private LinearLayout inspectorBody;

    private TextView selectionTitle;
    private Button toggleModeBtn;
    private Button swipeBtn;
    private Button shapeBtn;
    private Button lockBtn;
    private Button collapseBtn;
    private SeekBar widthSeek;
    private SeekBar heightSeek;
    private SeekBar opacitySeek;

    private static final int[] COLOR_PALETTE = {
        0xFFFFFFFF, 0xFF3DDC84, 0xFFFF8A8A, 0xFF7DD3FC,
        0xFFFDE047, 0xFFC084FC, 0xFFF472B6, 0xFF94A3B8
    };

    @Override
    protected void onCreate(Bundle savedInstanceState) {
        super.onCreate(savedInstanceState);
        LauncherConfig.installCrashHandler(this);
        config = new LauncherConfig(this);
        String targetProfile = getIntent().getStringExtra("profile_name");
        if (targetProfile == null || targetProfile.isEmpty()) {
            targetProfile = config.activeProfileName;
        }
        workingProfile = config.loadProfile(targetProfile).copy(targetProfile);
        if (!workingProfile.buttons.isEmpty()) {
            selectedButtonId = workingProfile.buttons.get(0).id;
        }

        FrameLayout root = new FrameLayout(this);
        root.setBackgroundColor(0xFF0B1117);

        canvasView = new EditorCanvasView(this);
        root.addView(canvasView, new FrameLayout.LayoutParams(
                ViewGroup.LayoutParams.MATCH_PARENT, ViewGroup.LayoutParams.MATCH_PARENT));

        root.addView(buildTopToolbar(), new FrameLayout.LayoutParams(
                ViewGroup.LayoutParams.MATCH_PARENT, ViewGroup.LayoutParams.WRAP_CONTENT, Gravity.TOP));
        root.addView(buildBottomInspector(), new FrameLayout.LayoutParams(
                ViewGroup.LayoutParams.MATCH_PARENT, ViewGroup.LayoutParams.WRAP_CONTENT, Gravity.BOTTOM));

        setContentView(root);
        refreshInspector();
    }

    @Override
    public void onWindowFocusChanged(boolean hasFocus) {
        super.onWindowFocusChanged(hasFocus);
        if (hasFocus) enterFullscreenSafe();
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
                        View.SYSTEM_UI_FLAG_FULLSCREEN
                                | View.SYSTEM_UI_FLAG_HIDE_NAVIGATION
                                | View.SYSTEM_UI_FLAG_IMMERSIVE_STICKY);
            }
        } catch (Throwable ignored) {}
    }

    private int dp(int v) {
        return Math.round(v * getResources().getDisplayMetrics().density);
    }

    private GradientDrawable pillBg(int fill, int stroke, int radiusDp) {
        GradientDrawable d = new GradientDrawable();
        d.setColor(fill);
        d.setStroke(dp(1), stroke);
        d.setCornerRadius(dp(radiusDp));
        return d;
    }

    private Button makePillBtn(String text, int bgColor, int strokeColor) {
        Button b = new Button(this);
        b.setText(text);
        b.setTextSize(11f);
        b.setTypeface(Typeface.DEFAULT_BOLD);
        b.setTextColor(0xFFF8FAFC);
        b.setAllCaps(false);
        b.setBackground(pillBg(bgColor, strokeColor, 8));
        b.setPadding(dp(10), dp(2), dp(10), dp(2));
        LinearLayout.LayoutParams lp = new LinearLayout.LayoutParams(
                ViewGroup.LayoutParams.WRAP_CONTENT, dp(32));
        lp.setMargins(dp(3), 0, dp(3), 0);
        b.setLayoutParams(lp);
        return b;
    }

    private View buildTopToolbar() {
        HorizontalScrollView scroll = new HorizontalScrollView(this);
        scroll.setBackgroundColor(0xBB0B1117);
        LinearLayout bar = new LinearLayout(this);
        bar.setOrientation(LinearLayout.HORIZONTAL);
        bar.setPadding(dp(8), dp(5), dp(8), dp(5));

        Button backBtn = makePillBtn("← Geri", 0xFF1E293B, 0xFF475569);
        backBtn.setOnClickListener(new View.OnClickListener() {
            @Override public void onClick(View v) { finish(); }
        });

        Button addBtn = makePillBtn("+ Tuş Ekle", 0xFF1E3A8A, 0xFF38BDF8);
        addBtn.setOnClickListener(new View.OnClickListener() {
            @Override public void onClick(View v) { showAddButtonDialog(); }
        });

        Button stickBtn = makePillBtn("D-Pad / Analog", 0xFF115E59, 0xFF2DD4BF);
        stickBtn.setOnClickListener(new View.OnClickListener() {
            @Override public void onClick(View v) {
                selectedButtonId = null;
                selectedJoystick = true;
                workingProfile.joystickMode = (workingProfile.joystickMode == ControlProfile.STICK_ANALOG)
                        ? ControlProfile.STICK_DPAD : ControlProfile.STICK_ANALOG;
                canvasView.invalidate();
                refreshInspector();
            }
        });

        final Button snapBtn = makePillBtn("Izgara: AÇIK", 0xFF1E293B, 0xFF475569);
        snapBtn.setOnClickListener(new View.OnClickListener() {
            @Override public void onClick(View v) {
                gridSnap = !gridSnap;
                snapBtn.setText(gridSnap ? "Izgara: AÇIK" : "Izgara: KAPALI");
                canvasView.invalidate();
            }
        });

        Button resetBtn = makePillBtn("Sıfırla", 0xFF450A0A, 0xFFEF4444);
        resetBtn.setOnClickListener(new View.OnClickListener() {
            @Override public void onClick(View v) {
                workingProfile = ControlProfile.createPreset(workingProfile.name);
                selectedJoystick = false;
                selectedButtonId = workingProfile.buttons.isEmpty() ? null : workingProfile.buttons.get(0).id;
                canvasView.invalidate();
                refreshInspector();
            }
        });

        Button saveBtn = makePillBtn("✓ KAYDET", 0xFF15803D, 0xFF3DDC84);
        saveBtn.setOnClickListener(new View.OnClickListener() {
            @Override public void onClick(View v) {
                config.saveProfile(workingProfile);
                config.activeProfileName = workingProfile.name;
                config.save();
                Toast.makeText(ControlsEditorActivity.this,
                        "MJLauncher HUD kaydedildi: " + workingProfile.name, Toast.LENGTH_SHORT).show();
                finish();
            }
        });

        bar.addView(backBtn);
        bar.addView(addBtn);
        bar.addView(stickBtn);
        bar.addView(snapBtn);
        bar.addView(resetBtn);
        bar.addView(saveBtn);
        scroll.addView(bar);
        return scroll;
    }

    private View buildBottomInspector() {
        LinearLayout outer = new LinearLayout(this);
        outer.setOrientation(LinearLayout.VERTICAL);
        outer.setBackgroundColor(0xD80D131D);
        outer.setPadding(dp(10), dp(5), dp(10), dp(6));

        LinearLayout header = new LinearLayout(this);
        header.setOrientation(LinearLayout.HORIZONTAL);
        header.setGravity(Gravity.CENTER_VERTICAL);

        selectionTitle = new TextView(this);
        selectionTitle.setTextColor(0xFF3DDC84);
        selectionTitle.setTypeface(Typeface.DEFAULT_BOLD);
        selectionTitle.setTextSize(12f);
        selectionTitle.setLayoutParams(new LinearLayout.LayoutParams(
                0, ViewGroup.LayoutParams.WRAP_CONTENT, 1f));

        header.addView(selectionTitle);
        header.addView(makeNudgeBtn("◀", -0.005f, 0f));
        header.addView(makeNudgeBtn("▲", 0f, -0.005f));
        header.addView(makeNudgeBtn("▼", 0f, 0.005f));
        header.addView(makeNudgeBtn("▶", 0.005f, 0f));

        collapseBtn = makePillBtn("Gizle ▾", 0xFF1E293B, 0xFF475569);
        collapseBtn.setOnClickListener(new View.OnClickListener() {
            @Override public void onClick(View v) {
                inspectorExpanded = !inspectorExpanded;
                inspectorBody.setVisibility(inspectorExpanded ? View.VISIBLE : View.GONE);
                collapseBtn.setText(inspectorExpanded ? "Gizle ▾" : "Düzenle ▴");
            }
        });
        header.addView(collapseBtn);
        outer.addView(header);

        inspectorBody = new LinearLayout(this);
        inspectorBody.setOrientation(LinearLayout.VERTICAL);
        inspectorBody.setPadding(0, dp(4), 0, 0);

        HorizontalScrollView actionScroll = new HorizontalScrollView(this);
        LinearLayout row1 = new LinearLayout(this);
        row1.setOrientation(LinearLayout.HORIZONTAL);
        row1.setGravity(Gravity.CENTER_VERTICAL);

        Button bindBtn = makePillBtn("Tuş/İkon", 0xFF1E3A8A, 0xFF38BDF8);
        bindBtn.setOnClickListener(new View.OnClickListener() {
            @Override public void onClick(View v) { editSelectedButtonBinding(); }
        });

        shapeBtn = makePillBtn("Şekil", 0xFF1E293B, 0xFF475569);
        shapeBtn.setOnClickListener(new View.OnClickListener() {
            @Override public void onClick(View v) {
                ControlButtonSpec b = getSelectedButton();
                if (b != null) {
                    b.shape = (b.shape + 1) % 4;
                    canvasView.invalidate();
                    refreshInspector();
                }
            }
        });

        toggleModeBtn = makePillBtn("Basılı Tut", 0xFF1E293B, 0xFF475569);
        toggleModeBtn.setOnClickListener(new View.OnClickListener() {
            @Override public void onClick(View v) {
                ControlButtonSpec b = getSelectedButton();
                if (b != null) {
                    b.toggleMode = !b.toggleMode;
                    canvasView.invalidate();
                    refreshInspector();
                } else if (selectedJoystick) {
                    workingProfile.dynamicJoystick = !workingProfile.dynamicJoystick;
                    refreshInspector();
                }
            }
        });

        swipeBtn = makePillBtn("Swipe Bakış", 0xFF1E293B, 0xFF475569);
        swipeBtn.setOnClickListener(new View.OnClickListener() {
            @Override public void onClick(View v) {
                ControlButtonSpec b = getSelectedButton();
                if (b != null) {
                    b.swipeable = !b.swipeable;
                    canvasView.invalidate();
                    refreshInspector();
                }
            }
        });

        lockBtn = makePillBtn("Serbest", 0xFF1E293B, 0xFF475569);
        lockBtn.setOnClickListener(new View.OnClickListener() {
            @Override public void onClick(View v) {
                ControlButtonSpec b = getSelectedButton();
                if (b != null) {
                    b.locked = !b.locked;
                    canvasView.invalidate();
                    refreshInspector();
                }
            }
        });

        Button cloneBtn = makePillBtn("Çoğalt", 0xFF1E293B, 0xFF475569);
        cloneBtn.setOnClickListener(new View.OnClickListener() {
            @Override public void onClick(View v) {
                ControlButtonSpec b = getSelectedButton();
                if (b != null) {
                    ControlButtonSpec dup = b.copy();
                    dup.id = "dup_" + System.currentTimeMillis();
                    dup.xPct = ControlButtonSpec.clamp(b.xPct - 0.06f, 0.05f, 0.95f);
                    dup.yPct = ControlButtonSpec.clamp(b.yPct - 0.06f, 0.08f, 0.88f);
                    dup.locked = false;
                    workingProfile.buttons.add(dup);
                    selectedButtonId = dup.id;
                    canvasView.invalidate();
                    refreshInspector();
                }
            }
        });

        Button delBtn = makePillBtn("Sil", 0xFF7F1D1D, 0xFFEF4444);
        delBtn.setOnClickListener(new View.OnClickListener() {
            @Override public void onClick(View v) {
                ControlButtonSpec b = getSelectedButton();
                if (b != null) {
                    workingProfile.buttons.remove(b);
                    selectedButtonId = workingProfile.buttons.isEmpty() ? null : workingProfile.buttons.get(0).id;
                    canvasView.invalidate();
                    refreshInspector();
                }
            }
        });

        row1.addView(bindBtn);
        row1.addView(shapeBtn);
        row1.addView(toggleModeBtn);
        row1.addView(swipeBtn);
        row1.addView(lockBtn);
        row1.addView(cloneBtn);
        row1.addView(delBtn);

        for (final int swatchColor : COLOR_PALETTE) {
            View dot = new View(this);
            dot.setBackground(pillBg(swatchColor, 0xFF0F172A, 12));
            LinearLayout.LayoutParams dlp = new LinearLayout.LayoutParams(dp(22), dp(22));
            dlp.setMargins(dp(3), 0, dp(3), 0);
            dot.setLayoutParams(dlp);
            dot.setOnClickListener(new View.OnClickListener() {
                @Override public void onClick(View v) {
                    ControlButtonSpec b = getSelectedButton();
                    if (b != null) {
                        b.accentColor = swatchColor;
                        canvasView.invalidate();
                    }
                }
            });
            row1.addView(dot);
        }

        actionScroll.addView(row1);
        inspectorBody.addView(actionScroll);

        LinearLayout row2 = new LinearLayout(this);
        row2.setOrientation(LinearLayout.HORIZONTAL);
        row2.setGravity(Gravity.CENTER_VERTICAL);
        row2.setPadding(0, dp(4), 0, 0);

        widthSeek = addCompactSlider(row2, "En");
        widthSeek.setMax(100);
        widthSeek.setOnSeekBarChangeListener(new SeekBar.OnSeekBarChangeListener() {
            @Override public void onProgressChanged(SeekBar s, int progress, boolean fromUser) {
                if (!fromUser) return;
                if (selectedJoystick) {
                    workingProfile.joystickSizeDp = 86f + progress * 1.25f;
                } else {
                    ControlButtonSpec b = getSelectedButton();
                    if (b != null) b.widthDp = 28f + progress;
                }
                canvasView.invalidate();
            }
            @Override public void onStartTrackingTouch(SeekBar s) {}
            @Override public void onStopTrackingTouch(SeekBar s) {}
        });

        heightSeek = addCompactSlider(row2, "Boy");
        heightSeek.setMax(100);
        heightSeek.setOnSeekBarChangeListener(new SeekBar.OnSeekBarChangeListener() {
            @Override public void onProgressChanged(SeekBar s, int progress, boolean fromUser) {
                if (!fromUser) return;
                if (selectedJoystick) {
                    workingProfile.joystickSizeDp = 86f + progress * 1.25f;
                } else {
                    ControlButtonSpec b = getSelectedButton();
                    if (b != null) b.heightDp = 28f + progress;
                }
                canvasView.invalidate();
            }
            @Override public void onStartTrackingTouch(SeekBar s) {}
            @Override public void onStopTrackingTouch(SeekBar s) {}
        });

        opacitySeek = addCompactSlider(row2, "Şeffaf");
        opacitySeek.setMax(85);
        opacitySeek.setOnSeekBarChangeListener(new SeekBar.OnSeekBarChangeListener() {
            @Override public void onProgressChanged(SeekBar s, int progress, boolean fromUser) {
                if (!fromUser) return;
                float op = (15 + progress) / 100f;
                if (selectedJoystick) {
                    workingProfile.joystickOpacity = op;
                } else {
                    ControlButtonSpec b = getSelectedButton();
                    if (b != null) b.opacity = op;
                }
                canvasView.invalidate();
            }
            @Override public void onStartTrackingTouch(SeekBar s) {}
            @Override public void onStopTrackingTouch(SeekBar s) {}
        });

        inspectorBody.addView(row2);
        outer.addView(inspectorBody);
        return outer;
    }

    private Button makeNudgeBtn(String arrow, final float dx, final float dy) {
        Button b = makePillBtn(arrow, 0xFF1E293B, 0xFF334155);
        LinearLayout.LayoutParams lp = new LinearLayout.LayoutParams(dp(30), dp(28));
        lp.setMargins(dp(2), 0, dp(2), 0);
        b.setLayoutParams(lp);
        b.setPadding(0, 0, 0, 0);
        b.setOnClickListener(new View.OnClickListener() {
            @Override public void onClick(View v) {
                if (selectedJoystick) {
                    workingProfile.joystickXPct = ControlButtonSpec.clamp(workingProfile.joystickXPct + dx, 0.06f, 0.45f);
                    workingProfile.joystickYPct = ControlButtonSpec.clamp(workingProfile.joystickYPct + dy, 0.18f, 0.86f);
                } else {
                    ControlButtonSpec btn = getSelectedButton();
                    if (btn != null && !btn.locked) {
                        btn.xPct = ControlButtonSpec.clamp(btn.xPct + dx, 0.04f, 0.96f);
                        btn.yPct = ControlButtonSpec.clamp(btn.yPct + dy, 0.06f, 0.90f);
                    }
                }
                canvasView.invalidate();
            }
        });
        return b;
    }

    private SeekBar addCompactSlider(LinearLayout row, String label) {
        TextView tv = new TextView(this);
        tv.setText(label);
        tv.setTextColor(0xFFCBD5E1);
        tv.setTextSize(10.5f);
        tv.setPadding(dp(6), 0, dp(2), 0);
        row.addView(tv);

        SeekBar s = new SeekBar(this);
        s.setLayoutParams(new LinearLayout.LayoutParams(0, ViewGroup.LayoutParams.WRAP_CONTENT, 1f));
        row.addView(s);
        return s;
    }

    private ControlButtonSpec getSelectedButton() {
        if (selectedButtonId == null) return null;
        for (ControlButtonSpec b : workingProfile.buttons) {
            if (b.id.equals(selectedButtonId)) return b;
        }
        return null;
    }

    private void refreshInspector() {
        if (selectedJoystick) {
            selectionTitle.setText("Seçili: "
                    + (workingProfile.joystickMode == ControlProfile.STICK_DPAD ? "Çapraz (+) D-Pad" : "Analog Stick"));
            toggleModeBtn.setText(workingProfile.dynamicJoystick ? "Dinamik Stick" : "Sabit Stick");
            swipeBtn.setEnabled(false);
            shapeBtn.setEnabled(false);
            lockBtn.setEnabled(false);
            int p = Math.max(0, Math.min(100, Math.round((workingProfile.joystickSizeDp - 86f) / 1.25f)));
            widthSeek.setProgress(p);
            heightSeek.setProgress(p);
            opacitySeek.setProgress(Math.max(0, Math.min(85, Math.round(workingProfile.joystickOpacity * 100f - 15f))));
            return;
        }
        ControlButtonSpec b = getSelectedButton();
        shapeBtn.setEnabled(b != null);
        lockBtn.setEnabled(b != null);
        swipeBtn.setEnabled(b != null);
        if (b == null) {
            selectionTitle.setText("Düzenlemek için bir tuş veya D-Pad seçin");
            return;
        }
        selectionTitle.setText(b.label + " • " + GlfwKeys.describeAction(b.actionKind, b.keyCode));
        toggleModeBtn.setText(b.toggleMode ? "Mod: Aç/Kapa" : "Mod: Bas-Tut");
        swipeBtn.setText(b.swipeable ? "Swipe: AÇIK" : "Swipe: KAPALI");
        lockBtn.setText(b.locked ? "Kilitli" : "Serbest");
        String[] shapes = {"Kare", "Daire", "Hap", "Piksel"};
        shapeBtn.setText(shapes[b.shape % 4]);
        widthSeek.setProgress(Math.max(0, Math.min(100, Math.round(b.widthDp - 28f))));
        heightSeek.setProgress(Math.max(0, Math.min(100, Math.round(b.heightDp - 28f))));
        opacitySeek.setProgress(Math.max(0, Math.min(85, Math.round(b.opacity * 100f - 15f))));
    }

    private void showAddButtonDialog() {
        final String[] titles = new String[GlfwKeys.PRESET_ACTIONS.length];
        for (int i = 0; i < GlfwKeys.PRESET_ACTIONS.length; i++) {
            titles[i] = GlfwKeys.PRESET_ACTIONS[i].title;
        }
        new AlertDialog.Builder(this, android.R.style.Theme_DeviceDefault_Dialog_Alert)
                .setTitle("MJLauncher Tuşu Ekle")
                .setItems(titles, new DialogInterface.OnClickListener() {
                    @Override
                    public void onClick(DialogInterface dialog, int which) {
                        GlfwKeys.ActionChoice c = GlfwKeys.PRESET_ACTIONS[which];
                        String newId = "custom_" + System.currentTimeMillis();
                        ControlButtonSpec added = new ControlButtonSpec(
                                newId, c.defaultLabel, c.actionKind, c.code, c.iconKind,
                                0.50f, 0.45f, 54f, 54f, 0.78f,
                                ControlButtonSpec.SHAPE_ROUNDED, false, c.defaultSwipeable,
                                0xFFFFFFFF, false, true);
                        workingProfile.buttons.add(added);
                        selectedJoystick = false;
                        selectedButtonId = newId;
                        canvasView.invalidate();
                        refreshInspector();
                    }
                })
                .show();
    }

    private void editSelectedButtonBinding() {
        final ControlButtonSpec b = getSelectedButton();
        if (b == null) return;
        LinearLayout box = new LinearLayout(this);
        box.setOrientation(LinearLayout.VERTICAL);
        box.setPadding(dp(16), dp(10), dp(16), dp(4));

        final EditText labelInput = new EditText(this);
        labelInput.setText(b.label);
        labelInput.setHint("Tuş etiketi");
        box.addView(labelInput);

        final int[] chosenKind = {b.actionKind};
        final int[] chosenCode = {b.keyCode};
        final int[] chosenIcon = {b.iconKind};
        final Button pickKeyBtn = new Button(this);
        pickKeyBtn.setText("Tuş: " + GlfwKeys.describeAction(b.actionKind, b.keyCode));
        pickKeyBtn.setOnClickListener(new View.OnClickListener() {
            @Override public void onClick(View v) {
                String[] titles = new String[GlfwKeys.PRESET_ACTIONS.length];
                for (int i = 0; i < GlfwKeys.PRESET_ACTIONS.length; i++) {
                    titles[i] = GlfwKeys.PRESET_ACTIONS[i].title;
                }
                new AlertDialog.Builder(ControlsEditorActivity.this, android.R.style.Theme_DeviceDefault_Dialog_Alert)
                        .setTitle("Görev Seç")
                        .setItems(titles, new DialogInterface.OnClickListener() {
                            @Override public void onClick(DialogInterface d, int which) {
                                GlfwKeys.ActionChoice c = GlfwKeys.PRESET_ACTIONS[which];
                                chosenKind[0] = c.actionKind;
                                chosenCode[0] = c.code;
                                chosenIcon[0] = c.iconKind;
                                pickKeyBtn.setText("Tuş: " + c.title);
                                labelInput.setText(c.defaultLabel);
                            }
                        })
                        .show();
            }
        });
        box.addView(pickKeyBtn);

        final String[] iconNames = {
            "Yalnızca Metin", "Kılıç / Vur", "Blok / Koy", "Zıpla Oku", "Eğil İkonu",
            "Koş Çift Ok", "Envanter 3x3", "At İkonu", "Sohbet Balonu", "Duraklat ||",
            "Fare İmleci", "Klavye", "Kamera", "El Değiştir", "Blok Seç", "Gizle Göz", "Menü"
        };
        final Button pickIconBtn = new Button(this);
        pickIconBtn.setText("İkon: " + iconNames[Math.max(0, Math.min(iconNames.length - 1, b.iconKind))]);
        pickIconBtn.setOnClickListener(new View.OnClickListener() {
            @Override public void onClick(View v) {
                new AlertDialog.Builder(ControlsEditorActivity.this, android.R.style.Theme_DeviceDefault_Dialog_Alert)
                        .setTitle("Buton İkonu Seç")
                        .setItems(iconNames, new DialogInterface.OnClickListener() {
                            @Override public void onClick(DialogInterface d, int which) {
                                chosenIcon[0] = which;
                                pickIconBtn.setText("İkon: " + iconNames[which]);
                            }
                        })
                        .show();
            }
        });
        box.addView(pickIconBtn);

        new AlertDialog.Builder(this, android.R.style.Theme_DeviceDefault_Dialog_Alert)
                .setTitle("Tuş & İkon Ayarı")
                .setView(box)
                .setPositiveButton("Uygula", new DialogInterface.OnClickListener() {
                    @Override public void onClick(DialogInterface dialog, int which) {
                        b.label = labelInput.getText().toString().trim();
                        b.actionKind = chosenKind[0];
                        b.keyCode = chosenCode[0];
                        b.iconKind = chosenIcon[0];
                        canvasView.invalidate();
                        refreshInspector();
                    }
                })
                .setNegativeButton("İptal", null)
                .show();
    }

    private final class EditorCanvasView extends View {
        private final Paint paint = new Paint(Paint.ANTI_ALIAS_FLAG);
        private final RectF tmpRect = new RectF();
        private float dragOffsetX;
        private float dragOffsetY;
        private boolean dragging = false;

        EditorCanvasView(ControlsEditorActivity ctx) {
            super(ctx);
        }

        @Override
        protected void onDraw(Canvas canvas) {
            super.onDraw(canvas);
            int w = getWidth();
            int h = getHeight();
            if (w <= 0 || h <= 0) return;
            float density = getResources().getDisplayMetrics().density;

            HudRenderer.drawEditorWorldBackdrop(canvas, w, h, density, paint);

            if (gridSnap) {
                paint.setStyle(Paint.Style.STROKE);
                paint.setStrokeWidth(1f);
                paint.setColor(0x28FFFFFF);
                for (int i = 1; i < 20; i++) {
                    float gx = w * (i / 20f);
                    canvas.drawLine(gx, 0, gx, h, paint);
                }
                for (int j = 1; j < 12; j++) {
                    float gy = h * (j / 12f);
                    canvas.drawLine(0, gy, w, gy, paint);
                }
            }

            float jx = workingProfile.joystickXPct * w;
            float jy = workingProfile.joystickYPct * h;
            HudRenderer.drawMovementPad(
                    canvas, workingProfile, jx, jy, jx, jy, 0f, 0f,
                    false, selectedJoystick, density, paint);

            for (ControlButtonSpec b : workingProfile.buttons) {
                HudRenderer.computeButtonRect(b, w, h, density, tmpRect);
                boolean sel = !selectedJoystick && b.id.equals(selectedButtonId);
                HudRenderer.drawButton(canvas, tmpRect, b, false, sel, true, density, paint);
            }
        }

        @Override
        public boolean onTouchEvent(MotionEvent event) {
            int w = getWidth();
            int h = getHeight();
            float density = getResources().getDisplayMetrics().density;
            float x = event.getX();
            float y = event.getY();

            switch (event.getActionMasked()) {
                case MotionEvent.ACTION_DOWN:
                    dragging = false;
                    for (int i = workingProfile.buttons.size() - 1; i >= 0; i--) {
                        ControlButtonSpec b = workingProfile.buttons.get(i);
                        HudRenderer.computeButtonRect(b, w, h, density, tmpRect);
                        if (tmpRect.contains(x, y)) {
                            selectedJoystick = false;
                            selectedButtonId = b.id;
                            dragOffsetX = x - (b.xPct * w);
                            dragOffsetY = y - (b.yPct * h);
                            dragging = !b.locked;
                            invalidate();
                            refreshInspector();
                            return true;
                        }
                    }
                    float jx = workingProfile.joystickXPct * w;
                    float jy = workingProfile.joystickYPct * h;
                    float jr = (workingProfile.joystickSizeDp * 0.55f) * density;
                    if (Math.hypot(x - jx, y - jy) <= jr) {
                        selectedJoystick = true;
                        selectedButtonId = null;
                        dragOffsetX = x - jx;
                        dragOffsetY = y - jy;
                        dragging = true;
                        invalidate();
                        refreshInspector();
                        return true;
                    }
                    return true;

                case MotionEvent.ACTION_MOVE:
                    if (!dragging) return true;
                    float nx = (x - dragOffsetX) / Math.max(1f, w);
                    float ny = (y - dragOffsetY) / Math.max(1f, h);
                    if (gridSnap) {
                        nx = Math.round(nx * 40f) / 40f;
                        ny = Math.round(ny * 24f) / 24f;
                    }
                    if (selectedJoystick) {
                        workingProfile.joystickXPct = ControlButtonSpec.clamp(nx, 0.06f, 0.45f);
                        workingProfile.joystickYPct = ControlButtonSpec.clamp(ny, 0.18f, 0.86f);
                    } else {
                        ControlButtonSpec b = getSelectedButton();
                        if (b != null && !b.locked) {
                            b.xPct = ControlButtonSpec.clamp(nx, 0.04f, 0.96f);
                            b.yPct = ControlButtonSpec.clamp(ny, 0.06f, 0.90f);
                        }
                    }
                    invalidate();
                    return true;

                case MotionEvent.ACTION_UP:
                    dragging = false;
                    return true;
            }
            return super.onTouchEvent(event);
        }
    }
}
