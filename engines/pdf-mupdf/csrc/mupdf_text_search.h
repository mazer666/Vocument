/* Search for a word in the bytes of a PDF object.
 *
 * A PDF stores a text in more than one form: as plain text, as UTF-16 (big
 * endian or little endian) and as a hex string that carries the UTF-16 bytes
 * (`<FEFF00470045…>`). A comparison of the plain text alone therefore does not
 * see everything: a text that is stored hex encoded is perfectly readable in
 * the file, and a redaction that only compares plain text would report
 * success while the content is still in the document.
 *
 * The functions here answer the question "does this buffer carry the word?"
 * for all of these forms at once. */

#ifndef VOCUMENT_MUPDF_TEXT_SEARCH_H
#define VOCUMENT_MUPDF_TEXT_SEARCH_H

#include <stddef.h>
#include <string.h>

/* Compares two byte ranges, ignoring the case of the ASCII letters. */
static inline int vocument_bytes_equal_ci(
    const unsigned char *a,
    const unsigned char *b,
    size_t len
) {
    for (size_t i = 0; i < len; i++) {
        unsigned char ca = a[i];
        unsigned char cb = b[i];
        if (ca >= 'A' && ca <= 'Z') ca = (unsigned char)(ca - 'A' + 'a');
        if (cb >= 'A' && cb <= 'Z') cb = (unsigned char)(cb - 'A' + 'a');
        if (ca != cb) {
            return 0;
        }
    }
    return 1;
}

/* Answers whether the buffer carries the byte range — ignoring the case of
   the ASCII letters. */
static inline int vocument_contains_bytes_ci(
    const unsigned char *hay,
    size_t hay_len,
    const unsigned char *needle,
    size_t needle_len
) {
    if (needle_len == 0 || hay_len < needle_len) {
        return 0;
    }
    for (size_t i = 0; i + needle_len <= hay_len; i++) {
        if (hay[i] == needle[0] && vocument_bytes_equal_ci(hay + i, needle, needle_len)) {
            return 1;
        }
    }
    return 0;
}

/* The hex digits of a byte, in upper case. */
static inline void vocument_hex_pair(unsigned char value, char *out) {
    static const char digits[] = "0123456789ABCDEF";
    out[0] = digits[(value >> 4) & 0x0F];
    out[1] = digits[value & 0x0F];
}

/* Answers whether the buffer carries the word — as plain text, as UTF-16 in
   both byte orders or as the hex string of the UTF-16 bytes. */
static inline int vocument_contains_word(
    const unsigned char *data,
    size_t len,
    const char *word
) {
    if (!data || !word) {
        return 0;
    }
    size_t word_len = strlen(word);
    if (word_len == 0 || len == 0) {
        return 0;
    }

    /* 1. The plain text. */
    if (vocument_contains_bytes_ci(data, len, (const unsigned char *)word, word_len)) {
        return 1;
    }

    /* 2. UTF-16, big endian and little endian. Only the characters of the
       ASCII range are covered: the plain text comparison above takes care of
       the UTF-8 form of the other characters. */
    size_t utf16_be_len = word_len * 2;
    unsigned char utf16_be[512];
    unsigned char utf16_le[512];
    if (utf16_be_len <= sizeof(utf16_be)) {
        for (size_t i = 0; i < word_len; i++) {
            unsigned char c = (unsigned char)word[i];
            utf16_be[i * 2] = 0;
            utf16_be[i * 2 + 1] = c;
            utf16_le[i * 2] = c;
            utf16_le[i * 2 + 1] = 0;
        }
        if (vocument_contains_bytes_ci(data, len, utf16_be, utf16_be_len)) {
            return 1;
        }
        if (vocument_contains_bytes_ci(data, len, utf16_le, utf16_be_len)) {
            return 1;
        }
    }

    /* 3. The hex string of the UTF-16 bytes, with and without the byte order
       mark. A hex string in a PDF is written in upper case, but the file may
       carry it in lower case as well. */
    size_t hex_len = word_len * 4;
    unsigned char hex[1024];
    if (hex_len + 8 <= sizeof(hex)) {
        for (size_t i = 0; i < word_len; i++) {
            vocument_hex_pair(0, (char *)&hex[i * 4]);
            vocument_hex_pair((unsigned char)word[i], (char *)&hex[i * 4 + 2]);
        }
        if (vocument_contains_bytes_ci(data, len, hex, hex_len)) {
            return 1;
        }
        unsigned char with_bom[1024];
        vocument_hex_pair(0xFE, (char *)&with_bom[0]);
        vocument_hex_pair(0xFF, (char *)&with_bom[2]);
        memcpy(&with_bom[4], hex, hex_len);
        if (vocument_contains_bytes_ci(data, len, with_bom, hex_len + 4)) {
            return 1;
        }
    }

    return 0;
}

#endif /* VOCUMENT_MUPDF_TEXT_SEARCH_H */
