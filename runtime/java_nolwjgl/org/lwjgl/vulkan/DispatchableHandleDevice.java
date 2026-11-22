package org.lwjgl.vulkan;

public abstract class DispatchableHandleDevice extends org.lwjgl.system.Pointer.Default {
    protected DispatchableHandleDevice(long address, VKCapabilitiesDevice capabilities) { super(address); }
    public VKCapabilitiesDevice getCapabilities() { return new VKCapabilitiesDevice(); }
}
