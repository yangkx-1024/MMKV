#include <stdarg.h>
#include <stdbool.h>
#include <stdint.h>
#include <stdlib.h>

enum Types {
  I32,
  STR,
  BYTE,
  I64,
  F32,
  F64,
  ByteArray,
  I32Array,
  I64Array,
  F32Array,
  F64Array,
};

struct RawTypedArray {
  const void *array;
  enum Types type_token;
  uintptr_t len;
};

struct ByteSlice {
  const uint8_t *bytes;
  uintptr_t len;
};

struct NativeLogger {
  void *obj;
  void (*callback)(void *obj, int32_t level, const struct ByteSlice *content);
};

struct InternalError {
  int32_t code;
  const struct ByteSlice *reason;
};

struct RawBuffer {
  const void *rawData;
  enum Types typeToken;
  const struct InternalError *err;
};

typedef const char *RawCStr;

typedef const int32_t *Int32Array;

void __use_typed_array(struct RawTypedArray typed_array);

void initialize(const char *dir, struct NativeLogger logger);

void free_buffer(const void *ptr);

const struct RawBuffer *put_str(RawCStr key, RawCStr value);

const struct RawBuffer *get_str(RawCStr key);

const struct RawBuffer *put_i32(RawCStr key, int32_t value);

const struct RawBuffer *get_i32(RawCStr key);

const struct RawBuffer *put_i32_array(RawCStr key, Int32Array value, uintptr_t len);

const struct RawBuffer *get_i32_array(RawCStr key);
