# libmonoshader.so for Android: shaderc (NDK sources/third_party/shaderc) + SPIRV-Cross + the JNI glue.
# Built by tools/build_shader_lib.py, which passes SPIRV_CROSS_DIR (a SPIRV-Cross checkout).
LOCAL_PATH := $(call my-dir)

include $(CLEAR_VARS)
LOCAL_MODULE := monoshader
SPVX := $(SPIRV_CROSS_DIR)
LOCAL_SRC_FILES := ../monoshader.c \
    $(SPVX)/spirv_cross.cpp $(SPVX)/spirv_cfg.cpp $(SPVX)/spirv_cross_parsed_ir.cpp $(SPVX)/spirv_cross_util.cpp \
    $(SPVX)/spirv_glsl.cpp $(SPVX)/spirv_parser.cpp $(SPVX)/spirv_reflect.cpp $(SPVX)/spirv_cross_c.cpp
LOCAL_C_INCLUDES := $(SPVX)
LOCAL_CPP_EXTENSION := .cpp .cc
LOCAL_CPPFLAGS := -std=c++17 -fexceptions -frtti \
    -DSPIRV_CROSS_C_API_GLSL=1 -DSPIRV_CROSS_C_API_REFLECT=1 \
    -DSPIRV_CROSS_C_API_HLSL=0 -DSPIRV_CROSS_C_API_MSL=0 -DSPIRV_CROSS_C_API_CPP=0
LOCAL_STATIC_LIBRARIES := shaderc
LOCAL_LDLIBS := -llog
LOCAL_LDFLAGS := -Wl,--gc-sections
include $(BUILD_SHARED_LIBRARY)

$(call import-add-path, $(NDK_ROOT)/sources/third_party)
$(call import-module, shaderc)
