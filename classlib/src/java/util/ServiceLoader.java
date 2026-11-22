package java.util;

import java.util.function.Consumer;
import java.util.function.Supplier;
import java.util.stream.Stream;
import java.util.stream.StreamSupport;

/**
 * monolith overlay of ServiceLoader. Providers are resolved at build time from the
 * META-INF/services files of the application class path and compiled into a table;
 * instances are created by the runtime through each provider's public no-arg constructor.
 */
public final class ServiceLoader<S> implements Iterable<S> {
    private final Class<S> service;
    private List<S> cache;

    private ServiceLoader(Class<S> service) {
        this.service = Objects.requireNonNull(service, "service interface cannot be null");
    }

    private static native Object[] providers0(String serviceName);

    private synchronized List<S> providers() {
        if (cache == null) {
            Object[] ps = providers0(service.getName());
            List<S> l = new ArrayList<>(ps.length);
            for (Object p : ps) l.add(service.cast(p));
            cache = l;
        }
        return cache;
    }

    public Iterator<S> iterator() {
        return Collections.unmodifiableList(providers()).iterator();
    }

    public static interface Provider<S> extends Supplier<S> {
        Class<? extends S> type();

        @Override
        S get();
    }

    public Stream<Provider<S>> stream() {
        List<Provider<S>> l = new ArrayList<>();
        for (S s : providers()) {
            l.add(new Provider<S>() {
                @SuppressWarnings("unchecked")
                public Class<? extends S> type() { return (Class<? extends S>) s.getClass(); }
                public S get() { return s; }
            });
        }
        return l.stream();
    }

    public static <S> ServiceLoader<S> load(Class<S> service, ClassLoader loader) {
        return new ServiceLoader<>(service);
    }

    public static <S> ServiceLoader<S> load(Class<S> service, ClassLoader loader, Module callerModule) {
        return load(service, loader);
    }

    public static <S> ServiceLoader<S> load(Class<S> service) {
        return new ServiceLoader<>(service);
    }

    public static <S> ServiceLoader<S> loadInstalled(Class<S> service) {
        return new ServiceLoader<>(service);
    }

    public static <S> ServiceLoader<S> load(ModuleLayer layer, Class<S> service) {
        return new ServiceLoader<>(service);
    }

    public Optional<S> findFirst() {
        List<S> p = providers();
        return p.isEmpty() ? Optional.empty() : Optional.of(p.get(0));
    }

    public synchronized void reload() {
        cache = null;
    }

    public String toString() {
        return "java.util.ServiceLoader[" + service.getName() + "]";
    }
}
