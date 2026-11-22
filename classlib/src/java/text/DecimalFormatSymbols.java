package java.text;

import java.util.Locale;
import java.util.Objects;

/**
 * j2zig overlay of DecimalFormatSymbols. The CLDR/JRE locale data providers are not part of the
 * native image, so every locale gets the symbols the JDK uses for Locale.US / Locale.ROOT
 * (this limitation is listed in docs/STATUS.md).
 */
public class DecimalFormatSymbols implements Cloneable, java.io.Serializable {
    private static final long serialVersionUID = 5772796243397350300L;
    private char zeroDigit = '0';
    private char groupingSeparator = ',';
    private char decimalSeparator = '.';
    private char perMill = '‰';
    private char percent = '%';
    private char digit = '#';
    private char patternSeparator = ';';
    private String infinity = "∞";
    private String NaN = "NaN";
    private char minusSign = '-';
    private String currencySymbol = "$";
    private String intlCurrencySymbol = "USD";
    private char monetarySeparator = '.';
    private char exponential = 'E';
    private String exponentialSeparator = "E";
    private char monetaryGroupingSeparator = ',';
    private String perMillText = "‰";
    private String percentText = "%";
    private String minusSignText = "-";
    private final Locale locale;

    public DecimalFormatSymbols() {
        this(Locale.getDefault(Locale.Category.FORMAT));
    }

    public DecimalFormatSymbols(Locale locale) {
        this.locale = Objects.requireNonNull(locale);
        if (!"US".equals(locale.getCountry())) {
            currencySymbol = "¤";
            intlCurrencySymbol = "XXX";
        }
    }

    public static Locale[] getAvailableLocales() {
        return new Locale[]{Locale.ROOT, Locale.US};
    }

    public static final DecimalFormatSymbols getInstance() {
        return getInstance(Locale.getDefault(Locale.Category.FORMAT));
    }

    public static final DecimalFormatSymbols getInstance(Locale locale) {
        return new DecimalFormatSymbols(locale);
    }

    public Locale getLocale() { return locale; }
    public char getZeroDigit() { return zeroDigit; }
    public void setZeroDigit(char zeroDigit) { this.zeroDigit = zeroDigit; }
    public char getGroupingSeparator() { return groupingSeparator; }
    public void setGroupingSeparator(char c) { groupingSeparator = c; }
    public char getDecimalSeparator() { return decimalSeparator; }
    public void setDecimalSeparator(char c) { decimalSeparator = c; }
    public char getPerMill() { return perMill; }
    public void setPerMill(char c) { perMill = c; perMillText = Character.toString(c); }
    public char getPercent() { return percent; }
    public void setPercent(char c) { percent = c; percentText = Character.toString(c); }
    public char getDigit() { return digit; }
    public void setDigit(char c) { digit = c; }
    public char getPatternSeparator() { return patternSeparator; }
    public void setPatternSeparator(char c) { patternSeparator = c; }
    public String getInfinity() { return infinity; }
    public void setInfinity(String s) { infinity = s; }
    public String getNaN() { return NaN; }
    public void setNaN(String s) { NaN = s; }
    public char getMinusSign() { return minusSign; }
    public void setMinusSign(char c) { minusSign = c; minusSignText = Character.toString(c); }
    public String getCurrencySymbol() { return currencySymbol; }
    public void setCurrencySymbol(String s) { currencySymbol = s; }
    public String getInternationalCurrencySymbol() { return intlCurrencySymbol; }
    public void setInternationalCurrencySymbol(String s) { intlCurrencySymbol = s; }
    public java.util.Currency getCurrency() { return java.util.Currency.getInstance(intlCurrencySymbol); }
    public void setCurrency(java.util.Currency currency) {
        intlCurrencySymbol = currency.getCurrencyCode();
        currencySymbol = currency.getSymbol(locale);
    }
    public char getMonetaryDecimalSeparator() { return monetarySeparator; }
    public void setMonetaryDecimalSeparator(char sep) { monetarySeparator = sep; }
    public String getExponentSeparator() { return exponentialSeparator; }
    public void setExponentSeparator(String exp) { exponentialSeparator = Objects.requireNonNull(exp); }
    public char getMonetaryGroupingSeparator() { return monetaryGroupingSeparator; }
    public void setMonetaryGroupingSeparator(char c) { monetaryGroupingSeparator = c; }
    char getExponentialSymbol() { return exponential; }
    void setExponentialSymbol(char exp) { exponential = exp; }
    String getPerMillText() { return perMillText; }
    void setPerMillText(String s) { perMillText = s; perMill = s.length() == 1 ? s.charAt(0) : perMill; }
    String getPercentText() { return percentText; }
    void setPercentText(String s) { percentText = s; percent = s.length() == 1 ? s.charAt(0) : percent; }
    String getMinusSignText() { return minusSignText; }
    void setMinusSignText(String s) { minusSignText = s; minusSign = s.length() == 1 ? s.charAt(0) : minusSign; }

    public Object clone() {
        try {
            return super.clone();
        } catch (CloneNotSupportedException e) {
            throw new InternalError(e);
        }
    }

    public boolean equals(Object obj) {
        if (obj == null) return false;
        if (this == obj) return true;
        if (getClass() != obj.getClass()) return false;
        DecimalFormatSymbols other = (DecimalFormatSymbols) obj;
        return zeroDigit == other.zeroDigit && groupingSeparator == other.groupingSeparator
                && decimalSeparator == other.decimalSeparator && percentText.equals(other.percentText)
                && perMillText.equals(other.perMillText) && digit == other.digit && minusSignText.equals(other.minusSignText)
                && patternSeparator == other.patternSeparator && infinity.equals(other.infinity) && NaN.equals(other.NaN)
                && currencySymbol.equals(other.currencySymbol) && intlCurrencySymbol.equals(other.intlCurrencySymbol)
                && monetarySeparator == other.monetarySeparator && monetaryGroupingSeparator == other.monetaryGroupingSeparator
                && exponentialSeparator.equals(other.exponentialSeparator) && locale.equals(other.locale);
    }

    public int hashCode() {
        return Objects.hash(zeroDigit, groupingSeparator, decimalSeparator, percentText, perMillText, digit,
                minusSignText, patternSeparator, infinity, NaN, currencySymbol, intlCurrencySymbol,
                monetarySeparator, monetaryGroupingSeparator, exponentialSeparator, locale);
    }
}
