import java.net.*;

public class HttpFetch {
    static void t(String u) {
        try {
            HttpURLConnection c = (HttpURLConnection) new URL(u).openConnection();
            c.setConnectTimeout(3000);
            c.setReadTimeout(3000);
            c.setRequestProperty("Accept", "application/json");
            System.out.println(u + " -> " + c.getResponseCode());
        } catch (Throwable e) {
            System.out.println(u + " -> " + e.getClass().getName());
        }
    }
    public static void main(String[] a) {
        t("http://127.0.0.1:1/x");
        t("https://127.0.0.1:1/x");
        t("http://nonexistent.invalid/x");
        t("https://nonexistent.invalid/x");
    }
}
