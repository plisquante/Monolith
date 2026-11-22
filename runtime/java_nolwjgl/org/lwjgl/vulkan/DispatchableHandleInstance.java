package org.lwjgl.vulkan;

public abstract class DispatchableHandleInstance extends org.lwjgl.system.Pointer.Default {
    protected DispatchableHandleInstance(long address, VKCapabilitiesInstance capabilities) { super(address); }
    public VKCapabilitiesInstance getCapabilities() { return new VKCapabilitiesInstance(); }
}
