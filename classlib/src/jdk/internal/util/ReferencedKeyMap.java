package jdk.internal.util;

import java.lang.ref.Reference;
import java.lang.ref.ReferenceQueue;
import java.util.AbstractMap;
import java.util.ArrayList;
import java.util.Collection;
import java.util.Collections;
import java.util.LinkedHashSet;
import java.util.Map;
import java.util.Objects;
import java.util.Set;
import java.util.concurrent.ConcurrentHashMap;
import java.util.function.Supplier;
import java.util.function.UnaryOperator;

/**
 * j2zig overlay of jdk.internal.util.ReferencedKeyMap: identical API and behavior to OpenJDK,
 * except keySet/values/entrySet/toString iterate directly instead of through java.util.stream.Stream
 * (avoiding pulling Stream and ForkJoinPool into every MethodType / MethodHandle / Proxy user).
 */
public final class ReferencedKeyMap<K, V> implements Map<K, V> {
    private final boolean isSoft;
    private final Map<ReferenceKey<K>, V> map;
    private final ReferenceQueue<K> stale;

    private ReferencedKeyMap(boolean isSoft, Map<ReferenceKey<K>, V> map, ReferenceQueue<K> stale) {
        this.isSoft = isSoft;
        this.map = map;
        this.stale = stale;
    }

    public static <K, V> Supplier<Map<ReferenceKey<K>, V>> concurrentHashMapSupplier() {
        return new Supplier<Map<ReferenceKey<K>, V>>() {
            @Override
            public Map<ReferenceKey<K>, V> get() {
                return new ConcurrentHashMap<>();
            }
        };
    }

    public static <K, V> ReferencedKeyMap<K, V> create(boolean isSoft, Supplier<Map<ReferenceKey<K>, V>> supplier) {
        return new ReferencedKeyMap<>(isSoft, supplier.get(), new ReferenceQueue<>());
    }

    @SuppressWarnings("unchecked")
    private ReferenceKey<K> entryKey(Object key) {
        if (isSoft) {
            return new SoftReferenceKey<>((K) key, stale);
        } else {
            return new WeakReferenceKey<>((K) key, stale);
        }
    }

    @SuppressWarnings("unchecked")
    private ReferenceKey<K> lookupKey(Object key) {
        return new StrongReferenceKey<>((K) key);
    }

    @Override
    public int size() {
        removeStaleReferences();
        return map.size();
    }

    @Override
    public boolean isEmpty() {
        removeStaleReferences();
        return map.isEmpty();
    }

    @Override
    public boolean containsKey(Object key) {
        Objects.requireNonNull(key, "key must not be null");
        removeStaleReferences();
        return map.containsKey(lookupKey(key));
    }

    @Override
    public boolean containsValue(Object value) {
        Objects.requireNonNull(value, "value must not be null");
        removeStaleReferences();
        return map.containsValue(value);
    }

    @Override
    public V get(Object key) {
        removeStaleReferences();
        return getNoCheckStale(key);
    }

    private V getNoCheckStale(Object key) {
        Objects.requireNonNull(key, "key must not be null");
        return map.get(lookupKey(key));
    }

    @Override
    public V put(K key, V newValue) {
        Objects.requireNonNull(key, "key must not be null");
        Objects.requireNonNull(newValue, "value must not be null");
        removeStaleReferences();
        ReferenceKey<K> entryKey = entryKey(key);
        V oldValue = map.put(entryKey, newValue);
        if (oldValue != null) {
            entryKey.unused();
        }
        return oldValue;
    }

    @Override
    public V remove(Object key) {
        Objects.requireNonNull(key, "key must not be null");
        return map.remove(lookupKey(key));
    }

    @Override
    public void putAll(Map<? extends K, ? extends V> m) {
        removeStaleReferences();
        for (Entry<? extends K, ? extends V> entry : m.entrySet()) {
            put(entry.getKey(), entry.getValue());
        }
    }

    @Override
    public void clear() {
        removeStaleReferences();
        map.clear();
    }

    @Override
    public Set<K> keySet() {
        removeStaleReferences();
        LinkedHashSet<K> set = new LinkedHashSet<>();
        for (ReferenceKey<K> rk : map.keySet()) {
            K k = rk.get();
            if (k != null) set.add(k);
        }
        return Collections.unmodifiableSet(set);
    }

    @Override
    public Collection<V> values() {
        removeStaleReferences();
        return Collections.unmodifiableCollection(new ArrayList<>(map.values()));
    }

    @Override
    public Set<Entry<K, V>> entrySet() {
        removeStaleReferences();
        LinkedHashSet<Entry<K, V>> set = new LinkedHashSet<>();
        for (ReferenceKey<K> rk : map.keySet()) {
            K k = rk.get();
            if (k != null) {
                V v = getNoCheckStale(k);
                if (v != null) {
                    set.add(new AbstractMap.SimpleEntry<>(k, v));
                }
            }
        }
        return Collections.unmodifiableSet(set);
    }

    @Override
    public V putIfAbsent(K key, V newValue) {
        removeStaleReferences();
        ReferenceKey<K> entryKey = entryKey(key);
        V oldValue = map.putIfAbsent(entryKey, newValue);
        if (oldValue != null) {
            entryKey.unused();
        }
        return oldValue;
    }

    @Override
    public boolean remove(Object key, Object value) {
        return map.remove(lookupKey(key), value);
    }

    @Override
    public boolean replace(K key, V oldValue, V newValue) {
        removeStaleReferences();
        return map.replace(lookupKey(key), oldValue, newValue);
    }

    @Override
    public V replace(K key, V value) {
        removeStaleReferences();
        return map.replace(lookupKey(key), value);
    }

    @Override
    public String toString() {
        removeStaleReferences();
        StringBuilder sb = new StringBuilder("{");
        boolean first = true;
        for (ReferenceKey<K> rk : map.keySet()) {
            K k = rk.get();
            if (k != null) {
                if (!first) sb.append(", ");
                first = false;
                sb.append(k).append('=').append(getNoCheckStale(k));
            }
        }
        return sb.append('}').toString();
    }

    @SuppressWarnings("unchecked")
    public void removeStaleReferences() {
        while (true) {
            Object key = stale.poll();
            if (key == null) {
                break;
            }
            map.remove(key);
        }
    }

    public void prepareForAOTCache() {
        removeStaleReferences();
    }

    static <T> T intern(ReferencedKeyMap<T, ReferenceKey<T>> setMap, T key) {
        T value = existingKey(setMap, key);
        if (value != null) {
            return value;
        }
        return internKey(setMap, key);
    }

    static <T> T intern(ReferencedKeyMap<T, ReferenceKey<T>> setMap, T key, UnaryOperator<T> op) {
        T value = existingKey(setMap, key);
        if (value != null) {
            return value;
        }
        key = op.apply(key);
        return internKey(setMap, key);
    }

    private static <T> T existingKey(ReferencedKeyMap<T, ReferenceKey<T>> setMap, T key) {
        setMap.removeStaleReferences();
        ReferenceKey<T> entry = setMap.getNoCheckStale(key);
        return entry != null ? entry.get() : null;
    }

    private static <T> T internKey(ReferencedKeyMap<T, ReferenceKey<T>> setMap, T key) {
        ReferenceKey<T> entryKey = setMap.entryKey(key);
        T interned;
        do {
            setMap.removeStaleReferences();
            ReferenceKey<T> existing = setMap.map.putIfAbsent(entryKey, entryKey);
            if (existing == null) {
                return key;
            } else {
                interned = existing.get();
                if (interned != null) {
                    entryKey.unused();
                }
            }
        } while (interned == null);
        return interned;
    }

    static <T> boolean internAddKey(ReferencedKeyMap<T, ReferenceKey<T>> setMap, T key) {
        ReferenceKey<T> entryKey = setMap.entryKey(key);
        T interned;
        do {
            setMap.removeStaleReferences();
            ReferenceKey<T> existing = setMap.map.putIfAbsent(entryKey, entryKey);
            if (existing == null) {
                return true;
            } else {
                interned = existing.get();
                if (interned != null) {
                    entryKey.unused();
                }
            }
        } while (interned == null);
        return false;
    }
}
