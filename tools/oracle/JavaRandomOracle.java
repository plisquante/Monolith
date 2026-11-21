// Referans çıktı üretici: java.util.Random'un (JDK) sonuçlarını sabit bir fixture olarak yazar.
// Kullanım (depo kökünden):
//   javac -d /tmp/oracle tools/oracle/JavaRandomOracle.java
//   java -cp /tmp/oracle JavaRandomOracle > crates/mono-core/tests/fixtures/java_random.txt
// Satır biçimi: <seed> <op> <arg> <sonuç>
//   I  : nextInt()          sonuç = int
//   B  : nextInt(bound)     arg = bound
//   L  : nextLong()         sonuç = long
//   Z  : nextBoolean()      sonuç = 0|1
//   F  : nextFloat()        sonuç = float'ın ham bit değeri (int)
//   D  : nextDouble()       sonuç = double'ın ham bit değeri (long)
// Ham bit değeri kullanılır, böylece kayan nokta karşılaştırması bit-birebir olur.
import java.util.Random;

public class JavaRandomOracle {
    public static void main(String[] args) {
        long[] seeds = {0L, 1L, 42L, -1L, Long.MIN_VALUE, Long.MAX_VALUE, 123456789012345L, -987654321L};
        int[] bounds = {1, 2, 3, 7, 16, 100, 1000, 65536, 1 << 30, Integer.MAX_VALUE};
        StringBuilder sb = new StringBuilder();
        for (long seed : seeds) {
            Random r = new Random(seed);
            for (int k = 0; k < 300; k++) {
                switch (k % 6) {
                    case 0 -> sb.append(seed).append(" I 0 ").append(r.nextInt()).append('\n');
                    case 1 -> {
                        int b = bounds[(k / 6) % bounds.length];
                        sb.append(seed).append(" B ").append(b).append(' ').append(r.nextInt(b)).append('\n');
                    }
                    case 2 -> sb.append(seed).append(" L 0 ").append(r.nextLong()).append('\n');
                    case 3 -> sb.append(seed).append(" Z 0 ").append(r.nextBoolean() ? 1 : 0).append('\n');
                    case 4 -> sb.append(seed).append(" F 0 ").append(Float.floatToRawIntBits(r.nextFloat())).append('\n');
                    case 5 -> sb.append(seed).append(" D 0 ").append(Double.doubleToRawLongBits(r.nextDouble())).append('\n');
                    default -> throw new IllegalStateException();
                }
            }
        }
        System.out.print(sb);
    }
}
