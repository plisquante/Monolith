package java.lang;

import java.io.PrintStream;
import java.io.PrintWriter;
import java.util.ArrayList;
import java.util.List;

/** monolith overlay of Throwable. The backtrace is captured from the runtime shadow stack. */
public class Throwable implements java.io.Serializable {
    private static final long serialVersionUID = -3042686055658047285L;
    private static final StackTraceElement[] EMPTY = new StackTraceElement[0];
    private String detailMessage;
    private Throwable cause = this;
    private transient Object backtrace;
    private StackTraceElement[] stackTrace;
    private List<Throwable> suppressedExceptions;
    private boolean writableStackTrace = true;
    private boolean enableSuppression = true;
    /** JDK 22+: read by Error/Exception constructors (JFR throwable tracing, never enabled here). */
    static boolean jfrTracing;

    public Throwable() {
        fillInStackTrace();
    }

    public Throwable(String message) {
        fillInStackTrace();
        detailMessage = message;
    }

    public Throwable(String message, Throwable cause) {
        fillInStackTrace();
        detailMessage = message;
        this.cause = cause;
    }

    public Throwable(Throwable cause) {
        fillInStackTrace();
        detailMessage = (cause == null ? null : cause.toString());
        this.cause = cause;
    }

    protected Throwable(String message, Throwable cause, boolean enableSuppression, boolean writableStackTrace) {
        this.writableStackTrace = writableStackTrace;
        if (writableStackTrace) fillInStackTrace();
        detailMessage = message;
        this.cause = cause;
        this.enableSuppression = enableSuppression;
    }

    /** Runtime hook: NoClassDefFoundError for a class whose initializer failed earlier (JDK 21 format). */
    static Throwable monoNcdfe(Class<?> c, Throwable original, Thread t) {
        NoClassDefFoundError e = new NoClassDefFoundError("Could not initialize class " + c.getName());
        if (original != null) {
            ExceptionInInitializerError x = new ExceptionInInitializerError("Exception " + original
                    + " [in thread \"" + (t == null ? "main" : t.getName()) + "\"]");
            x.setStackTrace(original.getStackTrace());
            e.initCause(x);
        }
        return e;
    }

    /** package-private JDK helper (JavaLangAccess.setCause) */
    final void setCause(Throwable t) {
        this.cause = t;
    }

    public String getMessage() {
        return detailMessage;
    }

    public String getLocalizedMessage() {
        return getMessage();
    }

    public synchronized Throwable getCause() {
        return (cause == this ? null : cause);
    }

    public synchronized Throwable initCause(Throwable cause) {
        if (this.cause != this)
            throw new IllegalStateException("Can't overwrite cause with " + String.valueOf(cause), this);
        if (cause == this) throw new IllegalArgumentException("Self-causation not permitted", this);
        this.cause = cause;
        return this;
    }

    public String toString() {
        String s = getClass().getName();
        String message = getLocalizedMessage();
        return (message != null) ? (s + ": " + message) : s;
    }

    public void printStackTrace() {
        printStackTrace(System.err);
    }

    public void printStackTrace(PrintStream s) {
        StringBuilder sb = new StringBuilder();
        render(sb, "", "", new ArrayList<>());
        s.print(sb.toString());
    }

    public void printStackTrace(PrintWriter s) {
        StringBuilder sb = new StringBuilder();
        render(sb, "", "", new ArrayList<>());
        s.print(sb.toString());
    }

    private void render(StringBuilder sb, String caption, String prefix, List<Throwable> seen) {
        for (Throwable t : seen) {
            if (t == this) {
                sb.append(prefix).append(caption).append("[CIRCULAR REFERENCE: ").append(this).append("]\n");
                return;
            }
        }
        seen.add(this);
        sb.append(prefix).append(caption).append(this).append('\n');
        for (StackTraceElement e : getOurStackTrace()) sb.append(prefix).append("\tat ").append(e).append('\n');
        if (suppressedExceptions != null)
            for (Throwable se : suppressedExceptions) se.render(sb, "Suppressed: ", prefix + "\t", seen);
        Throwable c = getCause();
        if (c != null) c.render(sb, "Caused by: ", prefix, seen);
    }

    public synchronized Throwable fillInStackTrace() {
        if (writableStackTrace) {
            backtrace = backtrace0();
            stackTrace = null;
        }
        return this;
    }

    /** Returns String[] with (class, method, file, line) quadruples, innermost first, skipping exception construction frames. */
    private native Object backtrace0();

    public StackTraceElement[] getStackTrace() {
        return getOurStackTrace().clone();
    }

    private synchronized StackTraceElement[] getOurStackTrace() {
        if (stackTrace == null) {
            if (backtrace == null) return EMPTY;
            String[] bt = (String[]) backtrace;
            StackTraceElement[] st = new StackTraceElement[bt.length / 4];
            for (int i = 0; i < st.length; i++) {
                String file = bt[4 * i + 2];
                st[i] = new StackTraceElement(bt[4 * i], bt[4 * i + 1], file.isEmpty() ? null : file,
                        Integer.parseInt(bt[4 * i + 3]));
            }
            stackTrace = st;
        }
        return stackTrace;
    }

    public void setStackTrace(StackTraceElement[] stackTrace) {
        StackTraceElement[] copy = stackTrace.clone();
        for (int i = 0; i < copy.length; i++)
            if (copy[i] == null) throw new NullPointerException("stackTrace[" + i + "]");
        synchronized (this) {
            this.stackTrace = copy;
        }
    }

    public final synchronized void addSuppressed(Throwable exception) {
        if (exception == this) throw new IllegalArgumentException("Self-suppression not permitted", exception);
        if (exception == null) throw new NullPointerException("Cannot suppress a null exception.");
        if (!enableSuppression) return;
        if (suppressedExceptions == null) suppressedExceptions = new ArrayList<>(1);
        suppressedExceptions.add(exception);
    }

    public final synchronized Throwable[] getSuppressed() {
        if (suppressedExceptions == null) return new Throwable[0];
        return suppressedExceptions.toArray(new Throwable[0]);
    }
}
