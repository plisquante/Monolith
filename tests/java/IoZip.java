import java.io.*;
import java.nio.charset.StandardCharsets;
import java.util.*;
import java.util.zip.*;

public class IoZip {
    public static void main(String[] args) throws Exception {
        File dir = new File(System.getProperty("java.io.tmpdir"), "j2z-iozip-" + ProcessHandleLess.pid());
        dir.mkdirs();
        File f = new File(dir, "data.txt");
        try (Writer w = new OutputStreamWriter(new FileOutputStream(f), StandardCharsets.UTF_8)) {
            for (int i = 0; i < 1000; i++) w.write("line " + i + " çğü €\n");
        }
        System.out.println(f.exists() + " " + f.isFile() + " " + f.length() + " " + dir.isDirectory() + " " + f.getName());
        int lines = 0; long sum = 0;
        try (BufferedReader r = new BufferedReader(new InputStreamReader(new FileInputStream(f), StandardCharsets.UTF_8))) {
            String line;
            while ((line = r.readLine()) != null) { lines++; sum += line.hashCode(); }
        }
        System.out.println(lines + " " + sum);
        try (FileInputStream in = new FileInputStream(new File(dir, "missing.txt"))) {
            System.out.println("opened?");
        } catch (FileNotFoundException e) {
            System.out.println("FNF " + e.getMessage().endsWith("(No such file or directory)"));
        }
        byte[] raw = new byte[200000];
        Random rnd = new Random(1);
        for (int i = 0; i < raw.length; i++) raw[i] = (byte) ("abcabcabd".charAt(i % 9) + (rnd.nextInt(50) == 0 ? 1 : 0));
        CRC32 crc = new CRC32(); crc.update(raw);
        Adler32 ad = new Adler32(); ad.update(raw, 10, 5000);
        System.out.println("crc " + Long.toHexString(crc.getValue()) + " adler " + Long.toHexString(ad.getValue()));
        ByteArrayOutputStream bos = new ByteArrayOutputStream();
        try (GZIPOutputStream gz = new GZIPOutputStream(bos)) { gz.write(raw); }
        byte[] packed = bos.toByteArray();
        System.out.println("gzip " + packed.length + " " + Arrays.hashCode(packed));
        ByteArrayOutputStream back = new ByteArrayOutputStream();
        try (GZIPInputStream gin = new GZIPInputStream(new ByteArrayInputStream(packed))) { gin.transferTo(back); }
        System.out.println("roundtrip " + Arrays.equals(raw, back.toByteArray()));
        Deflater d = new Deflater(Deflater.BEST_COMPRESSION, true);
        d.setInput(raw); d.finish();
        byte[] buf = new byte[300000]; int n = d.deflate(buf); d.end();
        Inflater inf = new Inflater(true); inf.setInput(buf, 0, n);
        byte[] out = new byte[raw.length]; int m = inf.inflate(out); inf.end();
        System.out.println("raw deflate " + n + " inflate " + m + " " + Arrays.equals(raw, out));
        File zf = new File(dir, "a.zip");
        try (ZipOutputStream zo = new ZipOutputStream(new FileOutputStream(zf))) {
            for (int i = 0; i < 3; i++) { ZipEntry e = new ZipEntry("e" + i + ".txt"); e.setTime(0); zo.putNextEntry(e); zo.write(("entry " + i).repeat(100).getBytes()); zo.closeEntry(); }
        }
        try (ZipInputStream zi = new ZipInputStream(new FileInputStream(zf))) {
            ZipEntry e;
            while ((e = zi.getNextEntry()) != null) System.out.print(e.getName() + "=" + new String(zi.readAllBytes()).length() + " ");
        }
        System.out.println();
        try (RandomAccessFile raf = new RandomAccessFile(new File(dir, "raf.bin"), "rw")) {
            raf.writeInt(42); raf.writeLong(-7); raf.writeUTF("hé"); raf.seek(4);
            System.out.println("raf " + raf.readLong() + " " + raf.readUTF() + " " + raf.length() + " " + raf.getFilePointer());
        }
        String[] names = dir.list(); Arrays.sort(names);
        System.out.println(Arrays.toString(names));
        for (File x : dir.listFiles()) x.delete();
        System.out.println("deleted " + dir.delete() + " " + dir.exists());
    }
}

class ProcessHandleLess {
    static long pid() { return Math.abs(System.nanoTime() % 100000); }
}
