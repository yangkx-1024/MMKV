#include <stdarg.h>
#include <stdbool.h>
#include <stdint.h>
#include <stdlib.h>

void initialize(const char *dir);

void put_i32(const char *key, int32_t value);

int get_i32(const char *key);
