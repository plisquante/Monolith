// TLS spike: bir HTTPS GET, JSSE (çevrilmiş sun.security.ssl) üzerinden.
import java.io.InputStream;
import java.net.URI;
import java.net.http.HttpClient;
import java.net.http.HttpRequest;
import java.net.http.HttpResponse;

public class TlsProbe {
    public static void main(String[] args) throws Exception {
        String url = args.length > 0 ? args[0] : "https://example.com/";
        HttpClient c = HttpClient.newBuilder().connectTimeout(java.time.Duration.ofSeconds(20)).build();
        HttpResponse<String> r = c.send(HttpRequest.newBuilder(URI.create(url)).GET().build(),
                HttpResponse.BodyHandlers.ofString());
        System.out.println("status " + r.statusCode());
        System.out.println("bytes>0 " + (r.body().length() > 0));
        System.out.println("protocol " + r.version());
    }
}
