package java.lang;

/**
 * SIGHUP, SIGINT and SIGTERM end the program like {@code System.exit(128 + signal)}: the shutdown hooks run first, as on
 * HotSpot (Minecraft saves the world in its hook). The runtime turns a signal into a number on a pipe
 * (monolith/rt/signals.rs); this class runs a daemon thread that waits for the number and calls {@code Runtime.exit}.
 * Started by the first {@code Runtime.addShutdownHook}: until a hook exists the default action (end the process) is the
 * same thing. Where the runtime has no signal support {@code install0} returns false and nothing is started.
 */
final class MonoSignals {
    private static boolean started;

    private MonoSignals() {}

    static synchronized void install() {
        if (started) return;
        started = true;
        if (!install0()) return;
        Thread t = new Thread(null, new Runnable() {
            @Override
            public void run() {
                for (;;) {
                    int sig = await0();
                    if (sig <= 0) return;
                    Runtime.getRuntime().exit(128 + sig);
                }
            }
        }, "Signal Dispatcher", 0);
        t.setDaemon(true);
        t.start();
    }

    private static native boolean install0();

    private static native int await0();
}
