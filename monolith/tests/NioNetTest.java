import java.net.*;
import java.nio.*;
import java.nio.channels.*;
import java.nio.file.*;
import java.util.*;

public class NioNetTest {
    public static void main(String[] args) throws Exception {
        // 1. NIO Path / Files / DirectoryStream / FileChannel + mmap
        Path dir = Files.createTempDirectory("mono_nio_");
        Path p = dir.resolve("test.dat");
        Files.writeString(p, "Hello NIO Channel + MMap! ");
        try (FileChannel ch = FileChannel.open(p, StandardOpenOption.READ, StandardOpenOption.WRITE)) {
            MappedByteBuffer mbb = ch.map(FileChannel.MapMode.READ_WRITE, ch.size(), 16);
            mbb.put("MC26.3-RUST!".getBytes("UTF-8"));
            mbb.force();
        }
        String content = Files.readString(p);
        System.out.println("nio " + content.trim() + " sz=" + Files.size(p));

        List<String> listed = new ArrayList<>();
        try (DirectoryStream<Path> ds = Files.newDirectoryStream(dir)) {
            for (Path entry : ds) listed.add(entry.getFileName().toString());
        }
        System.out.println("dir " + listed);
        Files.delete(p);
        Files.delete(dir);

        // 2. TCP loopback via NIO ServerSocketChannel + SocketChannel + Selector (EPoll)
        try (ServerSocketChannel srv = ServerSocketChannel.open()) {
            srv.bind(new InetSocketAddress(InetAddress.getLoopbackAddress(), 0));
            int port = ((InetSocketAddress) srv.getLocalAddress()).getPort();
            Selector sel = Selector.open();
            try (SocketChannel cli = SocketChannel.open()) {
                cli.configureBlocking(false);
                cli.connect(new InetSocketAddress(InetAddress.getLoopbackAddress(), port));
                SelectionKey key = cli.register(sel, SelectionKey.OP_CONNECT);
                while (!cli.finishConnect()) {
                    sel.select(100);
                }
                try (SocketChannel acc = srv.accept()) {
                    cli.write(ByteBuffer.wrap("PING".getBytes("UTF-8")));
                    ByteBuffer in = ByteBuffer.allocate(16);
                    acc.read(in);
                    in.flip();
                    byte[] rb = new byte[in.remaining()];
                    in.get(rb);
                    acc.write(ByteBuffer.wrap((new String(rb, "UTF-8") + "_PONG").getBytes("UTF-8")));
                }
                key.cancel();
                sel.selectNow();
                cli.configureBlocking(true);
                ByteBuffer resp = ByteBuffer.allocate(16);
                cli.read(resp);
                resp.flip();
                byte[] ob = new byte[resp.remaining()];
                resp.get(ob);
                System.out.println("tcp " + new String(ob, "UTF-8"));
            }
            sel.close();
        }

        // 3. UDP loopback via DatagramChannel
        try (DatagramChannel rx = DatagramChannel.open(); DatagramChannel tx = DatagramChannel.open()) {
            rx.bind(new InetSocketAddress(InetAddress.getLoopbackAddress(), 0));
            SocketAddress dst = rx.getLocalAddress();
            tx.send(ByteBuffer.wrap("UDP_OK".getBytes("UTF-8")), dst);
            ByteBuffer d = ByteBuffer.allocate(16);
            rx.receive(d);
            d.flip();
            byte[] u = new byte[d.remaining()];
            d.get(u);
            System.out.println("udp " + new String(u, "UTF-8"));
        }

        // 4. NetworkInterface check
        Enumeration<NetworkInterface> nis = NetworkInterface.getNetworkInterfaces();
        boolean hasLo = false;
        while (nis != null && nis.hasMoreElements()) {
            NetworkInterface ni = nis.nextElement();
            if (ni.isLoopback()) hasLo = true;
        }
        System.out.println("netif loopback=" + hasLo);
    }
}
