#ifndef VOCUMENT_TESSERACT_BRIDGE_H
#define VOCUMENT_TESSERACT_BRIDGE_H

#ifdef __cplusplus
extern "C" {
#endif

typedef void* VocumentOcrEngineHandle;

typedef void (*VocumentWordCallback)(
    const char* text,
    int x0, int y0, int x1, int y1,
    float confidence,
    int block_num,
    int line_num,
    int word_num,
    void* user_data
);

typedef void (*VocumentSymbolCallback)(
    const char* text,
    int x0, int y0, int x1, int y1,
    float confidence,
    int block_num,
    int line_num,
    int word_num,
    void* user_data
);

VocumentOcrEngineHandle vocument_ocr_create(const char* datapath, const char* languages);

int vocument_ocr_set_image(
    VocumentOcrEngineHandle handle,
    const unsigned char* pixels,
    int width,
    int height,
    int bytes_per_pixel,
    int bytes_per_line
);

int vocument_ocr_recognize(VocumentOcrEngineHandle handle);

char* vocument_ocr_get_text(VocumentOcrEngineHandle handle);

int vocument_ocr_get_mean_confidence(VocumentOcrEngineHandle handle);

char* vocument_ocr_get_hocr(VocumentOcrEngineHandle handle, int page_num);

int vocument_ocr_iterate_results(
    VocumentOcrEngineHandle handle,
    VocumentWordCallback word_cb,
    VocumentSymbolCallback symbol_cb,
    void* user_data
);

void vocument_ocr_free_string(char* str);

void vocument_ocr_destroy(VocumentOcrEngineHandle handle);

#ifdef __cplusplus
}
#endif

#endif // VOCUMENT_TESSERACT_BRIDGE_H
