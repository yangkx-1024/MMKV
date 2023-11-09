#include <stdarg.h>
#include <stdbool.h>
#include <stdint.h>
#include <stdlib.h>

typedef struct ByteSlice {
  const uint8_t *bytes;
  uintptr_t len;
} ByteSlice;

typedef struct NativeLogger {
  void *obj;
  void (*callback)(void *obj, int32_t level, struct ByteSlice content);
} NativeLogger;

typedef struct InternalError {
  int32_t code;
  const struct ByteSlice *reason;
} InternalError;

typedef struct VoidResult {
  const struct InternalError *err;
} VoidResult;

typedef const char *RawCStr;

typedef struct Result_ByteSlice {
  const struct ByteSlice *rawData;
  const struct InternalError *err;
} Result_ByteSlice;

typedef struct Result_i32 {
  const int32_t *rawData;
  const struct InternalError *err;
} Result_i32;

void initialize(const char *dir, struct NativeLogger logger);

void destroy_void_result(const struct VoidResult *ptr);

const struct VoidResult *put_str(RawCStr key, RawCStr value);

const struct Result_ByteSlice *get_str(RawCStr key);

void destroy_str_result(const struct Result_ByteSlice *ptr);

const struct VoidResult *put_i32(RawCStr key, int32_t value);

const struct Result_i32 *get_i32(RawCStr key);

void destroy_i32_result(const struct Result_i32 *ptr);
