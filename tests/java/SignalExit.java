// SIGTERM must run the shutdown hooks and end the process with status 143 (128 + 15), as on HotSpot. Minecraft saves the world
// in its shutdown hook, so a `kill` or a system shutdown that skipped the hooks would lose the session.
public class SignalExit {
    public static void main(String[] args) throws Exception {
        Runtime.getRuntime().addShutdownHook(new Thread(new Runnable() {
            @Override
            public void run() {
                System.out.println("shutdown hook ran");
                System.out.flush();
            }
        }));
        System.out.println("ready");
        System.out.flush();
        long pid = ProcessHandle.current().pid();
        Process p = new ProcessBuilder("kill", "-TERM", Long.toString(pid)).inheritIO().start();
        p.waitFor(); // prints nothing: the signal may end the program before this returns
        Thread.sleep(20000); // the signal ends the program long before this returns
        System.out.println("not reached");
    }
}
