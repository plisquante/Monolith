import java.io.*;
import java.lang.reflect.Array;
import java.util.*;
import java.util.concurrent.atomic.*;
import java.util.zip.*;

public class IoZipTest {
    public static void main(String[] args) throws Exception {
        File dir = new File("/tmp/mono_iozip_test_" + args.length);
        deleteRec(dir);
        dir.mkdir();

        File f1 = new File(dir, "hello.txt");
        boolean created = f1.createNewFile();
        try (FileOutputStream fos = new FileOutputStream(f1)) {
            fos.write("Hello ".getBytes("UTF-8"));
        }
        try (FileOutputStream fos = new FileOutputStream(f1, true)) {
            fos.write("Monolith!".getBytes("UTF-8"));
        }
        byte[] readBuf = new byte[64];
        int nr;
        try (FileInputStream fis = new FileInputStream(f1)) {
            long skipped = fis.skip(6);
            int av = fis.available();
            nr = fis.read(readBuf);
            System.out.println("fis " + created + " " + skipped + " " + (av >= 9) + " " + new String(readBuf, 0, nr, "UTF-8"));
        }

        File f2 = new File(dir, "raf.bin");
        try (RandomAccessFile raf = new RandomAccessFile(f2, "rw")) {
            raf.writeUTF("MC26.3");
            raf.writeInt(123456789);
            raf.seek(0);
            String s = raf.readUTF();
            int iv = raf.readInt();
            raf.setLength(100);
            long len = raf.length();
            System.out.println("raf " + s + " " + iv + " " + len);
        }

        File f3 = new File(dir, "renamed.bin");
        f2.renameTo(f3);
        String[] list = dir.list();
        Arrays.sort(list);
        System.out.println("fs " + f3.isFile() + " " + dir.isDirectory() + " " + Arrays.toString(list));

        // Zip + CRC32 + Adler32
        CRC32 crc = new CRC32();
        Adler32 adl = new Adler32();
        byte[] payload = "Minecraft 26.3 pure Rust native runtime benchmark payload! ".repeat(50).getBytes("UTF-8");
        crc.update(payload, 0, payload.length);
        adl.update(payload, 0, payload.length);

        ByteArrayOutputStream baos = new ByteArrayOutputStream();
        try (ZipOutputStream zos = new ZipOutputStream(baos)) {
            zos.putNextEntry(new ZipEntry("world/level.dat"));
            zos.write(payload);
            zos.closeEntry();
            zos.putNextEntry(new ZipEntry("assets/pack.mcmeta"));
            zos.write("{\"pack\":26}".getBytes("UTF-8"));
            zos.closeEntry();
        }
        byte[] zipBytes = baos.toByteArray();
        List<String> znames = new ArrayList<>();
        ByteArrayOutputStream unz = new ByteArrayOutputStream();
        try (ZipInputStream zis = new ZipInputStream(new ByteArrayInputStream(zipBytes))) {
            ZipEntry ze;
            while ((ze = zis.getNextEntry()) != null) {
                znames.add(ze.getName());
                if (ze.getName().equals("world/level.dat")) {
                    byte[] buf = new byte[512];
                    int k;
                    while ((k = zis.read(buf)) > 0) unz.write(buf, 0, k);
                }
            }
        }
        System.out.println("zip " + Long.toHexString(crc.getValue()) + " " + Long.toHexString(adl.getValue())
                + " " + znames + " eq=" + Arrays.equals(payload, unz.toByteArray()));

        // MultiDim reflection array + Atomics + TimeZone
        int[][] m = (int[][]) Array.newInstance(int.class, 2, 3);
        m[1][2] = 777;
        AtomicBoolean ab = new AtomicBoolean(false);
        boolean c1 = ab.compareAndSet(false, true);
        AtomicReference<String> ar = new AtomicReference<>("a");
        boolean c2 = ar.compareAndSet("a", "b");
        System.out.println("misc " + m.length + "x" + m[0].length + "=" + m[1][2] + " " + c1 + " " + ab.get() + " " + c2 + " " + ar.get());

        deleteRec(dir);
    }

    private static void deleteRec(File f) {
        if (f.isDirectory()) {
            File[] fs = f.listFiles();
            if (fs != null) for (File c : fs) deleteRec(c);
        }
        f.delete();
    }
}
