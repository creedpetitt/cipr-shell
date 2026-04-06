#include "runtime.h"
#include "vendor/klib/kvec.h"
#include "vendor/klib/khash.h"
#include <stdlib.h>
#include <string.h>

// --- VECTORS ---

typedef kvec_t(int64_t) int_vec_t;
typedef kvec_t(cipr_string_t*) str_vec_t;

void* cipr_int_vec_new(void) {
    int_vec_t *v = (int_vec_t*)malloc(sizeof(int_vec_t));
    kv_init(*v);
    return (void*)v;
}

void cipr_int_vec_push(void *vec, int64_t val) {
    int_vec_t *v = (int_vec_t*)vec;
    kv_push(int64_t, *v, val);
}

int64_t cipr_int_vec_get(void *vec, int64_t idx) {
    int_vec_t *v = (int_vec_t*)vec;
    if (idx < 0 || (size_t)idx >= kv_size(*v)) {
        cipr_runtime_oob(idx, (int64_t)kv_size(*v));
    }
    return kv_A(*v, idx);
}

void cipr_int_vec_set(void *vec, int64_t idx, int64_t val) {
    int_vec_t *v = (int_vec_t*)vec;
    if (idx < 0 || (size_t)idx >= kv_size(*v)) {
        cipr_runtime_oob(idx, (int64_t)kv_size(*v));
    }
    kv_A(*v, idx) = val;
}

int64_t cipr_int_vec_len(void *vec) {
    int_vec_t *v = (int_vec_t*)vec;
    return (int64_t)kv_size(*v);
}

void cipr_int_vec_free(void *vec) {
    if (!vec) return;
    int_vec_t *v = (int_vec_t*)vec;
    kv_destroy(*v);
    free(v);
}

void* cipr_str_vec_new(void) {
    str_vec_t *v = (str_vec_t*)malloc(sizeof(str_vec_t));
    kv_init(*v);
    return (void*)v;
}

void cipr_str_vec_push(void *vec, cipr_string_t *val) {
    str_vec_t *v = (str_vec_t*)vec;
    kv_push(cipr_string_t*, *v, val);
}

cipr_str_t cipr_str_vec_get(void *vec, int64_t idx) {
    str_vec_t *v = (str_vec_t*)vec;
    if (idx < 0 || (size_t)idx >= kv_size(*v)) {
        cipr_runtime_oob(idx, (int64_t)kv_size(*v));
    }
    cipr_string_t *s = kv_A(*v, idx);
    if (!s) return cipr_empty_str();
    return s->view;
}

void cipr_str_vec_set(void *vec, int64_t idx, cipr_string_t *val) {
    str_vec_t *v = (str_vec_t*)vec;
    if (idx < 0 || (size_t)idx >= kv_size(*v)) {
        cipr_runtime_oob(idx, (int64_t)kv_size(*v));
    }
    cipr_string_free(kv_A(*v, idx));
    kv_A(*v, idx) = val;
}

int64_t cipr_str_vec_len(void *vec) {
    str_vec_t *v = (str_vec_t*)vec;
    return (int64_t)kv_size(*v);
}

void cipr_str_vec_free(void *vec) {
    if (!vec) return;
    str_vec_t *v = (str_vec_t*)vec;
    for (size_t i = 0; i < kv_size(*v); ++i) {
        cipr_string_free(kv_A(*v, i));
    }
    kv_destroy(*v);
    free(v);
}


// --- MAPS ---

KHASH_MAP_INIT_STR(str_int, int64_t)
KHASH_MAP_INIT_STR(str_str, cipr_string_t*)

void* cipr_str_int_map_new(void) {
    return (void*)kh_init(str_int);
}

void cipr_str_int_map_put(void *map, cipr_str_t key, int64_t val) {
    khash_t(str_int) *h = (khash_t(str_int)*)map;
    int ret;
    // Always use a fresh copy as the key in the table for safety
    khiter_t k = kh_put(str_int, h, key.data, &ret);
    if (ret != 0) { // New key was inserted
        char *key_copy = malloc((size_t)key.len + 1);
        memcpy(key_copy, key.data, (size_t)key.len + 1);
        kh_key(h, k) = key_copy;
    }
    kh_value(h, k) = val;
}

int64_t cipr_str_int_map_get(void *map, cipr_str_t key) {
    khash_t(str_int) *h = (khash_t(str_int)*)map;
    khiter_t k = kh_get(str_int, h, key.data);
    return (k == kh_end(h)) ? 0 : kh_value(h, k);
}

int64_t cipr_str_int_map_contains(void *map, cipr_str_t key) {
    khash_t(str_int) *h = (khash_t(str_int)*)map;
    khiter_t k = kh_get(str_int, h, key.data);
    return (k != kh_end(h)) ? 1 : 0;
}

void cipr_str_int_map_remove(void *map, cipr_str_t key) {
    khash_t(str_int) *h = (khash_t(str_int)*)map;
    khiter_t k = kh_get(str_int, h, key.data);
    if (k != kh_end(h)) {
        free((void*)kh_key(h, k)); // Clean up the copied key
        kh_del(str_int, h, k);
    }
}

void cipr_str_int_map_free(void *map) {
    if (!map) return;
    khash_t(str_int) *h = (khash_t(str_int)*)map;
    for (khiter_t k = kh_begin(h); k != kh_end(h); ++k) {
        if (kh_exist(h, k)) free((void*)kh_key(h, k));
    }
    kh_destroy(str_int, h);
}


void* cipr_str_str_map_new(void) {
    return (void*)kh_init(str_str);
}

void cipr_str_str_map_put(void *map, cipr_str_t key, cipr_string_t *val) {
    khash_t(str_str) *h = (khash_t(str_str)*)map;
    int ret;
    khiter_t k = kh_put(str_str, h, key.data, &ret);
    if (ret != 0) { // New key
        char *key_copy = malloc((size_t)key.len + 1);
        memcpy(key_copy, key.data, (size_t)key.len + 1);
        kh_key(h, k) = key_copy;
    } else { // Overwrite: free old value data
        cipr_string_free(kh_value(h, k));
    }
    kh_value(h, k) = val;
}

cipr_str_t cipr_str_str_map_get(void *map, cipr_str_t key) {
    khash_t(str_str) *h = (khash_t(str_str)*)map;
    khiter_t k = kh_get(str_str, h, key.data);
    if (k == kh_end(h)) return (cipr_str_t){ .len = 0, .data = "" };
    if (!kh_value(h, k)) return cipr_empty_str();
    return kh_value(h, k)->view;
}

int64_t cipr_str_str_map_contains(void *map, cipr_str_t key) {
    khash_t(str_str) *h = (khash_t(str_str)*)map;
    khiter_t k = kh_get(str_str, h, key.data);
    return (k != kh_end(h)) ? 1 : 0;
}

void cipr_str_str_map_remove(void *map, cipr_str_t key) {
    khash_t(str_str) *h = (khash_t(str_str)*)map;
    khiter_t k = kh_get(str_str, h, key.data);
    if (k != kh_end(h)) {
        free((void*)kh_key(h, k));
        cipr_string_free(kh_value(h, k));
        kh_del(str_str, h, k);
    }
}

void cipr_str_str_map_free(void *map) {
    if (!map) return;
    khash_t(str_str) *h = (khash_t(str_str)*)map;
    for (khiter_t k = kh_begin(h); k != kh_end(h); ++k) {
        if (kh_exist(h, k)) {
            free((void*)kh_key(h, k));
            cipr_string_free(kh_value(h, k));
        }
    }
    kh_destroy(str_str, h);
}
