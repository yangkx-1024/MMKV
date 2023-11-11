#include <stdarg.h>
#include <stdbool.h>
#include <stdint.h>
#include <stdlib.h>

enum Types {
  I32,
  Str,
  Bool,
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

typedef const uint8_t *CByteArray;

typedef const int32_t *CI32Array;

typedef const int64_t *CI64Array;

typedef const float *CF32Array;

typedef const double *CF64Array;

void __use_typed_array(struct RawTypedArray typed_array);

void initialize(const char *dir, struct NativeLogger logger);

void free_buffer(const void *ptr);

void close_instance(void);

void clear_data(void);

const struct RawBuffer *put_str(RawCStr key, RawCStr value);

const struct RawBuffer *get_str(RawCStr key);

const struct RawBuffer *put_bool(RawCStr key, bool value);

const struct RawBuffer *get_bool(RawCStr key);

const struct RawBuffer *put_i32(RawCStr key, int32_t value);

const struct RawBuffer *get_i32(RawCStr key);

const struct RawBuffer *put_i64(RawCStr key, int64_t value);

const struct RawBuffer *get_i64(RawCStr key);

const struct RawBuffer *put_f32(RawCStr key, float value);

const struct RawBuffer *get_f32(RawCStr key);

const struct RawBuffer *put_f64(RawCStr key, double value);

const struct RawBuffer *get_f64(RawCStr key);

const struct RawBuffer *put_byte_array(RawCStr key, CByteArray value, uintptr_t len);

const struct RawBuffer *get_byte_array(RawCStr key);

const struct RawBuffer *put_i32_array(RawCStr key, CI32Array value, uintptr_t len);

const struct RawBuffer *get_i32_array(RawCStr key);

const struct RawBuffer *put_i64_array(RawCStr key, CI64Array value, uintptr_t len);

const struct RawBuffer *get_i64_array(RawCStr key);

const struct RawBuffer *put_f32_array(RawCStr key, CF32Array value, uintptr_t len);

const struct RawBuffer *get_f32_array(RawCStr key);

const struct RawBuffer *put_f64_array(RawCStr key, CF64Array value, uintptr_t len);

const struct RawBuffer *get_f64_array(RawCStr key);
