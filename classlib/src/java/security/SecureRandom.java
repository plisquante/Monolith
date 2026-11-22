package java.security;

import java.util.Random;

/**
 * monolith overlay of SecureRandom: every instance draws from the operating system CSPRNG
 * (getrandom(2)), like the JDK's NativePRNG, without the JCA provider framework.
 */
public class SecureRandom extends Random {
    private static final long serialVersionUID = 4940670005562187L;
    private final String algorithm;

    public SecureRandom() {
        super(0);
        this.algorithm = "NativePRNG";
    }

    public SecureRandom(byte[] seed) {
        this();
    }

    private SecureRandom(String algorithm) {
        super(0);
        this.algorithm = algorithm;
    }

    public static SecureRandom getInstance(String algorithm) throws NoSuchAlgorithmException {
        if (algorithm == null) throw new NullPointerException("null algorithm name");
        return new SecureRandom(algorithm);
    }

    public static SecureRandom getInstance(String algorithm, String provider) throws NoSuchAlgorithmException, NoSuchProviderException {
        return getInstance(algorithm);
    }

    public static SecureRandom getInstanceStrong() throws NoSuchAlgorithmException {
        return new SecureRandom("NativePRNGBlocking");
    }

    public String getAlgorithm() {
        return algorithm;
    }

    public synchronized void setSeed(byte[] seed) {}

    @Override
    public void setSeed(long seed) {}

    @Override
    public void nextBytes(byte[] bytes) {
        fill0(bytes);
    }

    @Override
    protected final int next(int numBits) {
        byte[] b = new byte[4];
        fill0(b);
        int next = (b[0] & 0xff) << 24 | (b[1] & 0xff) << 16 | (b[2] & 0xff) << 8 | (b[3] & 0xff);
        return next >>> (32 - numBits);
    }

    public static byte[] getSeed(int numBytes) {
        byte[] b = new byte[numBytes];
        fill0(b);
        return b;
    }

    public byte[] generateSeed(int numBytes) {
        return getSeed(numBytes);
    }

    public String toString() {
        return algorithm;
    }

    private static native void fill0(byte[] bytes);
}
