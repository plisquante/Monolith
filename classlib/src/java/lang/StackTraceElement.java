package java.lang;

/** j2zig overlay of StackTraceElement. */
public final class StackTraceElement implements java.io.Serializable {
    private final String declaringClass;
    private final String methodName;
    private final String fileName;
    private final int lineNumber;

    public StackTraceElement(String declaringClass, String methodName, String fileName, int lineNumber) {
        this.declaringClass = java.util.Objects.requireNonNull(declaringClass, "Declaring class is null");
        this.methodName = java.util.Objects.requireNonNull(methodName, "Method name is null");
        this.fileName = fileName;
        this.lineNumber = lineNumber;
    }

    private String classLoaderName, moduleName, moduleVersion;

    public StackTraceElement(String classLoaderName, String moduleName, String moduleVersion,
                             String declaringClass, String methodName, String fileName, int lineNumber) {
        this(declaringClass, methodName, fileName, lineNumber);
        this.classLoaderName = classLoaderName;
        this.moduleName = moduleName;
        this.moduleVersion = moduleVersion;
    }

    public String getClassLoaderName() { return classLoaderName; }
    public String getModuleName() { return moduleName; }
    public String getModuleVersion() { return moduleVersion; }

    public String getFileName() { return fileName; }
    public int getLineNumber() { return lineNumber; }
    public String getClassName() { return declaringClass; }
    public String getMethodName() { return methodName; }
    public boolean isNativeMethod() { return lineNumber == -2; }

    public String toString() {
        String loc;
        if (lineNumber == -2) loc = "Native Method";
        else if (fileName == null) loc = "Unknown Source";
        else if (lineNumber >= 0) loc = fileName + ":" + lineNumber;
        else loc = fileName;
        return declaringClass + "." + methodName + "(" + loc + ")";
    }

    public boolean equals(Object obj) {
        if (obj == this) return true;
        if (!(obj instanceof StackTraceElement e)) return false;
        return e.declaringClass.equals(declaringClass) && e.lineNumber == lineNumber
            && java.util.Objects.equals(methodName, e.methodName) && java.util.Objects.equals(fileName, e.fileName);
    }

    public int hashCode() {
        return 31 * (31 * declaringClass.hashCode() + methodName.hashCode()) + java.util.Objects.hashCode(fileName) * 31 + lineNumber;
    }
}
