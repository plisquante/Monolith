package com.monolith.touchcore;

import android.graphics.Canvas;
import android.graphics.Color;
import android.graphics.LinearGradient;
import android.graphics.Paint;
import android.graphics.Path;
import android.graphics.RectF;
import android.graphics.Shader;
import android.graphics.Typeface;

public final class HudRenderer {
    private static final Path TMP_PATH = new Path();
    private static final RectF TMP_RECT = new RectF();

    private static final String[] HOTBAR_NAMES = {
        "Çim Bloğu", "Toprak", "Taş", "Meşe Tahtası", "Meşe Kütüğü",
        "Yaprak", "Kırıktaş", "Altın Bloğu", "Işıktaşı"
    };
    private static final int[] HOTBAR_TOP_COLORS = {
        0xFF59B337, 0xFF8B5A2B, 0xFF94A3B8, 0xFFC89B54, 0xFF7D5333,
        0xFF2D8A4E, 0xFF788290, 0xFFFACC15, 0xFFFDE047
    };
    private static final int[] HOTBAR_SIDE_COLORS = {
        0xFF785134, 0xFF6B4220, 0xFF6E7A8A, 0xFFA47A3C, 0xFF563720,
        0xFF1F6337, 0xFF555E6B, 0xFFCA8A04, 0xFFEAB308
    };

    private HudRenderer() {}

    public static float dp(float v, float density) {
        return v * density;
    }

    public static void computeButtonRect(
            ControlButtonSpec b, int viewW, int viewH, float density, RectF out) {
        float w = b.widthDp * density;
        float h = b.heightDp * density;
        float cx = b.xPct * viewW;
        float cy = b.yPct * viewH;
        out.set(cx - w * 0.5f, cy - h * 0.5f, cx + w * 0.5f, cy + h * 0.5f);
    }

    /** Vanilla auto GUI scale: largest integer scale that keeps a 320x240 logical screen. */
    public static int guiScale(int viewW, int viewH) {
        return Math.max(1, Math.min(viewW / 320, viewH / 240));
    }

    /** Bounds of the hotbar the game itself draws (182x22 GUI px, bottom centered). */
    public static RectF getHotbarRect(int viewW, int viewH, float density) {
        float s = guiScale(viewW, viewH);
        float totalW = 182f * s;
        float barH = 22f * s;
        float left = (viewW - totalW) * 0.5f;
        return new RectF(left, viewH - barH, left + totalW, viewH);
    }

    /** Hotbar slot (0..8) under x, or -1 when x is outside the slot row. */
    public static int hotbarSlotAt(int viewW, int viewH, float density, float x) {
        RectF bar = getHotbarRect(viewW, viewH, density);
        float s = guiScale(viewW, viewH);
        float rel = (x - bar.left - s) / (20f * s);
        if (rel < 0f || rel >= 9f) return -1;
        return (int) rel;
    }

    public static void drawButton(
            Canvas canvas,
            RectF rect,
            ControlButtonSpec b,
            boolean active,
            boolean selectedInEditor,
            boolean showEditorBadges,
            float density,
            Paint paint) {
        if (b.shape == ControlButtonSpec.SHAPE_PIXEL) {
            drawPixelButton(canvas, rect, b, active, selectedInEditor, showEditorBadges, density, paint);
            return;
        }
        float corner = (b.shape == ControlButtonSpec.SHAPE_CIRCLE)
                ? Math.min(rect.width(), rect.height()) * 0.5f
                : (b.shape == ControlButtonSpec.SHAPE_PILL)
                        ? Math.min(rect.width(), rect.height()) * 0.5f
                        : dp(10f, density);

        int tint = b.accentColor;
        int alphaNorm = Math.round(b.opacity * 255f);

        // Soft drop shadow
        paint.setStyle(Paint.Style.FILL);
        paint.setColor(Color.argb(Math.round(alphaNorm * 0.30f), 0, 0, 0));
        TMP_RECT.set(rect.left, rect.top + dp(1.5f, density), rect.right, rect.bottom + dp(1.5f, density));
        canvas.drawRoundRect(TMP_RECT, corner, corner, paint);

        // Glassmorphism fill
        if (active) {
            paint.setColor(Color.argb(
                    Math.min(240, Math.round(alphaNorm * 0.65f)),
                    Color.red(tint), Color.green(tint), Color.blue(tint)));
        } else {
            paint.setColor(Color.argb(Math.round(alphaNorm * 0.52f), 13, 18, 26));
        }
        canvas.drawRoundRect(rect, corner, corner, paint);

        // Crisp glass border
        paint.setStyle(Paint.Style.STROKE);
        paint.setStrokeWidth(dp(selectedInEditor ? 2.4f : (active ? 1.8f : 1.25f), density));
        if (selectedInEditor) {
            paint.setColor(0xFFFACC15);
        } else if (active) {
            paint.setColor(Color.argb(Math.min(255, alphaNorm + 40), Color.red(tint), Color.green(tint), Color.blue(tint)));
        } else {
            paint.setColor(Color.argb(Math.round(alphaNorm * 0.48f), 255, 255, 255));
        }
        canvas.drawRoundRect(rect, corner, corner, paint);

        // Subtle top glass specular highlight
        if (!selectedInEditor) {
            paint.setStrokeWidth(dp(0.8f, density));
            paint.setColor(Color.argb(Math.round(alphaNorm * 0.22f), 255, 255, 255));
            canvas.drawLine(
                    rect.left + corner * 0.55f,
                    rect.top + dp(1.5f, density),
                    rect.right - corner * 0.55f,
                    rect.top + dp(1.5f, density),
                    paint);
        }

        int fgAlpha = Math.min(255, Math.round(b.opacity * 255f + 25f));
        int iconColor = active ? 0xFFFFFFFF : Color.argb(fgAlpha,
                (Color.red(tint) + 255 * 2) / 3,
                (Color.green(tint) + 255 * 2) / 3,
                (Color.blue(tint) + 255 * 2) / 3);

        float minSide = Math.min(rect.width(), rect.height());
        if (b.iconKind != ControlButtonSpec.ICON_TEXT) {
            boolean showSubLabel = minSide >= dp(46f, density) && b.label != null && !b.label.isEmpty();
            float iconCy = showSubLabel ? (rect.centerY() - minSide * 0.10f) : rect.centerY();
            float iconR = minSide * (showSubLabel ? 0.23f : 0.27f);
            drawVectorIcon(canvas, b.iconKind, rect.centerX(), iconCy, iconR, iconColor, density, paint);

            if (showSubLabel) {
                paint.setStyle(Paint.Style.FILL);
                paint.setTextAlign(Paint.Align.CENTER);
                paint.setTypeface(Typeface.DEFAULT_BOLD);
                paint.setTextSize(dp(8.5f, density));
                paint.setColor(Color.argb(Math.round(fgAlpha * 0.88f), 241, 245, 249));
                canvas.drawText(b.label, rect.centerX(), rect.bottom - dp(5.5f, density), paint);
            }
        } else {
            paint.setStyle(Paint.Style.FILL);
            paint.setTextAlign(Paint.Align.CENTER);
            paint.setTypeface(Typeface.DEFAULT_BOLD);
            String txt = b.label != null ? b.label : "";
            float tSize = Math.min(minSide * 0.36f, dp(12.5f, density));
            if (txt.length() > 5) tSize *= 0.78f;
            paint.setTextSize(Math.max(dp(8.5f, density), tSize));
            paint.setColor(iconColor);
            float ty = rect.centerY() - ((paint.descent() + paint.ascent()) * 0.5f);
            canvas.drawText(txt, rect.centerX(), ty, paint);
        }

        if (showEditorBadges) {
            paint.setStyle(Paint.Style.FILL);
            paint.setTextAlign(Paint.Align.RIGHT);
            paint.setTypeface(Typeface.MONOSPACE);
            paint.setTextSize(dp(7.5f, density));
            String tag = (b.toggleMode ? "T" : "") + (b.swipeable ? "S" : "") + (b.locked ? "L" : "");
            if (!tag.isEmpty()) {
                paint.setColor(0xFFFACC15);
                canvas.drawText(tag, rect.right - dp(4f, density), rect.top + dp(9.5f, density), paint);
            }
        }
    }

    private static void drawVectorIcon(
            Canvas canvas, int kind, float cx, float cy, float r, int color, float density, Paint p) {
        p.setColor(color);
        p.setStrokeCap(Paint.Cap.ROUND);
        p.setStrokeJoin(Paint.Join.ROUND);

        switch (kind) {
            case ControlButtonSpec.ICON_SWORD: {
                p.setStyle(Paint.Style.STROKE);
                p.setStrokeWidth(dp(2.1f, density));
                // Sword blade diagonal bottom-left to top-right
                canvas.drawLine(cx - r * 0.7f, cy + r * 0.7f, cx + r * 0.78f, cy - r * 0.78f, p);
                // Crossguard
                canvas.drawLine(cx - r * 0.55f, cy + r * 0.15f, cx - r * 0.15f, cy + r * 0.55f, p);
                break;
            }
            case ControlButtonSpec.ICON_CUBE: {
                p.setStyle(Paint.Style.STROKE);
                p.setStrokeWidth(dp(1.7f, density));
                float h = r * 0.88f;
                float w = r * 0.80f;
                TMP_PATH.reset();
                TMP_PATH.moveTo(cx, cy - h);
                TMP_PATH.lineTo(cx + w, cy - h * 0.45f);
                TMP_PATH.lineTo(cx + w, cy + h * 0.45f);
                TMP_PATH.lineTo(cx, cy + h);
                TMP_PATH.lineTo(cx - w, cy + h * 0.45f);
                TMP_PATH.lineTo(cx - w, cy - h * 0.45f);
                TMP_PATH.close();
                canvas.drawPath(TMP_PATH, p);
                canvas.drawLine(cx - w, cy - h * 0.45f, cx, cy + dp(0.5f, density), p);
                canvas.drawLine(cx + w, cy - h * 0.45f, cx, cy + dp(0.5f, density), p);
                canvas.drawLine(cx, cy + dp(0.5f, density), cx, cy + h, p);
                break;
            }
            case ControlButtonSpec.ICON_JUMP: {
                p.setStyle(Paint.Style.STROKE);
                p.setStrokeWidth(dp(2.3f, density));
                TMP_PATH.reset();
                TMP_PATH.moveTo(cx - r * 0.68f, cy + r * 0.15f);
                TMP_PATH.lineTo(cx, cy - r * 0.62f);
                TMP_PATH.lineTo(cx + r * 0.68f, cy + r * 0.15f);
                canvas.drawPath(TMP_PATH, p);
                canvas.drawLine(cx, cy - r * 0.52f, cx, cy + r * 0.72f, p);
                break;
            }
            case ControlButtonSpec.ICON_SNEAK: {
                p.setStyle(Paint.Style.STROKE);
                p.setStrokeWidth(dp(2.1f, density));
                TMP_PATH.reset();
                TMP_PATH.moveTo(cx - r * 0.62f, cy - r * 0.25f);
                TMP_PATH.lineTo(cx, cy + r * 0.35f);
                TMP_PATH.lineTo(cx + r * 0.62f, cy - r * 0.25f);
                canvas.drawPath(TMP_PATH, p);
                canvas.drawLine(cx - r * 0.62f, cy + r * 0.66f, cx + r * 0.62f, cy + r * 0.66f, p);
                break;
            }
            case ControlButtonSpec.ICON_SPRINT: {
                p.setStyle(Paint.Style.STROKE);
                p.setStrokeWidth(dp(2.0f, density));
                for (int i = -1; i <= 0; i++) {
                    float ox = i * r * 0.46f;
                    TMP_PATH.reset();
                    TMP_PATH.moveTo(cx + ox - r * 0.16f, cy - r * 0.55f);
                    TMP_PATH.lineTo(cx + ox + r * 0.42f, cy);
                    TMP_PATH.lineTo(cx + ox - r * 0.16f, cy + r * 0.55f);
                    canvas.drawPath(TMP_PATH, p);
                }
                break;
            }
            case ControlButtonSpec.ICON_INV: {
                p.setStyle(Paint.Style.FILL);
                float dotR = Math.max(dp(1.4f, density), r * 0.17f);
                float step = r * 0.55f;
                for (int gy = -1; gy <= 1; gy++) {
                    for (int gx = -1; gx <= 1; gx++) {
                        canvas.drawCircle(cx + gx * step, cy + gy * step, dotR, p);
                    }
                }
                break;
            }
            case ControlButtonSpec.ICON_DROP: {
                p.setStyle(Paint.Style.STROKE);
                p.setStrokeWidth(dp(1.9f, density));
                canvas.drawLine(cx, cy - r * 0.68f, cx, cy + r * 0.36f, p);
                TMP_PATH.reset();
                TMP_PATH.moveTo(cx - r * 0.42f, cy);
                TMP_PATH.lineTo(cx, cy + r * 0.42f);
                TMP_PATH.lineTo(cx + r * 0.42f, cy);
                canvas.drawPath(TMP_PATH, p);
                canvas.drawLine(cx - r * 0.65f, cy + r * 0.72f, cx + r * 0.65f, cy + r * 0.72f, p);
                break;
            }
            case ControlButtonSpec.ICON_CHAT: {
                p.setStyle(Paint.Style.STROKE);
                p.setStrokeWidth(dp(1.7f, density));
                TMP_RECT.set(cx - r * 0.78f, cy - r * 0.56f, cx + r * 0.78f, cy + r * 0.42f);
                canvas.drawRoundRect(TMP_RECT, dp(3f, density), dp(3f, density), p);
                canvas.drawLine(cx - r * 0.35f, cy + r * 0.42f, cx - r * 0.48f, cy + r * 0.78f, p);
                break;
            }
            case ControlButtonSpec.ICON_PAUSE: {
                p.setStyle(Paint.Style.FILL);
                float bw = Math.max(dp(2.2f, density), r * 0.24f);
                float bh = r * 0.64f;
                float gap = r * 0.32f;
                canvas.drawRoundRect(new RectF(cx - gap - bw, cy - bh, cx - gap, cy + bh), dp(1.2f, density), dp(1.2f, density), p);
                canvas.drawRoundRect(new RectF(cx + gap, cy - bh, cx + gap + bw, cy + bh), dp(1.2f, density), dp(1.2f, density), p);
                break;
            }
            case ControlButtonSpec.ICON_CURSOR: {
                p.setStyle(Paint.Style.STROKE);
                p.setStrokeWidth(dp(1.8f, density));
                TMP_PATH.reset();
                TMP_PATH.moveTo(cx - r * 0.42f, cy - r * 0.72f);
                TMP_PATH.lineTo(cx - r * 0.42f, cy + r * 0.65f);
                TMP_PATH.lineTo(cx - r * 0.05f, cy + r * 0.32f);
                TMP_PATH.lineTo(cx + r * 0.34f, cy + r * 0.74f);
                TMP_PATH.lineTo(cx + r * 0.52f, cy + r * 0.14f);
                TMP_PATH.close();
                canvas.drawPath(TMP_PATH, p);
                break;
            }
            case ControlButtonSpec.ICON_KEYBOARD: {
                p.setStyle(Paint.Style.STROKE);
                p.setStrokeWidth(dp(1.6f, density));
                TMP_RECT.set(cx - r * 0.85f, cy - r * 0.52f, cx + r * 0.85f, cy + r * 0.52f);
                canvas.drawRoundRect(TMP_RECT, dp(2.5f, density), dp(2.5f, density), p);
                canvas.drawLine(cx - r * 0.42f, cy + r * 0.24f, cx + r * 0.42f, cy + r * 0.24f, p);
                canvas.drawLine(cx - r * 0.55f, cy - r * 0.16f, cx - r * 0.28f, cy - r * 0.16f, p);
                canvas.drawLine(cx - r * 0.08f, cy - r * 0.16f, cx + r * 0.16f, cy - r * 0.16f, p);
                canvas.drawLine(cx + r * 0.34f, cy - r * 0.16f, cx + r * 0.55f, cy - r * 0.16f, p);
                break;
            }
            case ControlButtonSpec.ICON_CAMERA: {
                p.setStyle(Paint.Style.STROKE);
                p.setStrokeWidth(dp(1.7f, density));
                TMP_RECT.set(cx - r * 0.82f, cy - r * 0.46f, cx + r * 0.82f, cy + r * 0.56f);
                canvas.drawRoundRect(TMP_RECT, dp(2.5f, density), dp(2.5f, density), p);
                canvas.drawCircle(cx, cy + r * 0.05f, r * 0.28f, p);
                break;
            }
            case ControlButtonSpec.ICON_SWAP: {
                p.setStyle(Paint.Style.STROKE);
                p.setStrokeWidth(dp(1.8f, density));
                canvas.drawLine(cx - r * 0.65f, cy - r * 0.26f, cx + r * 0.65f, cy - r * 0.26f, p);
                canvas.drawLine(cx + r * 0.32f, cy - r * 0.55f, cx + r * 0.65f, cy - r * 0.26f, p);
                canvas.drawLine(cx - r * 0.65f, cy + r * 0.26f, cx + r * 0.65f, cy + r * 0.26f, p);
                canvas.drawLine(cx - r * 0.32f, cy + r * 0.55f, cx - r * 0.65f, cy + r * 0.26f, p);
                break;
            }
            case ControlButtonSpec.ICON_PICK: {
                p.setStyle(Paint.Style.STROKE);
                p.setStrokeWidth(dp(1.8f, density));
                canvas.drawCircle(cx, cy, r * 0.68f, p);
                canvas.drawLine(cx - r * 0.32f, cy, cx + r * 0.32f, cy, p);
                canvas.drawLine(cx, cy - r * 0.32f, cx, cy + r * 0.32f, p);
                break;
            }
            case ControlButtonSpec.ICON_EYE: {
                p.setStyle(Paint.Style.STROKE);
                p.setStrokeWidth(dp(1.7f, density));
                TMP_RECT.set(cx - r * 0.82f, cy - r * 0.44f, cx + r * 0.82f, cy + r * 0.44f);
                canvas.drawOval(TMP_RECT, p);
                p.setStyle(Paint.Style.FILL);
                canvas.drawCircle(cx, cy, r * 0.24f, p);
                break;
            }
            case ControlButtonSpec.ICON_DOTS: {
                p.setStyle(Paint.Style.FILL);
                float s = r * 0.24f;
                for (int i = -1; i <= 1; i++) {
                    canvas.drawRect(cx + i * r * 0.78f - s, cy - s, cx + i * r * 0.78f + s, cy + s, p);
                }
                break;
            }
            case ControlButtonSpec.ICON_SLASH: {
                p.setStyle(Paint.Style.STROKE);
                p.setStrokeWidth(dp(3.2f, density));
                canvas.drawLine(cx - r * 0.45f, cy + r * 0.85f, cx + r * 0.45f, cy - r * 0.85f, p);
                break;
            }
            case ControlButtonSpec.ICON_MENU: {
                p.setStyle(Paint.Style.STROKE);
                p.setStrokeWidth(dp(1.9f, density));
                canvas.drawLine(cx - r * 0.6f, cy - r * 0.45f, cx + r * 0.6f, cy - r * 0.45f, p);
                canvas.drawLine(cx - r * 0.6f, cy, cx + r * 0.6f, cy, p);
                canvas.drawLine(cx - r * 0.6f, cy + r * 0.45f, cx + r * 0.6f, cy + r * 0.45f, p);
                break;
            }
        }
    }

    /** Stone-grey Minecraft button: dark outline, light top-left bevel, dark bottom-right bevel. */
    static void drawPixelRect(Canvas canvas, RectF r, boolean pressed, int alpha, float density, Paint paint) {
        float o = dp(2.2f, density);
        float bv = dp(2.2f, density);
        paint.setStyle(Paint.Style.FILL);
        paint.setColor(Color.argb(alpha, 0x18, 0x18, 0x18));
        canvas.drawRect(r, paint);
        float l = r.left + o, t = r.top + o, rr = r.right - o, b = r.bottom - o;
        paint.setColor(Color.argb(alpha, pressed ? 0x6E : 0x8B, pressed ? 0x6E : 0x8B, pressed ? 0x6E : 0x8B));
        canvas.drawRect(l, t, rr, b, paint);
        paint.setColor(Color.argb(alpha, pressed ? 0x55 : 0xC6, pressed ? 0x55 : 0xC6, pressed ? 0x55 : 0xC6));
        canvas.drawRect(l, t, rr, t + bv, paint);
        canvas.drawRect(l, t, l + bv, b, paint);
        paint.setColor(Color.argb(alpha, pressed ? 0xC6 : 0x55, pressed ? 0xC6 : 0x55, pressed ? 0xC6 : 0x55));
        canvas.drawRect(l, b - bv, rr, b, paint);
        canvas.drawRect(rr - bv, t, rr, b, paint);
    }

    private static void drawPixelButton(
            Canvas canvas, RectF rect, ControlButtonSpec b, boolean active, boolean selectedInEditor,
            boolean showEditorBadges, float density, Paint paint) {
        int alpha = Math.round(b.opacity * 255f);
        drawPixelRect(canvas, rect, active, alpha, density, paint);
        if (selectedInEditor) {
            paint.setStyle(Paint.Style.STROKE);
            paint.setStrokeWidth(dp(2.4f, density));
            paint.setColor(0xFFFACC15);
            canvas.drawRect(rect, paint);
        }
        float minSide = Math.min(rect.width(), rect.height());
        int ink = Color.argb(Math.min(255, alpha + 20), 0x14, 0x14, 0x14);
        if (b.iconKind != ControlButtonSpec.ICON_TEXT) {
            float bold = dp(0.55f, density);
            drawVectorIcon(canvas, b.iconKind, rect.centerX() - bold, rect.centerY() - bold, minSide * 0.30f, ink, density, paint);
            drawVectorIcon(canvas, b.iconKind, rect.centerX() + bold, rect.centerY() + bold, minSide * 0.30f, ink, density, paint);
        } else {
            paint.setStyle(Paint.Style.FILL);
            paint.setTextAlign(Paint.Align.CENTER);
            paint.setTypeface(Typeface.DEFAULT_BOLD);
            String txt = b.label != null ? b.label : "";
            paint.setTextSize(Math.max(dp(9f, density), Math.min(minSide * 0.42f, dp(17f, density))));
            paint.setColor(ink);
            float ty = rect.centerY() - ((paint.descent() + paint.ascent()) * 0.5f);
            canvas.drawText(txt, rect.centerX(), ty, paint);
        }
    }

    private static void drawPixelPad(Canvas canvas, float cx, float cy, float halfSpan, float thumbX, float thumbY,
                                     boolean activeTouch, boolean selectedInEditor, int alpha, float density, Paint paint) {
        TMP_RECT.set(cx - halfSpan, cy - halfSpan, cx + halfSpan, cy + halfSpan);
        paint.setStyle(Paint.Style.FILL);
        paint.setColor(Color.argb(Math.round(alpha * 0.28f), 255, 255, 255));
        canvas.drawRect(TMP_RECT, paint);
        paint.setStyle(Paint.Style.STROKE);
        paint.setStrokeWidth(dp(selectedInEditor ? 3f : 2.4f, density));
        paint.setColor(selectedInEditor ? 0xFFFACC15 : Color.argb(Math.round(alpha * 0.75f), 255, 255, 255));
        canvas.drawRect(TMP_RECT, paint);
        float k = halfSpan * 0.50f;
        float tx = activeTouch ? thumbX : cx;
        float ty = activeTouch ? thumbY : cy;
        TMP_RECT.set(tx - k, ty - k, tx + k, ty + k);
        drawPixelRect(canvas, TMP_RECT, activeTouch, Math.min(255, Math.round(alpha * 1.05f)), density, paint);
    }

    public static void drawMovementPad(
            Canvas canvas,
            ControlProfile profile,
            float cx,
            float cy,
            float thumbX,
            float thumbY,
            float moveX,
            float moveY,
            boolean activeTouch,
            boolean selectedInEditor,
            float density,
            Paint paint) {
        float halfSpan = (profile.joystickSizeDp * 0.5f) * density;
        int alpha = Math.round(profile.joystickOpacity * 255f);

        if (profile.joystickMode == ControlProfile.STICK_DPAD) {
            float cell = halfSpan * 0.62f;
            float gap = dp(2.5f, density);
            boolean wOn = moveY > 0.25f;
            boolean sOn = moveY < -0.25f;
            boolean aOn = moveX < -0.25f;
            boolean dOn = moveX > 0.25f;

            // 4 cardinal arms of connected MJLauncher/Bedrock D-Pad
            drawDpadArm(canvas, cx, cy - cell - gap, cell, wOn, selectedInEditor, alpha, 0, density, paint);
            drawDpadArm(canvas, cx, cy + cell + gap, cell, sOn, selectedInEditor, alpha, 1, density, paint);
            drawDpadArm(canvas, cx - cell - gap, cy, cell, aOn, selectedInEditor, alpha, 2, density, paint);
            drawDpadArm(canvas, cx + cell + gap, cy, cell, dOn, selectedInEditor, alpha, 3, density, paint);

            // Subtle diagonal strafe assist triangles when pressing forward
            if (wOn || selectedInEditor) {
                drawDiagPad(canvas, cx - cell - gap, cy - cell - gap, cell * 0.82f, wOn && aOn, alpha, "↖", density, paint);
                drawDiagPad(canvas, cx + cell + gap, cy - cell - gap, cell * 0.82f, wOn && dOn, alpha, "↗", density, paint);
            }

            // Center pivot circle
            TMP_RECT.set(cx - cell * 0.46f, cy - cell * 0.46f, cx + cell * 0.46f, cy + cell * 0.46f);
            paint.setStyle(Paint.Style.FILL);
            paint.setColor(Color.argb(Math.round(alpha * 0.46f), 14, 20, 28));
            canvas.drawRoundRect(TMP_RECT, dp(8f, density), dp(8f, density), paint);
            paint.setStyle(Paint.Style.STROKE);
            paint.setStrokeWidth(dp(selectedInEditor ? 2f : 1.1f, density));
            paint.setColor(selectedInEditor ? 0xFFFACC15 : Color.argb(Math.round(alpha * 0.40f), 255, 255, 255));
            canvas.drawRoundRect(TMP_RECT, dp(8f, density), dp(8f, density), paint);
            return;
        }

        if (profile.joystickMode == ControlProfile.STICK_PIXEL) {
            drawPixelPad(canvas, cx, cy, halfSpan, thumbX, thumbY, activeTouch, selectedInEditor, alpha, density, paint);
            return;
        }

        // Modern Analog Joystick
        paint.setStyle(Paint.Style.FILL);
        paint.setColor(Color.argb(Math.round(alpha * 0.34f), 13, 18, 26));
        canvas.drawCircle(cx, cy, halfSpan, paint);

        paint.setStyle(Paint.Style.STROKE);
        paint.setStrokeWidth(dp(selectedInEditor ? 2.4f : 1.5f, density));
        paint.setColor(selectedInEditor ? 0xFFFACC15 : Color.argb(Math.round(alpha * 0.45f), 255, 255, 255));
        canvas.drawCircle(cx, cy, halfSpan, paint);

        // Cardinal tick lines inside ring
        paint.setStrokeWidth(dp(1.2f, density));
        paint.setColor(Color.argb(Math.round(alpha * 0.28f), 255, 255, 255));
        float tIn = halfSpan * 0.78f;
        float tOut = halfSpan * 0.94f;
        canvas.drawLine(cx, cy - tIn, cx, cy - tOut, paint);
        canvas.drawLine(cx, cy + tIn, cx, cy + tOut, paint);
        canvas.drawLine(cx - tIn, cy, cx - tOut, cy, paint);
        canvas.drawLine(cx + tIn, cy, cx + tOut, cy, paint);

        // Thumb cap
        float tx = activeTouch ? thumbX : cx;
        float ty = activeTouch ? thumbY : cy;
        float knobR = halfSpan * 0.40f;
        paint.setStyle(Paint.Style.FILL);
        paint.setColor(activeTouch
                ? Color.argb(Math.min(240, Math.round(alpha * 0.78f)), 61, 220, 132)
                : Color.argb(Math.round(alpha * 0.58f), 226, 232, 240));
        canvas.drawCircle(tx, ty, knobR, paint);

        paint.setStyle(Paint.Style.STROKE);
        paint.setStrokeWidth(dp(1.5f, density));
        paint.setColor(Color.argb(Math.min(255, alpha + 30), 255, 255, 255));
        canvas.drawCircle(tx, ty, knobR, paint);
    }

    private static void drawDpadArm(
            Canvas canvas, float cx, float cy, float size, boolean active, boolean selected,
            int alpha, int dir, float density, Paint paint) {
        float half = size * 0.5f;
        TMP_RECT.set(cx - half, cy - half, cx + half, cy + half);
        paint.setStyle(Paint.Style.FILL);
        if (active) {
            paint.setColor(Color.argb(Math.min(230, Math.round(alpha * 0.72f)), 61, 220, 132));
        } else {
            paint.setColor(Color.argb(Math.round(alpha * 0.50f), 14, 20, 28));
        }
        canvas.drawRoundRect(TMP_RECT, dp(8f, density), dp(8f, density), paint);

        paint.setStyle(Paint.Style.STROKE);
        paint.setStrokeWidth(dp(selected ? 2.2f : 1.25f, density));
        paint.setColor(selected ? 0xFFFACC15
                : active ? 0xFF3DDC84 : Color.argb(Math.round(alpha * 0.46f), 255, 255, 255));
        canvas.drawRoundRect(TMP_RECT, dp(8f, density), dp(8f, density), paint);

        // Crisp chevron arrow inside arm
        float ar = size * 0.22f;
        paint.setStrokeWidth(dp(2.0f, density));
        paint.setColor(Color.argb(Math.min(255, alpha + 35), 255, 255, 255));
        TMP_PATH.reset();
        if (dir == 0) { // UP
            TMP_PATH.moveTo(cx - ar, cy + ar * 0.45f);
            TMP_PATH.lineTo(cx, cy - ar * 0.55f);
            TMP_PATH.lineTo(cx + ar, cy + ar * 0.45f);
        } else if (dir == 1) { // DOWN
            TMP_PATH.moveTo(cx - ar, cy - ar * 0.45f);
            TMP_PATH.lineTo(cx, cy + ar * 0.55f);
            TMP_PATH.lineTo(cx + ar, cy - ar * 0.45f);
        } else if (dir == 2) { // LEFT
            TMP_PATH.moveTo(cx + ar * 0.45f, cy - ar);
            TMP_PATH.lineTo(cx - ar * 0.55f, cy);
            TMP_PATH.lineTo(cx + ar * 0.45f, cy + ar);
        } else { // RIGHT
            TMP_PATH.moveTo(cx - ar * 0.45f, cy - ar);
            TMP_PATH.lineTo(cx + ar * 0.55f, cy);
            TMP_PATH.lineTo(cx - ar * 0.45f, cy + ar);
        }
        canvas.drawPath(TMP_PATH, paint);
    }

    private static void drawDiagPad(
            Canvas canvas, float cx, float cy, float size, boolean active, int alpha, String sym, float density, Paint paint) {
        float half = size * 0.48f;
        TMP_RECT.set(cx - half, cy - half, cx + half, cy + half);
        paint.setStyle(Paint.Style.FILL);
        paint.setColor(active
                ? Color.argb(Math.min(210, Math.round(alpha * 0.65f)), 61, 220, 132)
                : Color.argb(Math.round(alpha * 0.34f), 14, 20, 28));
        canvas.drawRoundRect(TMP_RECT, dp(7f, density), dp(7f, density), paint);
        paint.setStyle(Paint.Style.STROKE);
        paint.setStrokeWidth(dp(1f, density));
        paint.setColor(Color.argb(Math.round(alpha * 0.30f), 255, 255, 255));
        canvas.drawRoundRect(TMP_RECT, dp(7f, density), dp(7f, density), paint);

        paint.setStyle(Paint.Style.FILL);
        paint.setTextAlign(Paint.Align.CENTER);
        paint.setTextSize(size * 0.36f);
        paint.setColor(Color.argb(Math.round(alpha * 0.72f), 241, 245, 249));
        float ty = cy - ((paint.descent() + paint.ascent()) * 0.5f);
        canvas.drawText(sym, cx, ty, paint);
    }

    public static void drawCrosshair(Canvas canvas, int w, int h, float density, Paint paint) {
        float cx = w * 0.5f;
        float cy = h * 0.5f;
        float arm = dp(7.5f, density);
        float gap = dp(1.5f, density);

        paint.setStyle(Paint.Style.STROKE);
        paint.setStrokeCap(Paint.Cap.SQUARE);
        paint.setStrokeWidth(dp(3.0f, density));
        paint.setColor(0x99000000);
        canvas.drawLine(cx - arm, cy, cx - gap, cy, paint);
        canvas.drawLine(cx + gap, cy, cx + arm, cy, paint);
        canvas.drawLine(cx, cy - arm, cx, cy - gap, paint);
        canvas.drawLine(cx, cy + gap, cx, cy + arm, paint);

        paint.setStrokeWidth(dp(1.6f, density));
        paint.setColor(0xEEFFFFFF);
        canvas.drawLine(cx - arm, cy, cx - gap, cy, paint);
        canvas.drawLine(cx + gap, cy, cx + arm, cy, paint);
        canvas.drawLine(cx, cy - arm, cx, cy - gap, paint);
        canvas.drawLine(cx, cy + gap, cx, cy + arm, paint);
    }

    public static void drawHotbar(
            Canvas canvas, int w, int h, int selectedSlot, boolean showName, float density, Paint paint) {
        RectF bar = getHotbarRect(w, h, density);
        float slotW = bar.width() / 9f;

        paint.setStyle(Paint.Style.FILL);
        paint.setColor(0xA60E141E);
        canvas.drawRoundRect(bar, dp(7f, density), dp(7f, density), paint);

        paint.setStyle(Paint.Style.STROKE);
        paint.setStrokeWidth(dp(1.2f, density));
        paint.setColor(0x66FFFFFF);
        canvas.drawRoundRect(bar, dp(7f, density), dp(7f, density), paint);

        for (int i = 0; i < 9; i++) {
            float sx = bar.left + i * slotW;
            TMP_RECT.set(sx + dp(2.5f, density), bar.top + dp(2.5f, density),
                    sx + slotW - dp(2.5f, density), bar.bottom - dp(2.5f, density));

            if (i == selectedSlot) {
                paint.setStyle(Paint.Style.FILL);
                paint.setColor(0x33FFFFFF);
                canvas.drawRoundRect(TMP_RECT, dp(4f, density), dp(4f, density), paint);
                paint.setStyle(Paint.Style.STROKE);
                paint.setStrokeWidth(dp(1.8f, density));
                paint.setColor(0xFFF8FAFC);
                canvas.drawRoundRect(TMP_RECT, dp(4f, density), dp(4f, density), paint);
            }

            // Isometric mini voxel block
            float bw = dp(13f, density);
            float cx = TMP_RECT.centerX();
            float cy = TMP_RECT.centerY() - dp(1f, density);
            drawMiniIsoBlock(canvas, cx, cy, bw * 0.5f, HOTBAR_TOP_COLORS[i], HOTBAR_SIDE_COLORS[i], paint);

            paint.setStyle(Paint.Style.FILL);
            paint.setTextAlign(Paint.Align.LEFT);
            paint.setTypeface(Typeface.MONOSPACE);
            paint.setTextSize(dp(7.5f, density));
            paint.setColor(i == selectedSlot ? 0xFFFFFFFF : 0x99CBD5E1);
            canvas.drawText(String.valueOf(i + 1), TMP_RECT.left + dp(3f, density), TMP_RECT.top + dp(9f, density), paint);
        }

        if (showName && selectedSlot >= 0 && selectedSlot < HOTBAR_NAMES.length) {
            paint.setStyle(Paint.Style.FILL);
            paint.setTextAlign(Paint.Align.CENTER);
            paint.setTypeface(Typeface.DEFAULT_BOLD);
            paint.setTextSize(dp(11f, density));
            paint.setColor(0xCC000000);
            canvas.drawText(HOTBAR_NAMES[selectedSlot], bar.centerX() + dp(1f, density), bar.top - dp(7f, density), paint);
            paint.setColor(0xFFF8FAFC);
            canvas.drawText(HOTBAR_NAMES[selectedSlot], bar.centerX(), bar.top - dp(8f, density), paint);
        }
    }

    private static void drawMiniIsoBlock(
            Canvas canvas, float cx, float cy, float r, int topColor, int sideColor, Paint paint) {
        paint.setStyle(Paint.Style.FILL);
        // Top diamond
        TMP_PATH.reset();
        TMP_PATH.moveTo(cx, cy - r);
        TMP_PATH.lineTo(cx + r, cy - r * 0.42f);
        TMP_PATH.lineTo(cx, cy + r * 0.16f);
        TMP_PATH.lineTo(cx - r, cy - r * 0.42f);
        TMP_PATH.close();
        paint.setColor(topColor);
        canvas.drawPath(TMP_PATH, paint);

        // Left face
        TMP_PATH.reset();
        TMP_PATH.moveTo(cx - r, cy - r * 0.42f);
        TMP_PATH.lineTo(cx, cy + r * 0.16f);
        TMP_PATH.lineTo(cx, cy + r);
        TMP_PATH.lineTo(cx - r, cy + r * 0.42f);
        TMP_PATH.close();
        paint.setColor(sideColor);
        canvas.drawPath(TMP_PATH, paint);

        // Right face darker
        TMP_PATH.reset();
        TMP_PATH.moveTo(cx, cy + r * 0.16f);
        TMP_PATH.lineTo(cx + r, cy - r * 0.42f);
        TMP_PATH.lineTo(cx + r, cy + r * 0.42f);
        TMP_PATH.lineTo(cx, cy + r);
        TMP_PATH.close();
        paint.setColor(Color.argb(255,
                Math.max(0, Color.red(sideColor) - 24),
                Math.max(0, Color.green(sideColor) - 24),
                Math.max(0, Color.blue(sideColor) - 24)));
        canvas.drawPath(TMP_PATH, paint);
    }

    public static void drawEditorWorldBackdrop(Canvas canvas, int w, int h, float density, Paint paint) {
        // Sky gradient
        paint.setStyle(Paint.Style.FILL);
        paint.setShader(new LinearGradient(0, 0, 0, h * 0.62f,
                0xFF38BDF8, 0xFFBAE6FD, Shader.TileMode.CLAMP));
        canvas.drawRect(0, 0, w, h * 0.62f, paint);
        paint.setShader(null);

        // Blocky clouds
        paint.setColor(0xCCFFFFFF);
        canvas.drawRect(w * 0.12f, h * 0.14f, w * 0.28f, h * 0.20f, paint);
        canvas.drawRect(w * 0.60f, h * 0.10f, w * 0.80f, h * 0.16f, paint);
        canvas.drawRect(w * 0.36f, h * 0.22f, w * 0.49f, h * 0.27f, paint);

        // Rolling voxel hills + grass floor
        paint.setColor(0xFF3B873E);
        canvas.drawRect(0, h * 0.56f, w, h * 0.68f, paint);
        paint.setColor(0xFF4CA64C);
        canvas.drawRect(0, h * 0.62f, w, h * 0.72f, paint);
        paint.setColor(0xFF6B4226);
        canvas.drawRect(0, h * 0.72f, w, h, paint);

        // Subtle voxel block grid across terrain
        paint.setStyle(Paint.Style.STROKE);
        paint.setStrokeWidth(dp(1f, density));
        paint.setColor(0x26000000);
        float tile = dp(42f, density);
        for (float gx = 0; gx < w; gx += tile) {
            canvas.drawLine(gx, h * 0.56f, gx, h, paint);
        }
        for (float gy = h * 0.56f; gy < h; gy += tile) {
            canvas.drawLine(0, gy, w, gy, paint);
        }

        // Crosshair + Hotbar preview so layout matches real gameplay 1:1
        drawCrosshair(canvas, w, h, density, paint);
        drawHotbar(canvas, w, h, 0, false, density, paint);
    }
}
