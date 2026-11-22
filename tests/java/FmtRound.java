public class FmtRound {
    public static void main(String[] a) {
        double[] v = {0.05, 0.5, 0.95, 0.096, 0.004, 9.5, 99.95, 0.0005};
        for (double d : v) System.out.println(d + " " + String.format("%.0f|%.1f|%.2f", d, d, d));
    }
}
