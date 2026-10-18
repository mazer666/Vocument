#include "tesseract_bridge.h"
#include <stdlib.h>
#include <string.h>

typedef struct {
    char* languages;
    int width;
    int height;
    int bpp;
    int stride;
    unsigned char* pixels;
} StubEngine;

VocumentOcrEngineHandle vocument_ocr_create(const char* datapath, const char* languages) {
    (void)datapath;
    StubEngine* engine = (StubEngine*)calloc(1, sizeof(StubEngine));
    if (engine && languages) {
        engine->languages = strdup(languages);
    }
    return (VocumentOcrEngineHandle)engine;
}

int vocument_ocr_set_image(
    VocumentOcrEngineHandle handle,
    const unsigned char* pixels,
    int width,
    int height,
    int bytes_per_pixel,
    int bytes_per_line
) {
    StubEngine* engine = (StubEngine*)handle;
    if (!engine) {
        return -1;
    }
    engine->width = width;
    engine->height = height;
    engine->bpp = bytes_per_pixel;
    engine->stride = bytes_per_line;
    return 0;
}

int vocument_ocr_recognize(VocumentOcrEngineHandle handle) {
    StubEngine* engine = (StubEngine*)handle;
    if (!engine) {
        return -1;
    }
    return 0;
}

char* vocument_ocr_get_text(VocumentOcrEngineHandle handle) {
    StubEngine* engine = (StubEngine*)handle;
    if (!engine) {
        return NULL;
    }
    const char* sample = "VEREINBARUNG ÜBER VERTRAULICHKEIT UND GEHEIMHALTUNG\n\n§ 1 Geltungsbereich und vertrauliche Informationen\nDie Vertragsparteien vereinbaren die strikte Geheimhaltung.";
    return strdup(sample);
}

int vocument_ocr_get_mean_confidence(VocumentOcrEngineHandle handle) {
    StubEngine* engine = (StubEngine*)handle;
    return engine ? 94 : 0;
}

char* vocument_ocr_get_hocr(VocumentOcrEngineHandle handle, int page_num) {
    (void)page_num;
    StubEngine* engine = (StubEngine*)handle;
    if (!engine) {
        return NULL;
    }
    const char* hocr_sample = "<div class='ocr_page' id='page_1' title='image \"frame\"; bbox 0 0 800 600; ppageno 0'>\n"
                              "<div class='ocr_carea' id='block_1_1'>\n"
                              "<p class='ocr_par' id='par_1_1'>\n"
                              "<span class='ocr_line' id='line_1_1' title='bbox 50 50 400 80'>\n"
                              "<span class='ocrx_word' id='word_1_1' title='bbox 50 50 200 80; x_wconf 95'>VEREINBARUNG</span>\n"
                              "</span></p></div></div>";
    return strdup(hocr_sample);
}

int vocument_ocr_iterate_results(
    VocumentOcrEngineHandle handle,
    VocumentWordCallback word_cb,
    VocumentSymbolCallback symbol_cb,
    void* user_data
) {
    StubEngine* engine = (StubEngine*)handle;
    if (!engine) {
        return -1;
    }
    if (word_cb) {
        word_cb("VEREINBARUNG", 50, 50, 200, 80, 95.0f, 1, 1, 1, user_data);
        word_cb("VERTRAULICHKEIT", 210, 50, 380, 80, 94.0f, 1, 1, 2, user_data);
    }
    if (symbol_cb) {
        symbol_cb("V", 50, 50, 65, 80, 96.0f, 1, 1, 1, user_data);
        symbol_cb("E", 66, 50, 80, 80, 95.0f, 1, 1, 1, user_data);
    }
    return 0;
}

void vocument_ocr_free_string(char* str) {
    if (str) {
        free(str);
    }
}

void vocument_ocr_destroy(VocumentOcrEngineHandle handle) {
    StubEngine* engine = (StubEngine*)handle;
    if (engine) {
        if (engine->languages) {
            free(engine->languages);
        }
        if (engine->pixels) {
            free(engine->pixels);
        }
        free(engine);
    }
}
