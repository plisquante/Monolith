import java.lang.reflect.Field;
import java.lang.reflect.InvocationHandler;
import java.lang.reflect.Method;
import java.lang.reflect.Proxy;

public class JnaProxyFold {
    public interface UdevLike {
        long udev_new();
        int udev_enumerate_scan_devices(long handle, String subsys);
    }

    public static void main(String[] args) throws Exception {
        Field coder = String.class.getDeclaredField("coder");
        if (coder == null || coder.getType() != byte.class) {
            throw new AssertionError("missing String.coder");
        }
        Class<?>[] ifaces = new Class<?>[]{ UdevLike.class };
        UdevLike proxy = (UdevLike) Proxy.newProxyInstance(
                JnaProxyFold.class.getClassLoader(),
                ifaces,
                new InvocationHandler() {
                    @Override
                    public Object invoke(Object p, Method m, Object[] a) {
                        if ("udev_new".equals(m.getName())) return 1234L;
                        if ("udev_enumerate_scan_devices".equals(m.getName())) {
                            return ((Long) a[0]).intValue() + ((String) a[1]).length();
                        }
                        return null;
                    }
                });
        long h = proxy.udev_new();
        int r = proxy.udev_enumerate_scan_devices(h, "drm");
        System.out.println("JNA_PROXY_CODER_OK h=" + h + " r=" + r + " field=" + coder.getName());
    }
}
