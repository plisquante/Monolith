import java.util.*;

public class Strings {
    static String kind(String s) {
        switch (s) {
            case "apple": return "fruit";
            case "carrot": case "potato": return "veg";
            case "Aa": return "Aa"; case "BB": return "BB";
            default: return "unknown";
        }
    }

    public static void main(String[] args) {
        String a = "Hello", b = "World";
        String c = a + ", " + b + '!' + 42 + 3.5 + 2.5f + true + 'x' + 10L + null + (Object) "obj";
        System.out.println(c + " " + c.length() + " " + c.hashCode());
        System.out.println(c.substring(7) + "|" + c.substring(0, 5) + "|" + c.indexOf('o') + "|" + c.lastIndexOf("o") + "|" + c.indexOf("World") + "|" + c.charAt(4));
        System.out.println(c.toUpperCase() + "|" + c.toLowerCase() + "|" + "  trim me  ".trim() + "|" + "  strip  ".strip() + "|" + "ab".repeat(3) + "|" + "".isEmpty() + "|" + " ".isBlank());
        System.out.println(String.join(",", "a", "b", "c") + "|" + String.join("-", List.of("x", "y")) + "|" + "a,b,,c,,".split(",").length + "|" + Arrays.toString("a1b2c3".split("\\d")) + "|" + Arrays.toString("x  y z".split(" +")));
        System.out.println("banana".replace('a', 'o') + "|" + "banana".replace("an", "AN") + "|" + "aaa".replaceAll("a+", "b") + "|" + "x1y22z".replaceAll("[0-9]+", "#") + "|" + "Hello".matches("H.*o"));
        System.out.println("apple".compareTo("apricot") + " " + "a".compareToIgnoreCase("A") + " " + "abc".equals("abc") + " " + "ABC".equalsIgnoreCase("abc") + " " + "abc".contains("bc") + " " + "abc".startsWith("ab") + " " + "abc".endsWith("bc"));
        for (String s : new String[]{"apple", "carrot", "potato", "Aa", "BB", "zzz"}) System.out.print(kind(s) + " ");
        System.out.println();
        StringBuilder sb = new StringBuilder();
        for (int i = 0; i < 20; i++) sb.append(i).append(',');
        sb.setLength(sb.length() - 1);
        sb.insert(0, "[").append(']').reverse();
        sb.deleteCharAt(0).replace(0, 2, "XX").setCharAt(3, '#');
        System.out.println(sb + " " + sb.length() + " " + sb.indexOf("1") + " " + sb.charAt(5));
        String s1 = "intern", s2 = new String("intern");
        System.out.println((s1 == s2) + " " + (s1 == s2.intern()) + " " + (s1 == "intern") + " " + s1.equals(s2));
        char[] cs = "characters".toCharArray();
        Arrays.sort(cs);
        System.out.println(new String(cs) + " " + String.valueOf(cs, 2, 3) + " " + String.valueOf(3.0) + " " + String.valueOf(true));
        String unicode = "çğüşöıİ €𝄞 日本";
        System.out.println(unicode + " " + unicode.length() + " " + unicode.codePointCount(0, unicode.length()) + " " + unicode.toUpperCase() + " " + Arrays.toString(unicode.getBytes(java.nio.charset.StandardCharsets.UTF_8)).length());
        System.out.println(new String(unicode.getBytes(java.nio.charset.StandardCharsets.UTF_8), java.nio.charset.StandardCharsets.UTF_8).equals(unicode));
        String text = """
                Text block line 1
                  indented "quoted"
                end""";
        System.out.println(text);
        System.out.println("chars: " + "hello".chars().filter(ch -> ch == 'l').count());
        Object o = 42;
        String r = o instanceof Integer i && i > 40 ? "big " + i : "small";
        System.out.println(r);
        System.out.println(Integer.valueOf(127) == Integer.valueOf(127));
        System.out.println(String.valueOf((Object) null) + " " + Objects.toString(null, "def") + " " + Objects.hash(1, 2, 3) + " " + Arrays.hashCode(new int[]{1, 2}) + " " + List.of(1, 2).hashCode() + " " + "x".hashCode());
    }
}
