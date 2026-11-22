import j2z.rhi.NativeRhi;
import java.lang.foreign.Arena;
import java.lang.foreign.MemorySegment;
import java.lang.foreign.ValueLayout;
import java.lang.invoke.MethodHandles;
import java.lang.invoke.VarHandle;

public class ClientCoreE2E {
    record Vec2(int x, int y) {}
    private static int sharedCounter = 0;

    public static void main(String[] args) throws Throwable {
        // 1. GC-free allocation burst inside explicit ArenaScope + non-blocking epoch reclamation
        long t0 = System.nanoTime();
        NativeRhi.pushArenaScope();
        long sum = 0;
        for (int i = 0; i < 20_000; i++) {
            int[] arr = new int[16];
            arr[0] = i;
            sum += arr[0];
        }
        NativeRhi.popArenaScope();
        System.gc();
        long dtMs = (System.nanoTime() - t0) / 1_000_000L;
        if (sum != 199_990_000L) throw new AssertionError("bad sum: " + sum);

        // 2. JDK 25 link-error & missing-native closure: Records, MethodHandles, VarHandle, MemorySegment
        Object rc = Vec2.class.getRecordComponents();
        if (rc == null) throw new AssertionError("record components null");

        VarHandle vhField = MethodHandles.lookup().findStaticVarHandle(ClientCoreE2E.class, "sharedCounter", int.class);
        vhField.set(42);
        if ((int) vhField.get() != 42) throw new AssertionError("static VarHandle failed");

        VarHandle intView = ValueLayout.JAVA_INT.varHandle();
        try (Arena arena = Arena.ofConfined()) {
            MemorySegment seg = arena.allocate(64);
            intView.set(seg, 0L, 0x12345678);
            int readBack = (int) intView.get(seg, 0L);
            if (readBack != 0x12345678) throw new AssertionError("MemorySegment VarHandle failed: " + readBack);
        }

        // 3 & 4. Full Java -> Zig RHI + Vanilla-safe World Culling + GPU-Driven Unified Instancing E2E
        int backend = NativeRhi.init(NativeRhi.BACKEND_AUTO, NativeRhi.POWER_ECO_BATTERY);
        if (backend < 1 || backend > 3) throw new AssertionError("unexpected backend: " + backend);

        NativeRhi.clearSections();
        // Vanilla immutable chunk snapshots: camera(0,0,0), open(0,0,-1), solid occluder(0,0,-2), behind wall(0,0,-3), behind camera(0,0,+2)
        NativeRhi.submitSectionSnapshotEx(0, 0, 0, false, false, NativeRhi.PORTAL_ALL_OPEN, 0, 0, 0);
        NativeRhi.submitSectionSnapshotEx(0, 0, -1, true, false, NativeRhi.PORTAL_ALL_OPEN, 48, 0, 0);
        NativeRhi.submitSectionSnapshotEx(0, 0, -2, true, true, 0L, 36, 48, 32);
        NativeRhi.submitSectionSnapshotEx(0, 0, -3, true, false, NativeRhi.PORTAL_ALL_OPEN, 36, 84, 56);
        NativeRhi.submitSectionSnapshotEx(0, 0, 2, true, false, NativeRhi.PORTAL_ALL_OPEN, 36, 120, 80);

        int lastVisible = 0;
        int lastInstances = 0;
        int lastBatches = 0;
        for (int f = 0; f < 5; f++) {
            long ep = NativeRhi.beginFrame(1920, 1080);
            if (ep <= 0) throw new AssertionError("bad frame epoch: " + ep);

            lastVisible = NativeRhi.cullWorld(8f, 8f, 8f, 8f, 8f, -8f, 70f, 4f);
            if (lastVisible != 2 || NativeRhi.visibleSectionCount() != 2) {
                throw new AssertionError("expected 2 visible sections, got " + lastVisible);
            }
            int firstSz = NativeRhi.visibleSectionField(0, 2);
            if (firstSz != -1 && firstSz != -2) throw new AssertionError("unexpected visible section sz: " + firstSz);
            NativeRhi.glViewport(0, 0, 1920, 1080);
            NativeRhi.glClearColor(0.2f, 0.4f, 0.8f, 1.0f);

            NativeRhi.registerMesh(1, 36, 0, 0);
            NativeRhi.registerMesh(2, 72, 36, 24);
            NativeRhi.registerMesh(3, 48, 108, 60);
            NativeRhi.registerMesh(4, 6, 156, 90);

            // Visible non-block scene objects (Display entities, mob, chest, particle) + 2 culled (behind wall / behind cam)
            NativeRhi.pushInstance(NativeRhi.KIND_BLOCK_DISPLAY, 1, 8f, 8f, -2f, 1f, 1f, NativeRhi.BILLBOARD_FIXED, 1, -1);
            NativeRhi.pushInstance(NativeRhi.KIND_ITEM_DISPLAY, 1, 9f, 8f, -2f, 0.8f, 0.8f, NativeRhi.BILLBOARD_CENTER, 2, -1);
            NativeRhi.pushInstance(NativeRhi.KIND_TEXT_DISPLAY, 4, 7f, 9f, -2f, 1f, 1f, NativeRhi.BILLBOARD_VERTICAL, 3, -1);
            NativeRhi.pushInstance(NativeRhi.KIND_MOB, 2, 6f, 8f, -4f, 1f, 1.2f, NativeRhi.BILLBOARD_FIXED, 4, -1);
            NativeRhi.pushInstance(NativeRhi.KIND_CHEST, 3, 8f, 7f, -5f, 1f, 0.9f, NativeRhi.BILLBOARD_FIXED, 5, -1);
            NativeRhi.pushInstance(NativeRhi.KIND_PARTICLE, 4, 8f, 8.5f, -1f, 0.4f, 0.4f, NativeRhi.BILLBOARD_CENTER, 6, -1);
            NativeRhi.pushInstance(NativeRhi.KIND_MOB, 2, 8f, 8f, -40f, 1f, 1f, NativeRhi.BILLBOARD_FIXED, 7, -1);   // occluded behind solid wall
            NativeRhi.pushInstance(NativeRhi.KIND_PLAYER, 2, 8f, 8f, 25f, 1f, 1f, NativeRhi.BILLBOARD_FIXED, 8, -1); // frustum culled

            lastInstances = NativeRhi.flushInstances(8f, 8f, 8f, 8f, 8f, -8f, 70f, 2f);
            lastBatches = NativeRhi.lastDrawBatches();
            if (lastInstances != 6 || lastBatches != 4) {
                throw new AssertionError("expected 6 surviving instances in 4 batches, got " + lastInstances + "/" + lastBatches);
            }
            NativeRhi.endFrame();
        }

        System.out.println("JAVA_CLIENT_CORE_E2E_OK backend=" + backend + " vis=" + lastVisible + " inst=" + lastInstances + " batches=" + lastBatches + " dtMs=" + dtMs);
    }
}
