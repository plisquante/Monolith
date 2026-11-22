package org.lwjgl.glfw;

public final class Callbacks {
    private Callbacks() {}

    public static void glfwFreeCallbacks(long window) {
        GLFW.glfwSetKeyCallback(window, null);
        GLFW.glfwSetMouseButtonCallback(window, null);
        GLFW.glfwSetCursorPosCallback(window, null);
        GLFW.glfwSetFramebufferSizeCallback(window, null);
        GLFW.glfwSetCharCallback(window, null);
        GLFW.glfwSetScrollCallback(window, null);
        GLFW.glfwSetWindowFocusCallback(window, null);
    }
}
