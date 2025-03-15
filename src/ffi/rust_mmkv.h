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

struct ByteSlice {
  const uint8_t *bytes;
  uintptr_t len;
};

struct NativeLogger {
  void *obj;
  void (*callback)(void *obj, int32_t level, const struct ByteSlice *content);
  void (*destroy)(void *obj);
};

struct InternalError {
  int32_t code;
  const struct ByteSlice *reason;
};

struct RawBuffer {
  const void *raw_data;
  enum Types type_token;
  const struct InternalError *err;
};

typedef const char *RawCStr;

typedef const uint8_t *CByteArray;

typedef const int32_t *CI32Array;

typedef const int64_t *CI64Array;

typedef const float *CF32Array;

typedef const double *CF64Array;

struct RawTypedArray {
  const void *array;
  enum Types type_token;
  uintptr_t len;
};

const void *new_instance(const char *dir);

void set_logger(struct NativeLogger logger);

void set_log_level(int32_t log_level);

void free_buffer(const void *ptr);

void close_instance(const void *ptr);

void clear_data(const void *ptr);

const struct RawBuffer *delete(const void *ptr, RawCStr key);

const struct RawBuffer *put_str(const void *ptr, RawCStr key, RawCStr value);

const struct RawBuffer *put_bool(const void *ptr, RawCStr key, bool value);

const struct RawBuffer *put_i32(const void *ptr, RawCStr key, int32_t value);

const struct RawBuffer *put_i64(const void *ptr, RawCStr key, int64_t value);

const struct RawBuffer *put_f32(const void *ptr, RawCStr key, float value);

const struct RawBuffer *put_f64(const void *ptr, RawCStr key, double value);

const struct RawBuffer *get_str(const void *ptr, RawCStr key);

const struct RawBuffer *get_bool(const void *ptr, RawCStr key);

const struct RawBuffer *get_i32(const void *ptr, RawCStr key);

const struct RawBuffer *get_i64(const void *ptr, RawCStr key);

const struct RawBuffer *get_f32(const void *ptr, RawCStr key);

const struct RawBuffer *get_f64(const void *ptr, RawCStr key);

const struct RawBuffer *get_byte_array(const void *ptr, RawCStr key);

const struct RawBuffer *get_i32_array(const void *ptr, RawCStr key);

const struct RawBuffer *get_i64_array(const void *ptr, RawCStr key);

const struct RawBuffer *get_f32_array(const void *ptr, RawCStr key);

const struct RawBuffer *get_f64_array(const void *ptr, RawCStr key);

const struct RawBuffer *put_byte_array(const void *ptr,
                                       RawCStr key,
                                       CByteArray value,
                                       uintptr_t len);

const struct RawBuffer *put_i32_array(const void *ptr, RawCStr key, CI32Array value, uintptr_t len);

const struct RawBuffer *put_i64_array(const void *ptr, RawCStr key, CI64Array value, uintptr_t len);

const struct RawBuffer *put_f32_array(const void *ptr, RawCStr key, CF32Array value, uintptr_t len);

const struct RawBuffer *put_f64_array(const void *ptr, RawCStr key, CF64Array value, uintptr_t len);
