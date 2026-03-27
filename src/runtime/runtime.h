#ifndef CIPR_RUNTIME_H
#define CIPR_RUNTIME_H

#include <stdint.h>

// -----------------------------------------------------------------------------
// Cipr Core Types
// -----------------------------------------------------------------------------

// This struct must perfectly match the LLVM StructType generated in codegen.rs
// { i64 length, i8* ptr }
typedef struct {
    int64_t len;
    const char* data;
} cipr_str_t;

// -----------------------------------------------------------------------------
// Core Native Functions (cipr_ prefix)
// -----------------------------------------------------------------------------

void cipr_print_str(cipr_str_t str);
void cipr_print_int(int64_t val);
void cipr_print_float(double val);
void cipr_print_bool(int val);

double cipr_time(void);

#endif // CIPR_RUNTIME_H
