import java.io.*;
import java.net.*;
import java.nio.*;
import java.nio.channels.*;
import java.nio.charset.StandardCharsets;
import java.nio.file.*;
import java.util.*;
import java.util.concurrent.*;

public class NetIo {
    public static void main(String[] args) throws Exception {
        // blocking sockets over loopback
        try (ServerSocket ss = new ServerSocket(0, 5, InetAddress.getLoopbackAddress())) {
            int port = ss.getLocalPort();
            Thread t = new Thread(() -> {
                try (Socket c = ss.accept()) {
                    BufferedReader r = new BufferedReader(new InputStreamReader(c.getInputStream()));
                    String line = r.readLine();
                    c.getOutputStream().write(("echo:" + line + "\n").getBytes());
                } catch (IOException e) { e.printStackTrace(); }
            });
            t.start();
            try (Socket s = new Socket(InetAddress.getLoopbackAddress(), port)) {
                s.setTcpNoDelay(true);
                s.setSoTimeout(5000);
                s.getOutputStream().write("hello\n".getBytes());
                System.out.println(new BufferedReader(new InputStreamReader(s.getInputStream())).readLine());
                System.out.println(s.getInetAddress().isLoopbackAddress() + " " + (s.getPort() == port) + " " + s.getTcpNoDelay());
            }
            t.join();
        }
        // nio selector + non blocking
        try (Selector sel = Selector.open(); ServerSocketChannel ssc = ServerSocketChannel.open()) {
            ssc.bind(new InetSocketAddress(InetAddress.getLoopbackAddress(), 0));
            ssc.configureBlocking(false);
            ssc.register(sel, SelectionKey.OP_ACCEPT);
            int port = ((InetSocketAddress) ssc.getLocalAddress()).getPort();
            SocketChannel cl = SocketChannel.open();
            cl.configureBlocking(false);
            cl.connect(new InetSocketAddress(InetAddress.getLoopbackAddress(), port));
            cl.register(sel, SelectionKey.OP_CONNECT);
            StringBuilder log = new StringBuilder();
            ByteBuffer buf = ByteBuffer.allocate(64);
            boolean sent = false, done = false;
            SocketChannel srv = null;
            for (int i = 0; i < 100 && !done; i++) {
                sel.select(500);
                for (Iterator<SelectionKey> it = sel.selectedKeys().iterator(); it.hasNext(); ) {
                    SelectionKey k = it.next(); it.remove();
                    if (k.isAcceptable()) {
                        srv = ssc.accept(); srv.configureBlocking(false); srv.register(sel, SelectionKey.OP_READ); log.append("A");
                    } else if (k.isConnectable()) {
                        cl.finishConnect(); k.interestOps(SelectionKey.OP_WRITE); log.append("C");
                    } else if (k.isWritable() && !sent) {
                        cl.write(ByteBuffer.wrap("ping".getBytes())); sent = true; k.interestOps(0); log.append("W");
                    } else if (k.isReadable()) {
                        int n = srv.read(buf); log.append("R" + n); done = true;
                    }
                }
            }
            buf.flip();
            char[] ev = log.toString().toCharArray(); java.util.Arrays.sort(ev); // key order within one select() is unspecified
            System.out.println(new String(ev) + " " + StandardCharsets.UTF_8.decode(buf));
            cl.close(); if (srv != null) srv.close();
        }
        // datagrams
        try (DatagramSocket a = new DatagramSocket(0, InetAddress.getLoopbackAddress()); DatagramSocket b = new DatagramSocket(0, InetAddress.getLoopbackAddress())) {
            byte[] msg = "udp!".getBytes();
            a.send(new DatagramPacket(msg, msg.length, InetAddress.getLoopbackAddress(), b.getLocalPort()));
            DatagramPacket p = new DatagramPacket(new byte[32], 32);
            b.setSoTimeout(3000);
            b.receive(p);
            System.out.println(new String(p.getData(), 0, p.getLength()) + " " + (p.getPort() == a.getLocalPort()));
        }
        // unix domain sockets
        Path sock = Files.createTempDirectory("mononet").resolve("s.sock");
        try (ServerSocketChannel us = ServerSocketChannel.open(StandardProtocolFamily.UNIX)) {
            us.bind(UnixDomainSocketAddress.of(sock));
            Thread t = new Thread(() -> {
                try (SocketChannel c = us.accept()) { ByteBuffer b = ByteBuffer.allocate(8); c.read(b); b.flip(); c.write(b); } catch (IOException e) { e.printStackTrace(); }
            });
            t.start();
            try (SocketChannel c = SocketChannel.open(UnixDomainSocketAddress.of(sock))) {
                c.write(ByteBuffer.wrap("unix".getBytes()));
                ByteBuffer b = ByteBuffer.allocate(8);
                c.read(b); b.flip();
                System.out.println(StandardCharsets.UTF_8.decode(b));
            }
            t.join();
        }
        Files.deleteIfExists(sock); Files.deleteIfExists(sock.getParent());
        // names and interfaces
        System.out.println(InetAddress.getByName("localhost").isLoopbackAddress() + " " + InetAddress.getByName("127.0.0.1").getHostAddress() + " " + InetAddress.getByAddress(new byte[]{10, 1, 2, 3}).getHostAddress());
        System.out.println(InetAddress.getByName("::1").getHostAddress() + " " + (InetAddress.getLocalHost() != null));
        boolean lo = false;
        for (NetworkInterface ni : Collections.list(NetworkInterface.getNetworkInterfaces())) if (ni.isLoopback() && ni.isUp()) lo = true;
        System.out.println("loopback iface " + lo + " " + (NetworkInterface.getByInetAddress(InetAddress.getLoopbackAddress()) != null));
        try { new Socket(InetAddress.getLoopbackAddress(), 1); } catch (ConnectException e) { System.out.println("refused: " + e.getMessage()); }
        try { InetAddress.getByName("no-such-host.invalid"); } catch (UnknownHostException e) { System.out.println("UHE"); }
        try (ServerSocket s1 = new ServerSocket(0)) { try { new ServerSocket(s1.getLocalPort()); } catch (BindException e) { System.out.println("bind: " + e.getMessage()); } }
        // executor + futures over sockets
        ExecutorService ex = Executors.newFixedThreadPool(4);
        try (ServerSocket ss = new ServerSocket(0, 50, InetAddress.getLoopbackAddress())) {
            ex.submit(() -> { for (int i = 0; i < 4; i++) { Socket c = ss.accept(); c.getOutputStream().write(i); c.close(); } return null; });
            int sum = 0;
            for (int i = 0; i < 4; i++) try (Socket c = new Socket(InetAddress.getLoopbackAddress(), ss.getLocalPort())) { sum += c.getInputStream().read(); }
            System.out.println("sum " + sum);
        }
        ex.shutdownNow();
    }
}
