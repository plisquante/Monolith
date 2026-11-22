import java.nio.ByteBuffer;
import java.util.zip.CRC32;
import java.util.zip.Adler32;

public class CrcTest {
    public static void main(String[] args) {
        CRC32 c = new CRC32();
        c.update("123456789".getBytes());
        System.out.println("check " + Long.toHexString(c.getValue()));           // 0xcbf43926
        CRC32 b = new CRC32();
        for (int i = 0; i < 256; i++) b.update(i);
        System.out.println("bytes " + Long.toHexString(b.getValue()));
        byte[] big = new byte[100_003];
        for (int i = 0; i < big.length; i++) big[i] = (byte) (i * 31 + 7);
        CRC32 d = new CRC32();
        d.update(big, 3, 99_000);
        d.update(big, 99_003, 1000);
        System.out.println("slices " + Long.toHexString(d.getValue()));
        CRC32 e = new CRC32();
        ByteBuffer bb = ByteBuffer.allocateDirect(4096);
        for (int i = 0; i < 4096; i++) bb.put((byte) (i ^ 0x5a));
        bb.flip();
        e.update(bb);
        System.out.println("direct " + Long.toHexString(e.getValue()));
        CRC32 f = new CRC32();
        f.update(new byte[0], 0, 0);
        f.update(-1);
        System.out.println("neg " + Long.toHexString(f.getValue()) + " " + Long.toHexString(new CRC32().getValue()));
        Adler32 a = new Adler32();
        a.update("Wikipedia".getBytes());
        System.out.println("adler " + Long.toHexString(a.getValue()));       // 0x11e60398
    }
}
