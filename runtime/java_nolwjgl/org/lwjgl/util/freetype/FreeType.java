package org.lwjgl.util.freetype;

import org.lwjgl.PointerBuffer;
import org.lwjgl.system.SharedLibrary;

import java.nio.ByteBuffer;
import java.nio.IntBuffer;

/** Zero-LWJGL FreeType adapter returning an error on init so TrueType fonts safely use bitmap font fallback. */
public final class FreeType {
    public static final int FT_ENCODING_UNICODE = 0x756E6963;
    private static final SharedLibrary LIB = new SharedLibrary.Default("freetype");

    private FreeType() {}

    public static SharedLibrary getLibrary() { return LIB; }
    public static int FT_Init_FreeType(PointerBuffer alibrary) { return -1; }
    public static int FT_Done_Library(long library) { return 0; }
    public static int FT_New_Memory_Face(long library, ByteBuffer file_base, long face_index, PointerBuffer aface) { return -1; }
    public static int FT_Done_Face(FT_Face face) { return 0; }
    public static int FT_Select_Charmap(FT_Face face, int encoding) { return -1; }
    public static int FT_Set_Pixel_Sizes(FT_Face face, int pixel_width, int pixel_height) { return -1; }
    public static int FT_Load_Glyph(FT_Face face, int glyph_index, int load_flags) { return -1; }
    public static long FT_Get_First_Char(FT_Face face, IntBuffer agindex) { return 0L; }
    public static long FT_Get_Next_Char(FT_Face face, long charcode, IntBuffer agindex) { return 0L; }
    public static String FT_Get_Font_Format(FT_Face face) { return "TrueType"; }
    public static void FT_Set_Transform(FT_Face face, FT_Matrix matrix, FT_Vector delta) {}
    public static String FT_Error_String(int error_code) { return "FreeType disabled in --no-lwjgl build (using bitmap fonts)"; }
}
