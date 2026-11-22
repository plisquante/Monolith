package org.lwjgl.system;

/** Only the class shape Minecraft's DebugMemoryUntracker reflects on; allocation tracking is not part of the no-LWJGL runtime. */
final class MemoryManage {
    private MemoryManage() {}

    static final class DebugAllocator implements MemoryUtil.MemoryAllocator {
        @Override public long malloc(long size) { throw new UnsupportedOperationException("LWJGL debug allocator is not available"); }
        @Override public long realloc(long ptr, long size) { throw new UnsupportedOperationException("LWJGL debug allocator is not available"); }
        @Override public void free(long ptr) { throw new UnsupportedOperationException("LWJGL debug allocator is not available"); }

        static void untrack(long address) {
            // never reached: the active allocator is never a DebugAllocator, so the caller's handle is null
        }
    }
}
