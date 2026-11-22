package java.io;

import java.nio.charset.Charset;
import java.util.Locale;

/**
 * monolith overlay of PrintStream: UTF-8 (or the requested charset) encoding done directly
 * through String.getBytes, no StreamEncoder / Charset service lookup.
 */
public class PrintStream extends FilterOutputStream implements Appendable, Closeable {
    private final boolean autoFlush;
    private boolean trouble = false;
    private boolean closing = false;
    private final String charsetName;
    private int csCode = -2;

    public PrintStream(OutputStream out) {
        this(out, false);
    }

    public PrintStream(OutputStream out, boolean autoFlush) {
        super(java.util.Objects.requireNonNull(out, "Null output stream"));
        this.autoFlush = autoFlush;
        this.charsetName = "UTF-8";
    }

    public PrintStream(OutputStream out, boolean autoFlush, String encoding) throws UnsupportedEncodingException {
        super(java.util.Objects.requireNonNull(out, "Null output stream"));
        this.autoFlush = autoFlush;
        "".getBytes(encoding);
        this.charsetName = encoding;
    }

    public PrintStream(OutputStream out, boolean autoFlush, Charset charset) {
        super(java.util.Objects.requireNonNull(out, "Null output stream"));
        this.autoFlush = autoFlush;
        this.charsetName = charset.name();
    }

    public PrintStream(String fileName) throws FileNotFoundException {
        this(new FileOutputStream(fileName), false);
    }

    public PrintStream(File file) throws FileNotFoundException {
        this(new FileOutputStream(file), false);
    }

    private void ensureOpen() throws IOException {
        if (out == null) throw new IOException("Stream closed");
    }

    public void flush() {
        synchronized (this) {
            try {
                ensureOpen();
                out.flush();
            } catch (IOException x) {
                trouble = true;
            }
        }
    }

    public void close() {
        synchronized (this) {
            if (!closing) {
                closing = true;
                try {
                    out.close();
                } catch (IOException x) {
                    trouble = true;
                }
                out = null;
            }
        }
    }

    public boolean checkError() {
        if (out != null) flush();
        return trouble;
    }

    protected void setError() {
        trouble = true;
    }

    protected void clearError() {
        trouble = false;
    }

    public void write(int b) {
        try {
            synchronized (this) {
                ensureOpen();
                out.write(b);
                if ((b == '\n') && autoFlush) out.flush();
            }
        } catch (InterruptedIOException x) {
            Thread.currentThread().interrupt();
        } catch (IOException x) {
            trouble = true;
        }
    }

    public void write(byte[] buf, int off, int len) {
        try {
            synchronized (this) {
                ensureOpen();
                out.write(buf, off, len);
                if (autoFlush) out.flush();
            }
        } catch (InterruptedIOException x) {
            Thread.currentThread().interrupt();
        } catch (IOException x) {
            trouble = true;
        }
    }

    public void write(byte[] buf) throws IOException {
        this.write(buf, 0, buf.length);
    }

    public void writeBytes(byte[] buf) {
        this.write(buf, 0, buf.length);
    }

    private void writeString(String s) {
        int cs = csCode;
        if (cs == -2) csCode = cs = String.monoCharsetCode(charsetName);
        byte[] b = cs < 0 ? s.getBytes() : String.monoEncode(cs, s);
        try {
            synchronized (this) {
                ensureOpen();
                out.write(b, 0, b.length);
                if (autoFlush && (s.indexOf('\n') >= 0)) out.flush();
            }
        } catch (InterruptedIOException x) {
            Thread.currentThread().interrupt();
        } catch (IOException x) {
            trouble = true;
        }
    }

    private void newLine() {
        writeString(System.lineSeparator());
    }

    public void print(boolean b) { writeString(String.valueOf(b)); }
    public void print(char c) { writeString(String.valueOf(c)); }
    public void print(int i) { writeString(String.valueOf(i)); }
    public void print(long l) { writeString(String.valueOf(l)); }
    public void print(float f) { writeString(String.valueOf(f)); }
    public void print(double d) { writeString(String.valueOf(d)); }
    public void print(char[] s) { writeString(new String(s)); }
    public void print(String s) { writeString(String.valueOf(s)); }
    public void print(Object obj) { writeString(String.valueOf(obj)); }

    public void println() { newLine(); }
    public void println(boolean x) { println(String.valueOf(x)); }
    public void println(char x) { println(String.valueOf(x)); }
    public void println(int x) { println(String.valueOf(x)); }
    public void println(long x) { println(String.valueOf(x)); }
    public void println(float x) { println(String.valueOf(x)); }
    public void println(double x) { println(String.valueOf(x)); }
    public void println(char[] x) { println(new String(x)); }

    public void println(String x) {
        synchronized (this) {
            writeString(String.valueOf(x).concat(System.lineSeparator()));
        }
    }

    public void println(Object x) {
        println(String.valueOf(x));
    }

    public PrintStream printf(String format, Object... args) {
        return format(format, args);
    }

    public PrintStream printf(Locale l, String format, Object... args) {
        return format(l, format, args);
    }

    public PrintStream format(String format, Object... args) {
        writeString(String.format(format, args));
        return this;
    }

    public PrintStream format(Locale l, String format, Object... args) {
        writeString(String.format(l, format, args));
        return this;
    }

    public PrintStream append(CharSequence csq) {
        print(String.valueOf(csq));
        return this;
    }

    public PrintStream append(CharSequence csq, int start, int end) {
        if (csq == null) csq = "null";
        return append(csq.subSequence(start, end));
    }

    public PrintStream append(char c) {
        print(c);
        return this;
    }

    public Charset charset() {
        return Charset.forName(charsetName);
    }
}
