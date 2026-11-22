package com.monolith.touchcore;

import org.json.JSONArray;
import org.json.JSONObject;

import java.io.ByteArrayOutputStream;
import java.io.File;
import java.io.FileInputStream;
import java.io.FileOutputStream;
import java.io.InputStream;
import java.net.HttpURLConnection;
import java.net.URL;
import java.nio.charset.StandardCharsets;
import java.security.MessageDigest;
import java.util.ArrayList;
import java.util.Iterator;
import java.util.List;
import java.util.concurrent.Callable;
import java.util.concurrent.ExecutorService;
import java.util.concurrent.Executors;
import java.util.concurrent.Future;
import java.util.concurrent.atomic.AtomicInteger;

/**
 * Official Mojang piston-meta version manifest + client.jar + libraries + asset index/objects
 * downloader with SHA-1 verification and bounded concurrency.
 */
public final class MojangDownloader {
    public static final String MANIFEST_URL =
            "https://piston-meta.mojang.com/mc/game/version_manifest_v2.json";
    private static final String RESOURCES_BASE =
            "https://resources.download.minecraft.net/";

    public static final class VersionEntry {
        public final String id;
        public final String type;
        public final String url;

        public VersionEntry(String id, String type, String url) {
            this.id = id;
            this.type = type;
            this.url = url;
        }

        @Override
        public String toString() {
            return id + " (" + type + ")";
        }
    }

    public interface ProgressListener {
        void onStatus(String message);
    }

    private static final class DownloadItem {
        final String url;
        final File dst;
        final String sha1;
        final long size;
        DownloadItem(String url, File dst, String sha1, long size) {
            this.url = url;
            this.dst = dst;
            this.sha1 = sha1;
            this.size = size;
        }
    }

    private MojangDownloader() {}

    public static List<VersionEntry> fetchReleases(int maxCount) throws Exception {
        String raw = downloadText(MANIFEST_URL);
        JSONObject root = new JSONObject(raw);
        JSONArray versions = root.getJSONArray("versions");
        List<VersionEntry> out = new ArrayList<VersionEntry>();
        for (int i = 0; i < versions.length() && out.size() < maxCount; i++) {
            JSONObject v = versions.getJSONObject(i);
            String type = v.optString("type", "");
            if ("release".equals(type) || out.size() < 4) {
                out.add(new VersionEntry(
                        v.getString("id"),
                        type,
                        v.getString("url")));
            }
        }
        return out;
    }

    public static File downloadClientAndMetadata(
            VersionEntry ver,
            File versionsRoot,
            File assetsRoot,
            ProgressListener listener) throws Exception {
        File librariesRoot = new File(versionsRoot.getParentFile(), "libraries");
        return downloadClientAndMetadata(ver, versionsRoot, librariesRoot, assetsRoot, true, listener);
    }

    public static File downloadClientAndMetadata(
            VersionEntry ver,
            File versionsRoot,
            File librariesRoot,
            File assetsRoot,
            boolean downloadAssetObjects,
            ProgressListener listener) throws Exception {
        File verDir = new File(versionsRoot, ver.id);
        verDir.mkdirs();

        if (listener != null) listener.onStatus("Sürüm bilgisi indiriliyor: " + ver.id);
        String metaRaw = downloadText(ver.url);
        File metaFile = new File(verDir, ver.id + ".json");
        writeString(metaFile, metaRaw);

        JSONObject meta = new JSONObject(metaRaw);
        JSONObject downloads = meta.getJSONObject("downloads");
        JSONObject client = downloads.getJSONObject("client");
        String clientUrl = client.getString("url");
        String clientSha1 = client.optString("sha1", "");
        long expectedSize = client.optLong("size", 0L);

        File clientJar = new File(verDir, "client.jar");
        if (!verifyFile(clientJar, clientSha1, expectedSize)) {
            if (listener != null) listener.onStatus("client.jar indiriliyor (" + (expectedSize / (1024 * 1024)) + " MB)...");
            downloadFileVerified(clientUrl, clientJar, clientSha1, expectedSize);
        }

        // Download non-native Java libraries (for completeness & desktop/build parity)
        JSONArray libraries = meta.optJSONArray("libraries");
        if (libraries != null && librariesRoot != null) {
            List<DownloadItem> libQueue = new ArrayList<DownloadItem>();
            for (int i = 0; i < libraries.length(); i++) {
                JSONObject lib = libraries.getJSONObject(i);
                String name = lib.optString("name", "");
                if (name.contains(":natives-") || !rulesAllowLinux(lib.optJSONArray("rules"))) {
                    continue;
                }
                JSONObject libDl = lib.optJSONObject("downloads");
                if (libDl == null) continue;
                JSONObject art = libDl.optJSONObject("artifact");
                if (art == null) continue;
                String path = art.optString("path", "");
                String url = art.optString("url", "");
                if (path.isEmpty() || url.isEmpty()) continue;
                File dst = new File(librariesRoot, path);
                String sha1 = art.optString("sha1", "");
                long sz = art.optLong("size", 0L);
                if (!verifyFile(dst, sha1, sz)) {
                    libQueue.add(new DownloadItem(url, dst, sha1, sz));
                }
            }
            if (!libQueue.isEmpty()) {
                downloadBatch(libQueue, "Kütüphane", listener);
            }
        }

        JSONObject assetIdx = meta.optJSONObject("assetIndex");
        if (assetIdx != null) {
            File idxDir = new File(assetsRoot, "indexes");
            idxDir.mkdirs();
            String idxId = assetIdx.getString("id");
            String idxSha1 = assetIdx.optString("sha1", "");
            long idxSize = assetIdx.optLong("size", 0L);
            File idxFile = new File(idxDir, idxId + ".json");
            if (!verifyFile(idxFile, idxSha1, idxSize)) {
                if (listener != null) listener.onStatus("Asset indeksi indiriliyor (" + idxId + ")...");
                downloadFileVerified(assetIdx.getString("url"), idxFile, idxSha1, idxSize);
            }
            if (downloadAssetObjects && idxFile.isFile()) {
                JSONObject idxJson = new JSONObject(readString(idxFile));
                JSONObject objects = idxJson.optJSONObject("objects");
                if (objects != null) {
                    File objectsDir = new File(assetsRoot, "objects");
                    List<DownloadItem> objQueue = new ArrayList<DownloadItem>();
                    Iterator<String> keys = objects.keys();
                    while (keys.hasNext()) {
                        String key = keys.next();
                        JSONObject entry = objects.optJSONObject(key);
                        if (entry == null) continue;
                        String hash = entry.optString("hash", "");
                        long sz = entry.optLong("size", 0L);
                        if (hash.length() < 2) continue;
                        String prefix = hash.substring(0, 2);
                        File dst = new File(new File(objectsDir, prefix), hash);
                        if (!verifyFile(dst, hash, sz)) {
                            objQueue.add(new DownloadItem(RESOURCES_BASE + prefix + "/" + hash, dst, hash, sz));
                        }
                    }
                    if (!objQueue.isEmpty()) {
                        downloadBatch(objQueue, "Asset", listener);
                    }
                }
            }
        }
        return clientJar;
    }

    private static boolean rulesAllowLinux(JSONArray rules) {
        if (rules == null || rules.length() == 0) return true;
        boolean allow = false;
        for (int i = 0; i < rules.length(); i++) {
            JSONObject r = rules.optJSONObject(i);
            if (r == null) continue;
            JSONObject os = r.optJSONObject("os");
            boolean matches = (os == null);
            if (os != null) {
                String osName = os.optString("name", "");
                matches = osName.isEmpty() || "linux".equals(osName);
            }
            if (matches) {
                allow = "allow".equals(r.optString("action", "allow"));
            }
        }
        return allow;
    }

    private static void downloadBatch(
            final List<DownloadItem> items,
            final String label,
            final ProgressListener listener) throws Exception {
        final int total = items.size();
        final AtomicInteger done = new AtomicInteger(0);
        ExecutorService pool = Executors.newFixedThreadPool(4);
        List<Future<Void>> futures = new ArrayList<Future<Void>>(total);
        try {
            for (final DownloadItem item : items) {
                futures.add(pool.submit(new Callable<Void>() {
                    @Override public Void call() throws Exception {
                        downloadFileVerified(item.url, item.dst, item.sha1, item.size);
                        int cur = done.incrementAndGet();
                        if (listener != null && (cur == 1 || cur == total || (cur % 25) == 0)) {
                            listener.onStatus(label + " indiriliyor: " + cur + " / " + total);
                        }
                        return null;
                    }
                }));
            }
            for (Future<Void> f : futures) {
                f.get();
            }
        } finally {
            pool.shutdownNow();
        }
    }

    private static boolean verifyFile(File f, String expectedSha1, long expectedSize) {
        if (!f.isFile()) return false;
        if (expectedSize > 0 && f.length() != expectedSize) return false;
        if (expectedSha1 == null || expectedSha1.isEmpty()) return true;
        try {
            MessageDigest md = MessageDigest.getInstance("SHA-1");
            FileInputStream in = new FileInputStream(f);
            byte[] buf = new byte[32768];
            int n;
            while ((n = in.read(buf)) > 0) md.update(buf, 0, n);
            in.close();
            byte[] dig = md.digest();
            StringBuilder sb = new StringBuilder(40);
            for (byte b : dig) {
                sb.append(Character.forDigit((b >>> 4) & 0xF, 16));
                sb.append(Character.forDigit(b & 0xF, 16));
            }
            return expectedSha1.equalsIgnoreCase(sb.toString());
        } catch (Throwable ignored) {
            return false;
        }
    }

    private static void downloadFileVerified(String urlStr, File dst, String sha1, long size) throws Exception {
        File parent = dst.getParentFile();
        if (parent != null) parent.mkdirs();
        Exception lastErr = null;
        for (int attempt = 0; attempt < 2; attempt++) {
            try {
                downloadFile(urlStr, dst);
                if (verifyFile(dst, sha1, size)) return;
                dst.delete();
                lastErr = new Exception("SHA-1/boyut doğrulama hatası: " + dst.getName());
            } catch (Exception e) {
                lastErr = e;
            }
        }
        throw lastErr;
    }

    private static String downloadText(String urlStr) throws Exception {
        HttpURLConnection c = (HttpURLConnection) new URL(urlStr).openConnection();
        c.setConnectTimeout(12000);
        c.setReadTimeout(20000);
        c.setRequestProperty("User-Agent", "TouchCore-MONOLITH/2.0");
        InputStream in = c.getInputStream();
        ByteArrayOutputStream bos = new ByteArrayOutputStream();
        byte[] buf = new byte[16384];
        int n;
        while ((n = in.read(buf)) > 0) bos.write(buf, 0, n);
        in.close();
        c.disconnect();
        return new String(bos.toByteArray(), StandardCharsets.UTF_8);
    }

    private static void downloadFile(String urlStr, File dst) throws Exception {
        File tmp = new File(dst.getParentFile(), dst.getName() + ".part." + Thread.currentThread().getId());
        HttpURLConnection c = (HttpURLConnection) new URL(urlStr).openConnection();
        c.setConnectTimeout(12000);
        c.setReadTimeout(30000);
        c.setRequestProperty("User-Agent", "TouchCore-MONOLITH/2.0");
        InputStream in = c.getInputStream();
        FileOutputStream out = new FileOutputStream(tmp);
        byte[] buf = new byte[32768];
        int n;
        while ((n = in.read(buf)) > 0) out.write(buf, 0, n);
        out.close();
        in.close();
        c.disconnect();
        if (dst.exists()) dst.delete();
        if (!tmp.renameTo(dst)) {
            tmp.delete();
            throw new Exception("Dosya taşınamadı: " + dst.getAbsolutePath());
        }
    }

    private static String readString(File src) throws Exception {
        FileInputStream in = new FileInputStream(src);
        ByteArrayOutputStream bos = new ByteArrayOutputStream();
        byte[] buf = new byte[16384];
        int n;
        while ((n = in.read(buf)) > 0) bos.write(buf, 0, n);
        in.close();
        return new String(bos.toByteArray(), StandardCharsets.UTF_8);
    }

    private static void writeString(File dst, String content) throws Exception {
        FileOutputStream out = new FileOutputStream(dst);
        out.write(content.getBytes(StandardCharsets.UTF_8));
        out.close();
    }
}
