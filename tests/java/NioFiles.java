import java.io.*;
import java.nio.*;
import java.nio.channels.*;
import java.nio.charset.StandardCharsets;
import java.nio.file.*;
import java.nio.file.attribute.*;
import java.util.*;
import java.util.stream.*;

public class NioFiles {
    public static void main(String[] args) throws Exception {
        Path dir = Files.createTempDirectory("j2zniotest");
        try {
            FileStore fs = Files.getFileStore(dir);
            System.out.println("store " + fs.type() + " " + (fs.getTotalSpace() > 0) + " " + fs.name().isEmpty());
            Path a = dir.resolve("a.txt");
            Files.writeString(a, "hello\nworld\n");
            System.out.println(Files.readString(a).length() + " " + Files.readAllLines(a) + " " + Files.size(a) + " " + Files.exists(a) + " " + Files.isDirectory(dir));
            try (BufferedWriter w = Files.newBufferedWriter(dir.resolve("b.txt"), StandardCharsets.UTF_8)) {
                for (int i = 0; i < 1000; i++) w.write("line " + i + "\n");
            }
            try (BufferedReader r = Files.newBufferedReader(dir.resolve("b.txt"))) {
                System.out.println(r.lines().mapToInt(String::length).sum());
            }
            try (FileChannel ch = FileChannel.open(dir.resolve("c.bin"), StandardOpenOption.CREATE, StandardOpenOption.READ, StandardOpenOption.WRITE)) {
                ByteBuffer bb = ByteBuffer.allocate(4096);
                for (int i = 0; i < 1024; i++) bb.putInt(i * 7);
                bb.flip();
                System.out.println("wrote " + ch.write(bb) + " size " + ch.size());
                ByteBuffer d = ByteBuffer.allocateDirect(16);
                ch.read(d, 8);
                d.flip();
                System.out.println(d.getInt() + " " + d.getInt() + " pos " + ch.position());
                try (FileLock l = ch.tryLock()) {
                    System.out.println("lock " + (l != null) + " " + l.isShared());
                }
                MappedByteBuffer m = ch.map(FileChannel.MapMode.READ_WRITE, 0, 4096);
                m.putInt(0, 12345);
                m.force();
                ch.truncate(2048);
                System.out.println("mapped " + m.getInt(4) + " size " + ch.size());
                ByteBuffer[] parts = { ByteBuffer.allocate(4), ByteBuffer.allocate(4) };
                ch.position(0);
                System.out.println("readv " + ch.read(parts) + " " + parts[0].flip().getInt() + " " + parts[1].flip().getInt());
                try (FileChannel out = FileChannel.open(dir.resolve("d.bin"), StandardOpenOption.CREATE, StandardOpenOption.WRITE)) {
                    System.out.println("transfer " + ch.transferTo(0, ch.size(), out) + " " + out.size());
                }
            }
            Files.createDirectories(dir.resolve("x/y/z"));
            Files.copy(a, dir.resolve("x/y/z/copy.txt"));
            Files.move(dir.resolve("b.txt"), dir.resolve("x/moved.txt"), StandardCopyOption.ATOMIC_MOVE);
            try (Stream<Path> s = Files.walk(dir)) {
                System.out.println(s.map(p -> dir.relativize(p).toString()).sorted().collect(Collectors.toList()));
            }
            try (DirectoryStream<Path> ds = Files.newDirectoryStream(dir, "*.bin")) {
                List<String> l = new ArrayList<>();
                for (Path p : ds) l.add(p.getFileName().toString());
                Collections.sort(l);
                System.out.println(l);
            }
            BasicFileAttributes at = Files.readAttributes(a, BasicFileAttributes.class);
            System.out.println(at.isRegularFile() + " " + at.size() + " " + (at.lastModifiedTime().toMillis() > 0));
            try (InputStream in = Files.newInputStream(a); OutputStream o = Files.newOutputStream(dir.resolve("e.txt"))) {
                System.out.println("copied " + in.transferTo(o));
            }
            try { Files.readString(dir.resolve("missing")); } catch (NoSuchFileException e) { System.out.println("NSFE " + dir.relativize(Path.of(e.getFile()))); }
        } finally {
            try (Stream<Path> s = Files.walk(dir)) {
                for (Path p : s.sorted(Comparator.reverseOrder()).collect(Collectors.toList())) Files.delete(p);
            }
        }
        System.out.println("cleaned " + !Files.exists(dir));
    }
}
