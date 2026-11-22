import io.netty.channel.epoll.Epoll;

public class NettyProbe {
    public static void main(String[] args) {
        System.out.println("noNative " + System.getProperty("io.netty.transport.noNative"));
        System.out.println("epoll " + Epoll.isAvailable());
    }
}
