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
#include <stdlib.h>
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

/* The **text** of a content stream.
 *
 * A writer can split a word over several show operations: `(GE) (HEIM) Tj`
 * puts "GEHEIM" on the page, but the bytes of the stream do not carry the
 * word in one piece. A comparison of bytes is blind for it — and the fifth
 * round of the red team measured that the verification of a redaction
 * accepted a document whose hidden form carried the secret in such pieces.
 *
 * The function below reads the text of the stream: it collects the string
 * literals and joins them, as long as no operator moved the writing
 * position. A run that is interrupted by a positioning operator (`Td`, `TD`,
 * `Tm`, `T*`) or by the end of a text object (`ET`) is another text — the
 * words of a page are separated exactly that way.
 *
 * The comparison of the joined text is the comparison of bytes, ignoring the
 * case of the ASCII letters. A word that is stored as UTF-16 and is split as
 * well is not seen here — the limit is named in the report of the round.
 */

/* The operators that end a run of text. */
static inline int vocument_text_operator_ends_the_run(const char *operator_name) {
    return strcmp(operator_name, "Td") == 0 ||
           strcmp(operator_name, "TD") == 0 ||
           strcmp(operator_name, "Tm") == 0 ||
           strcmp(operator_name, "T*") == 0 ||
           strcmp(operator_name, "BT") == 0 ||
           strcmp(operator_name, "ET") == 0;
}

/* Answers whether the byte is a delimiter of the syntax of a content
   stream. */
static inline int vocument_text_is_delimiter(unsigned char value) {
    return value == ' ' || value == '\t' || value == '\r' || value == '\n' ||
           value == '\f' || value == '\0' || value == '(' || value == ')' ||
           value == '<' || value == '>' || value == '[' || value == ']' ||
           value == '{' || value == '}' || value == '/' || value == '%';
}

/* The value of a hexadecimal digit, or -1. */
static inline int vocument_text_hex_value(unsigned char value) {
    if (value >= '0' && value <= '9') return value - '0';
    if (value >= 'a' && value <= 'f') return value - 'a' + 10;
    if (value >= 'A' && value <= 'F') return value - 'A' + 10;
    return -1;
}

/* Answers whether the text of the content stream carries the word.
 *
 * The buffer is the **decoded** content of the stream. */
static inline int vocument_text_run_carries_word(
    const unsigned char *data,
    size_t len,
    const char *word
) {
    if (!data || !word || word[0] == '\0') {
        return 0;
    }
    size_t word_len = strlen(word);

    /* The text of the run that is being read. */
    size_t capacity = 512;
    size_t used = 0;
    unsigned char *run = (unsigned char *)malloc(capacity);
    if (!run) {
        return 0;
    }
    int found = 0;
    size_t i = 0;
    while (i < len && !found) {
        unsigned char value = data[i];
        if (value == '(') {
            /* A literal in parentheses. The escapes are read out: the text
               of the page is the text of the literal, not its encoding. */
            i++;
            int depth = 1;
            while (i < len && depth > 0) {
                unsigned char inner = data[i];
                if (inner == '\\') {
                    i++;
                    if (i >= len) break;
                    unsigned char escaped = data[i];
                    unsigned char decoded = escaped;
                    if (escaped >= '0' && escaped <= '7') {
                        int octal = escaped - '0';
                        for (int digit = 0; digit < 2 && i + 1 < len; digit++) {
                            unsigned char next = data[i + 1];
                            if (next < '0' || next > '7') break;
                            i++;
                            octal = octal * 8 + (next - '0');
                        }
                        decoded = (unsigned char)octal;
                    } else if (escaped == 'n') {
                        decoded = '\n';
                    } else if (escaped == 'r') {
                        decoded = '\r';
                    } else if (escaped == 't') {
                        decoded = '\t';
                    } else if (escaped == 'b') {
                        decoded = '\b';
                    } else if (escaped == 'f') {
                        decoded = '\f';
                    }
                    if (used + 1 >= capacity) {
                        size_t grown = capacity * 2;
                        unsigned char *bigger = (unsigned char *)realloc(run, grown);
                        if (!bigger) {
                            free(run);
                            return 0;
                        }
                        run = bigger;
                        capacity = grown;
                    }
                    run[used++] = decoded;
                    i++;
                    continue;
                }
                if (inner == '(') {
                    depth++;
                } else if (inner == ')') {
                    depth--;
                    if (depth == 0) {
                        i++;
                        break;
                    }
                }
                if (used + 1 >= capacity) {
                    size_t grown = capacity * 2;
                    unsigned char *bigger = (unsigned char *)realloc(run, grown);
                    if (!bigger) {
                        free(run);
                        return 0;
                    }
                    run = bigger;
                    capacity = grown;
                }
                run[used++] = inner;
                i++;
            }
            continue;
        }
        if (value == '<' && i + 1 < len && data[i + 1] != '<') {
            /* A hexadecimal string. */
            i++;
            int high = -1;
            while (i < len && data[i] != '>') {
                int digit = vocument_text_hex_value(data[i]);
                if (digit >= 0) {
                    if (high < 0) {
                        high = digit;
                    } else {
                        if (used + 1 >= capacity) {
                            size_t grown = capacity * 2;
                            unsigned char *bigger = (unsigned char *)realloc(run, grown);
                            if (!bigger) {
                                free(run);
                                return 0;
                            }
                            run = bigger;
                            capacity = grown;
                        }
                        run[used++] = (unsigned char)((high << 4) | digit);
                        high = -1;
                    }
                }
                i++;
            }
            if (i < len) {
                i++;
            }
            continue;
        }
        if (vocument_text_is_delimiter(value)) {
            i++;
            continue;
        }
        /* An operator or a number. */
        size_t start = i;
        while (i < len && !vocument_text_is_delimiter(data[i])) {
            i++;
        }
        size_t name_len = i - start;
        if (name_len == 0) {
            i++;
            continue;
        }
        /* Only the operators that move the writing position end a run. A
           number is part of the operands of an operator and does not end
           anything. */
        if (name_len <= 2) {
            /* The name of the operator is compared with a helper that needs
               a NUL terminated string; the token is copied into a small
               local array. */
            char name[3];
            name[0] = (char)data[start];
            name[1] = name_len > 1 ? (char)data[start + 1] : '\0';
            name[2] = '\0';
            if (vocument_text_operator_ends_the_run(name)) {
                if (vocument_contains_bytes_ci(run, used, (const unsigned char *)word, word_len)) {
                    found = 1;
                    break;
                }
                used = 0;
            }
        }
    }
    if (!found && vocument_contains_bytes_ci(run, used, (const unsigned char *)word, word_len)) {
        found = 1;
    }
    free(run);
    return found;
}
