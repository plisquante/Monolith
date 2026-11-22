package org.lwjgl.system;

public interface SharedLibrary extends FunctionProvider, Pointer {
    String getName();
    String getPath();

    final class Default implements SharedLibrary {
        private final String name;
        private final String path;

        public Default(String name) {
            this(name, name);
        }

        public Default(String name, String path) {
            this.name = name;
            this.path = path;
        }

        @Override public String getName() { return name; }
        @Override public String getPath() { return path; }
        @Override public long address() { return 1L; }
        @Override public long getFunctionAddress(CharSequence functionName) { return 0L; }
    }
}
