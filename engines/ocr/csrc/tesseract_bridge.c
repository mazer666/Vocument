#include "tesseract_bridge.h"
#include <tesseract/capi.h>
#include <stdlib.h>
#include <string.h>

VocumentOcrEngineHandle vocument_ocr_create(const char* datapath, const char* languages) {
    TessBaseAPI* api = TessBaseAPICreate();
    if (!api) {
        return NULL;
    }
    int res = TessBaseAPIInit3(api, datapath, languages);
    if (res != 0) {
        TessBaseAPIDelete(api);
        return NULL;
    }
    return (VocumentOcrEngineHandle)api;
}

int vocument_ocr_set_image(
    VocumentOcrEngineHandle handle,
    const unsigned char* pixels,
    int width,
    int height,
    int bytes_per_pixel,
    int bytes_per_line
) {
    if (!handle || !pixels || width <= 0 || height <= 0) {
        return -1;
    }
    TessBaseAPI* api = (TessBaseAPI*)handle;
    TessBaseAPISetImage(api, pixels, width, height, bytes_per_pixel, bytes_per_line);
    return 0;
}

int vocument_ocr_recognize(VocumentOcrEngineHandle handle) {
    if (!handle) {
        return -1;
    }
    TessBaseAPI* api = (TessBaseAPI*)handle;
    return TessBaseAPIRecognize(api, NULL);
}

char* vocument_ocr_get_text(VocumentOcrEngineHandle handle) {
    if (!handle) {
        return NULL;
    }
    TessBaseAPI* api = (TessBaseAPI*)handle;
    return TessBaseAPIGetUTF8Text(api);
}

int vocument_ocr_get_mean_confidence(VocumentOcrEngineHandle handle) {
    if (!handle) {
        return 0;
    }
    TessBaseAPI* api = (TessBaseAPI*)handle;
    return TessBaseAPIMeanTextConf(api);
}

char* vocument_ocr_get_hocr(VocumentOcrEngineHandle handle, int page_num) {
    if (!handle) {
        return NULL;
    }
    TessBaseAPI* api = (TessBaseAPI*)handle;
    return TessBaseAPIGetHOCRText(api, page_num);
}

int vocument_ocr_iterate_results(
    VocumentOcrEngineHandle handle,
    VocumentWordCallback word_cb,
    VocumentSymbolCallback symbol_cb,
    void* user_data
) {
    if (!handle) {
        return -1;
    }
    TessBaseAPI* api = (TessBaseAPI*)handle;
    TessResultIterator* it = TessBaseAPIGetIterator(api);
    if (!it) {
        return 0;
    }
    TessPageIterator* pi = (TessPageIterator*)it;
    TessPageIteratorBegin(pi);

    int block_num = 0;
    int line_num = 0;
    int word_num = 0;

    do {
        if (TessPageIteratorIsAtBeginningOf(pi, RIL_BLOCK)) {
            block_num++;
            line_num = 0;
            word_num = 0;
        }
        if (TessPageIteratorIsAtBeginningOf(pi, RIL_TEXTLINE)) {
            line_num++;
            word_num = 0;
        }
        if (TessPageIteratorIsAtBeginningOf(pi, RIL_WORD)) {
            word_num++;
            int wl = 0, wt = 0, wr = 0, wb = 0;
            TessPageIteratorBoundingBox(pi, RIL_WORD, &wl, &wt, &wr, &wb);
            char* word_text = TessResultIteratorGetUTF8Text(it, RIL_WORD);
            float word_conf = TessResultIteratorConfidence(it, RIL_WORD);
            if (word_cb && word_text) {
                word_cb(word_text, wl, wt, wr, wb, word_conf, block_num, line_num, word_num, user_data);
            }
            if (word_text) {
                TessDeleteText(word_text);
            }
        }

        int sl = 0, st = 0, sr = 0, sb = 0;
        TessPageIteratorBoundingBox(pi, RIL_SYMBOL, &sl, &st, &sr, &sb);
        char* sym_text = TessResultIteratorGetUTF8Text(it, RIL_SYMBOL);
        float sym_conf = TessResultIteratorConfidence(it, RIL_SYMBOL);
        if (symbol_cb && sym_text) {
            symbol_cb(sym_text, sl, st, sr, sb, sym_conf, block_num, line_num, word_num, user_data);
        }
        if (sym_text) {
            TessDeleteText(sym_text);
        }
    } while (TessPageIteratorNext(pi, RIL_SYMBOL));

    TessResultIteratorDelete(it);
    return 0;
}

void vocument_ocr_free_string(char* str) {
    if (str) {
        TessDeleteText(str);
    }
}

void vocument_ocr_destroy(VocumentOcrEngineHandle handle) {
    if (handle) {
        TessBaseAPI* api = (TessBaseAPI*)handle;
        TessBaseAPIEnd(api);
        TessBaseAPIDelete(api);
    }
}
