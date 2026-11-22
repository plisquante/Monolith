/*
 * Desktop stand-in for the TouchCore Android host (libtouchcore_mono.so).
 *
 * The engine (runtime/gl_shim.zig) finds its window, size, GL context and input through five functions of that
 * library. This mock provides them on a Linux desktop with Mesa EGL (pbuffer surface, OpenGL ES 3), so the whole
 * Minecraft start-up, window creation and GLES rendering path can be exercised without a phone:
 *
 *   gcc -shared -fPIC -O1 tools/mock_android_host.c -o <dir>/libtouchcore_mono.so -lEGL -ldl
 *   ln -s libGLESv2.so.2 <dir>/libGLESv3.so            # the engine dlopens libGLESv3.so
 *   LD_LIBRARY_PATH=<dir>:<natives> MONO_RHI=1 MONO_NO_LWJGL=1 <engine> ...
 *
 * MOCK_FRAMES=N dumps frame N as a PPM to MOCK_DUMP (default /tmp/mock_frame.ppm); MOCK_EXIT_AFTER=M exits after M
 * frames. Both go to stderr/stdout of the engine as "MOCK_HOST ..." lines.
 */
#define _GNU_SOURCE
#include <EGL/egl.h>
#include <GLES3/gl3.h>
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <unistd.h>

typedef struct {
    int32_t width, height;
    uint8_t key_w, key_a, key_s, key_d, key_jump, key_sneak, key_sprint, key_lmb, key_rmb, key_esc, key_e, key_q,
        key_f, key_t, key_f3, key_f5, key_tab, third_person, cursor_visible, _pad[2];
    int32_t hotbar_slot;
    float look_dx, look_dy, cursor_x, cursor_y;
} TcPollSnapshot;

static const int W = 1280, H = 720;
static EGLDisplay dpy;
static EGLSurface surf;
static EGLContext ctx;
static unsigned long frames;

int32_t tc_android_has_window(void) { return 1; }

void tc_android_get_size(int32_t *w, int32_t *h) {
    if (w) *w = W;
    if (h) *h = H;
}

int32_t tc_android_init_egl(int32_t *w, int32_t *h) {
    if (w) *w = W;
    if (h) *h = H;
    if (ctx) return eglMakeCurrent(dpy, surf, surf, ctx) ? 1 : 0;
    dpy = eglGetDisplay(EGL_DEFAULT_DISPLAY);
    EGLint maj, min;
    if (!eglInitialize(dpy, &maj, &min)) { fprintf(stderr, "MOCK_HOST eglInitialize failed\n"); return 0; }
    eglBindAPI(EGL_OPENGL_ES_API);
    const EGLint cfg_attr[] = {EGL_SURFACE_TYPE, EGL_PBUFFER_BIT, EGL_RENDERABLE_TYPE, EGL_OPENGL_ES3_BIT,
                               EGL_RED_SIZE, 8, EGL_GREEN_SIZE, 8, EGL_BLUE_SIZE, 8, EGL_ALPHA_SIZE, 8,
                               EGL_DEPTH_SIZE, 24, EGL_NONE};
    EGLConfig cfg;
    EGLint n = 0;
    if (!eglChooseConfig(dpy, cfg_attr, &cfg, 1, &n) || n < 1) { fprintf(stderr, "MOCK_HOST no EGL config\n"); return 0; }
    const EGLint pb[] = {EGL_WIDTH, W, EGL_HEIGHT, H, EGL_NONE};
    surf = eglCreatePbufferSurface(dpy, cfg, pb);
    const EGLint ca[] = {EGL_CONTEXT_CLIENT_VERSION, 3, EGL_NONE};
    ctx = eglCreateContext(dpy, cfg, EGL_NO_CONTEXT, ca);
    if (surf == EGL_NO_SURFACE || ctx == EGL_NO_CONTEXT || !eglMakeCurrent(dpy, surf, surf, ctx)) {
        fprintf(stderr, "MOCK_HOST EGL context setup failed (0x%x)\n", eglGetError());
        return 0;
    }
    fprintf(stderr, "MOCK_HOST GLES: %s | %s\n", (const char *)glGetString(GL_VERSION), (const char *)glGetString(GL_RENDERER));
    return 1;
}

static void dump_ppm(const char *path) {
    unsigned char *px = malloc((size_t)W * H * 4);
    if (!px) return;
    glReadPixels(0, 0, W, H, GL_RGBA, GL_UNSIGNED_BYTE, px);
    FILE *f = fopen(path, "wb");
    if (f) {
        fprintf(f, "P6\n%d %d\n255\n", W, H);
        for (int y = H - 1; y >= 0; y--)
            for (int x = 0; x < W; x++) fwrite(px + ((size_t)y * W + x) * 4, 1, 3, f);
        fclose(f);
    }
    free(px);
}

static char shot_path[512];
void tc_android_swap_buffers(void) {
    glFinish();
    frames++;
    if (shot_path[0]) { dump_ppm(shot_path); fprintf(stderr, "MOCK_HOST shot %s\n", shot_path); shot_path[0] = 0; }
    if (frames % 100 == 0) fprintf(stderr, "MOCK_HOST frame %lu\n", frames);
    const char *dump = getenv("MOCK_FRAMES");
    if (dump && frames == strtoul(dump, NULL, 10)) {
        const char *p = getenv("MOCK_DUMP");
        dump_ppm(p ? p : "/tmp/mock_frame.ppm");
        fprintf(stderr, "MOCK_HOST dumped frame %lu\n", frames);
    }
    const char *every = getenv("MOCK_DUMP_EVERY"); /* MOCK_DUMP_EVERY=K: also write <MOCK_DUMP>.<frame>.ppm every K frames */
    if (every && frames % strtoul(every, NULL, 10) == 0) {
        const char *p = getenv("MOCK_DUMP");
        char name[512];
        snprintf(name, sizeof name, "%s.%lu.ppm", p ? p : "/tmp/mock_frame", frames);
        dump_ppm(name);
        fprintf(stderr, "MOCK_HOST dumped frame %lu\n", frames);
    }
    const char *ex = getenv("MOCK_EXIT_AFTER");
    if (ex && frames >= strtoul(ex, NULL, 10)) {
        fprintf(stderr, "MOCK_HOST exiting after %lu frames\n", frames);
        _exit(0);
    }
}

/*
 * Live input for UI tests. When MOCK_CMD names a file, its lines are executed once and the file is removed:
 *   move X Y     cursor to X,Y in pixels (the cursor is shown, as in the launcher's cursor mode)
 *   click        left button down for a few polls, then up
 *   key NAME     one poll with a key set: esc e q f t f3 f5 tab w a s d jump sneak
 *   shot PATH    PPM of the next presented frame
 */
static float cur_x = 0.5f, cur_y = 0.5f;
static int cursor_on, lmb_polls, key_polls;
static char key_name[16];
static int hotbar;

static void poll_commands(void) {
    const char *cmd = getenv("MOCK_CMD");
    if (!cmd) return;
    FILE *f = fopen(cmd, "r");
    if (!f) return;
    char line[600];
    while (fgets(line, sizeof line, f)) {
        float x, y;
        char arg[512];
        if (sscanf(line, "move %f %f", &x, &y) == 2) { cur_x = x / W; cur_y = y / H; cursor_on = 1; }
        else if (strncmp(line, "click", 5) == 0) lmb_polls = 6;
        else if (sscanf(line, "key %15s", key_name) == 1) key_polls = 6;
        else if (sscanf(line, "shot %511s", arg) == 1) snprintf(shot_path, sizeof shot_path, "%s", arg);
        else if (sscanf(line, "hotbar %d", &hotbar) == 1) {}
    }
    fclose(f);
    remove(cmd);
}

void tc_android_poll_snapshot(TcPollSnapshot *out) {
    if (!out) return;
    memset(out, 0, sizeof(*out));
    out->width = W;
    out->height = H;
    poll_commands();
    static unsigned long polls;
    polls++;
    if (getenv("MOCK_TRACE_POLLS") && (polls % 300 == 0 || lmb_polls == 5))
        fprintf(stderr, "MOCK_HOST poll %lu cursor_on=%d x=%.3f y=%.3f lmb_polls=%d\n", polls, cursor_on, cur_x, cur_y, lmb_polls);
    out->cursor_visible = (uint8_t)cursor_on;
    out->cursor_x = cur_x;
    out->cursor_y = cur_y;
    out->hotbar_slot = hotbar;
    if (lmb_polls > 0) { out->key_lmb = 1; lmb_polls--; }
    if (key_polls > 0) {
        key_polls--;
        if (!strcmp(key_name, "esc")) out->key_esc = 1;
        else if (!strcmp(key_name, "e")) out->key_e = 1;
        else if (!strcmp(key_name, "q")) out->key_q = 1;
        else if (!strcmp(key_name, "f")) out->key_f = 1;
        else if (!strcmp(key_name, "t")) out->key_t = 1;
        else if (!strcmp(key_name, "f3")) out->key_f3 = 1;
        else if (!strcmp(key_name, "f5")) out->key_f5 = 1;
        else if (!strcmp(key_name, "tab")) out->key_tab = 1;
        else if (!strcmp(key_name, "w")) out->key_w = 1;
        else if (!strcmp(key_name, "a")) out->key_a = 1;
        else if (!strcmp(key_name, "s")) out->key_s = 1;
        else if (!strcmp(key_name, "d")) out->key_d = 1;
        else if (!strcmp(key_name, "jump")) out->key_jump = 1;
        else if (!strcmp(key_name, "sneak")) out->key_sneak = 1;
    }
}
