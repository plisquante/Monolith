import com.mojang.text2speech.Narrator;

public class NarratorProbe {
    public static void main(String[] a) {
        Narrator n = Narrator.getNarrator();
        System.out.println(n == Narrator.EMPTY ? "narrator: empty" : "narrator: " + n.getClass().getName() + " active=" + n.active());
        n.destroy();
    }
}
