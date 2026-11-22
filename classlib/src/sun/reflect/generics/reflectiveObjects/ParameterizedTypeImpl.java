package sun.reflect.generics.reflectiveObjects;

import java.lang.reflect.MalformedParameterizedTypeException;
import java.lang.reflect.ParameterizedType;
import java.lang.reflect.Type;
import java.lang.reflect.TypeVariable;
import java.util.Arrays;
import java.util.Objects;
import java.util.StringJoiner;

/**
 * monolith overlay of ParameterizedTypeImpl: identical behavior and toString/equals/hashCode to the
 * JDK, except validateConstructorArguments builds its exception message by concatenation instead of
 * String.format (avoiding pulling java.util.Formatter / Locale / Calendar into every HashMap user).
 */
public class ParameterizedTypeImpl implements ParameterizedType {
    private final Type[] actualTypeArguments;
    private final Class<?> rawType;
    private final Type ownerType;

    private ParameterizedTypeImpl(Class<?> rawType, Type[] actualTypeArguments, Type ownerType) {
        this.actualTypeArguments = actualTypeArguments;
        this.rawType = rawType;
        this.ownerType = (ownerType != null) ? ownerType : rawType.getDeclaringClass();
        validateConstructorArguments();
    }

    private void validateConstructorArguments() {
        TypeVariable<?>[] formals = rawType.getTypeParameters();
        if (formals.length != actualTypeArguments.length) {
            throw new MalformedParameterizedTypeException(
                    "Mismatch of count of formal and actual type arguments in constructor of "
                            + rawType + ": " + formals.length + " formal argument(s) "
                            + Arrays.toString(formals) + " vs. " + actualTypeArguments.length
                            + " actual argument(s) " + Arrays.toString(actualTypeArguments));
        }
    }

    public static ParameterizedTypeImpl make(Class<?> rawType, Type[] actualTypeArguments, Type ownerType) {
        return new ParameterizedTypeImpl(rawType, actualTypeArguments, ownerType);
    }

    @Override
    public Type[] getActualTypeArguments() {
        return actualTypeArguments.clone();
    }

    @Override
    public Class<?> getRawType() {
        return rawType;
    }

    @Override
    public Type getOwnerType() {
        return ownerType;
    }

    @Override
    public boolean equals(Object o) {
        if (o instanceof ParameterizedType that) {
            if (this == that) return true;
            return Objects.equals(ownerType, that.getOwnerType())
                    && Objects.equals(rawType, that.getRawType())
                    && Arrays.equals(actualTypeArguments, that.getActualTypeArguments());
        }
        return false;
    }

    @Override
    public int hashCode() {
        return Arrays.hashCode(actualTypeArguments)
                ^ Objects.hashCode(ownerType)
                ^ Objects.hashCode(rawType);
    }

    @Override
    public String toString() {
        StringBuilder sb = new StringBuilder();
        if (ownerType != null) {
            sb.append(ownerType.getTypeName());
            sb.append("$");
            if (ownerType instanceof ParameterizedTypeImpl pt) {
                sb.append(rawType.getName().replace(pt.rawType.getName() + "$", ""));
            } else {
                sb.append(rawType.getSimpleName());
            }
        } else {
            sb.append(rawType.getName());
        }
        if (actualTypeArguments != null) {
            StringJoiner sj = new StringJoiner(", ", "<", ">");
            sj.setEmptyValue("");
            for (Type t : actualTypeArguments) {
                sj.add(t.getTypeName());
            }
            sb.append(sj.toString());
        }
        return sb.toString();
    }
}
