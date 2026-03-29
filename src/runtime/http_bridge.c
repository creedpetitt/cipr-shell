#include "runtime.h"

#include "http_config.h"
#include "vendor/akari/include/akari_http.h"

#include <stdio.h>
#include <stdlib.h>
#include <string.h>

static akari_context *cipr_current_http_ctx = NULL;

typedef struct {
    char method[8];
    char path[256];
    cipr_callable handler;
} cipr_http_callback;

static cipr_http_callback cipr_http_callbacks[CIPR_HTTP_MAX_CALLBACKS];
static int cipr_http_callback_count = 0;

static cipr_str_t cipr_empty_str(void) {
    return (cipr_str_t){ .len = 0, .data = "" };
}

static char *cipr_alloc_cstr(const char *data, size_t len) {
    char *out = malloc(len + 1);
    if (!out) {
        return NULL;
    }
    if (len > 0) {
        memcpy(out, data, len);
    }
    out[len] = '\0';
    return out;
}

static cipr_str_t cipr_owned_str(const char *data, size_t len) {
    if (!data) {
        return cipr_empty_str();
    }
    char *copy = cipr_alloc_cstr(data, len);
    if (!copy) {
        return cipr_empty_str();
    }
    return (cipr_str_t){ .len = (int64_t)len, .data = copy };
}

static int cipr_ctx_ready(void) {
    return cipr_current_http_ctx != NULL;
}

static void cipr_http_dispatch(akari_context *ctx) {
    cipr_current_http_ctx = ctx;
    for (int i = 0; i < cipr_http_callback_count; i++) {
        if ((size_t)ctx->method_len != strlen(cipr_http_callbacks[i].method)) {
            continue;
        }
        if (strncmp(ctx->method, cipr_http_callbacks[i].method, ctx->method_len) != 0) {
            continue;
        }
        if ((size_t)ctx->path_len != strlen(cipr_http_callbacks[i].path)) {
            continue;
        }
        if (strncmp(ctx->path, cipr_http_callbacks[i].path, ctx->path_len) != 0) {
            continue;
        }

        void (*fn)(void *) = (void (*)(void *))cipr_http_callbacks[i].handler.fn_ptr;
        fn(cipr_http_callbacks[i].handler.env_ptr);
        cipr_current_http_ctx = NULL;
        return;
    }

    akari_res_send(cipr_current_http_ctx, 404, "text/plain", "404 Route Not Found");
    cipr_current_http_ctx = NULL;
}

void cipr_http_start(int64_t port) {
    if (port <= 0 || port > 65535) {
        return;
    }

    akari_http_start((uint16_t)port);
}

void cipr_http_stop(void) {
    akari_stop();
}

cipr_str_t cipr_http_method(void) {
    if (!cipr_ctx_ready()) {
        return cipr_empty_str();
    }
    return cipr_owned_str(cipr_current_http_ctx->method, cipr_current_http_ctx->method_len);
}

cipr_str_t cipr_http_path(void) {
    if (!cipr_ctx_ready()) {
        return cipr_empty_str();
    }
    return cipr_owned_str(cipr_current_http_ctx->path, cipr_current_http_ctx->path_len);
}

cipr_str_t cipr_http_body(void) {
    if (!cipr_ctx_ready()) {
        return cipr_empty_str();
    }
    return cipr_owned_str(cipr_current_http_ctx->body, cipr_current_http_ctx->body_len);
}

cipr_str_t cipr_http_query(cipr_str_t key, cipr_str_t def) {
    if (!cipr_ctx_ready()) {
        return cipr_owned_str(def.data, (size_t)def.len);
    }

    char *key_c = cipr_alloc_cstr(key.data, (size_t)key.len);
    if (!key_c) {
        return cipr_owned_str(def.data, (size_t)def.len);
    }

    size_t out_len = 0;
    const char *val = akari_get_query_param(cipr_current_http_ctx, key_c, &out_len);
    free(key_c);
    if (!val) {
        return cipr_owned_str(def.data, (size_t)def.len);
    }
    return cipr_owned_str(val, out_len);
}

cipr_str_t cipr_http_param(cipr_str_t key, cipr_str_t def) {
    if (!cipr_ctx_ready()) {
        return cipr_owned_str(def.data, (size_t)def.len);
    }

    char *key_c = cipr_alloc_cstr(key.data, (size_t)key.len);
    if (!key_c) {
        return cipr_owned_str(def.data, (size_t)def.len);
    }

    size_t out_len = 0;
    const char *val = akari_get_path_param(cipr_current_http_ctx, key_c, &out_len);
    free(key_c);
    if (!val) {
        return cipr_owned_str(def.data, (size_t)def.len);
    }
    return cipr_owned_str(val, out_len);
}

void cipr_http_register(cipr_str_t method, cipr_str_t path, cipr_callable handler) {
    if (cipr_http_callback_count >= CIPR_HTTP_MAX_CALLBACKS) {
        return;
    }
    if (method.len <= 0 || path.len <= 0) {
        return;
    }

    cipr_http_callback *slot = &cipr_http_callbacks[cipr_http_callback_count];
    size_t mlen = (size_t)method.len;
    size_t plen = (size_t)path.len;
    if (mlen >= sizeof(slot->method)) {
        mlen = sizeof(slot->method) - 1;
    }
    if (plen >= sizeof(slot->path)) {
        plen = sizeof(slot->path) - 1;
    }
    memcpy(slot->method, method.data, mlen);
    slot->method[mlen] = '\0';
    memcpy(slot->path, path.data, plen);
    slot->path[plen] = '\0';
    slot->handler = handler;
    cipr_http_callback_count++;

    akari_http_add_route(slot->method, slot->path, cipr_http_dispatch);
}

void cipr_http_send(int64_t status, cipr_str_t content_type, cipr_str_t body) {
    if (!cipr_ctx_ready()) {
        return;
    }

    char *content_type_c = cipr_alloc_cstr(content_type.data, (size_t)content_type.len);
    if (!content_type_c) {
        return;
    }
    akari_res_data(cipr_current_http_ctx, (int)status, content_type_c, body.data, (size_t)body.len);
    free(content_type_c);
}

void cipr_http_json(int64_t status, cipr_str_t body) {
    if (!cipr_ctx_ready()) {
        return;
    }
    akari_res_data(cipr_current_http_ctx, (int)status, "application/json", body.data, (size_t)body.len);
}

void cipr_http_file(cipr_str_t path) {
    if (!cipr_ctx_ready()) {
        return;
    }

    char *path_c = cipr_alloc_cstr(path.data, (size_t)path.len);
    if (!path_c) {
        return;
    }
    akari_res_file(cipr_current_http_ctx, path_c);
    free(path_c);
}
