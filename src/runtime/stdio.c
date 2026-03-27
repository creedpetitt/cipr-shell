#include "runtime.h"
#include <stdio.h>

void cipr_print_str(cipr_str_t str) {
    printf("%.*s\n", (int)str.len, str.data);
}

void cipr_print_int(int64_t val) {
    printf("%lld\n", (long long)val);
}

void cipr_print_float(double val) {
    printf("%f\n", val);
}

void cipr_print_bool(int val) {
    printf("%s\n", val ? "true" : "false");
}
