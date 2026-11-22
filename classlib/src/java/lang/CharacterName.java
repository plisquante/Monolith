package java.lang;

/**
 * j2zig overlay: the Unicode character name table (uniName.dat resource) is not embedded in
 * native images. Character.getName returns null and \N{...} lookups fail as "unknown name".
 */
class CharacterName {
    private static final CharacterName INSTANCE = new CharacterName();

    static CharacterName getInstance() {
        return INSTANCE;
    }

    private CharacterName() {}

    public String getName(int cp) {
        return null;
    }

    public int getCodePoint(String name) {
        return -1;
    }
}
