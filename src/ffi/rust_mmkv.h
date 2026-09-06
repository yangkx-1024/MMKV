/*
 * MMKV C API.
 *
 * Handles: `new_instance` returns an opaque handle, or NULL when `dir` is not a writable
 * directory (the reason goes to the logger). Handles on the same directory share one
 * store and may be used from any thread. Release each handle with `close_instance`
 * exactly once and never use it afterwards.
 *
 * Results: every `put_*`, `get_*` and `delete` call returns a non-NULL `RawBuffer` the
 * caller owns and must pass to `free_buffer` exactly once. `err == NULL` means success.
 * Everything reachable from the buffer (value, error, reason) is owned by it.
 *
 * Errors are reported through `InternalError.code` (an `MMKV_ERR_*` constant), never by
 * aborting: a NULL handle, a NULL or non-UTF-8 key or value, and IO failures such as a
 * full disk all come back as codes. Passing NULL to `free_buffer`, `close_instance` or
 * `clear_data` is a no-op. Passing a pointer that did not come from this API, or
 * releasing one twice, is undefined behaviour.
 *
 * Logging: `set_logger` and `set_log_level` are process-wide. The logger's callbacks run
 * on MMKV's own logger thread, see `NativeLogger`.
 */

#include <stdarg.h>
#include <stdbool.h>
#include <stdint.h>
#include <stdlib.h>

/**
 * Error codes carried in `InternalError.code`.
 *
 * `0..=7` mirror the library's `Error` variants one to one; the negative codes are
 * argument errors caught at the C boundary before MMKV runs. Every code except
 * `MMKV_ERR_KEY_NOT_FOUND` comes with a human readable `reason`.
 */
#define MMKV_ERR_KEY_NOT_FOUND 0

#define MMKV_ERR_DECODE_FAILED 1

#define MMKV_ERR_TYPE_MISS_MATCH 2

#define MMKV_ERR_DATA_INVALID 3

#define MMKV_ERR_INSTANCE_CLOSED 4

#define MMKV_ERR_ENCODE_FAILED 5

/**
 * The data file could not be written, for example because the disk is full.
 */
#define MMKV_ERR_IO 6

/**
 * An internal lock was poisoned by a panic on another thread.
 */
#define MMKV_ERR_LOCK 7

/**
 * `key` is null or not valid UTF-8.
 */
#define MMKV_ERR_INVALID_KEY -1

/**
 * The instance pointer is null.
 */
#define MMKV_ERR_NULL_INSTANCE -2

/**
 * `value` is null with a non-zero `len`, or not valid UTF-8 for `put_str`.
 */
#define MMKV_ERR_INVALID_VALUE -3

/**
 * The value type a `RawBuffer` carries; also tags a `RawTypedArray`.
 */
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

/**
 * `len` bytes at `bytes`, not NUL-terminated. Owned by the `RawBuffer` or
 * `InternalError` it hangs off and freed with it; never free it directly.
 */
struct ByteSlice {
  const uint8_t *bytes;
  uintptr_t len;
  uintptr_t capacity;
};

/**
 * A log sink implemented by the host, installed with `set_logger`.
 *
 * MMKV writes its logs on a logger thread of its own, so `callback` and `destroy` run on
 * that thread, never on the thread that called into MMKV; both must be safe to call from
 * there. `content` is only valid for the duration of `callback`, copy it to keep it.
 * `destroy` runs exactly once, when a later `set_logger` replaces this logger.
 */
struct NativeLogger {
  void *obj;
  void (*callback)(void *obj, int32_t level, const struct ByteSlice *content);
  void (*destroy)(void *obj);
};

/**
 * A failed call: `code` is one of the `MMKV_ERR_*` constants and `reason` is NULL or a
 * UTF-8 message, owned by the `RawBuffer`.
 */
struct InternalError {
  int32_t code;
  const struct ByteSlice *reason;
};

/**
 * The result of a `put_*`, `get_*` or `delete` call, released with `free_buffer`.
 *
 * `err == NULL` means success, and `raw_data` then points at the value for `get_*`: a
 * `ByteSlice` for `Str`, a `RawTypedArray` for the array types, the scalar itself
 * otherwise. `raw_data` is NULL for `put_*` and `delete`. Everything reachable from the
 * buffer is owned by it.
 */
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

/**
 * `len` elements of `type_token` at `array`. Owned by its `RawBuffer` and freed with it.
 */
struct RawTypedArray {
  const void *array;
  enum Types type_token;
  uintptr_t len;
  uintptr_t capacity;
};

/**
 * Open the store in `dir`, a writable directory, and return an opaque handle for it.
 *
 * Returns null when `dir` is null, not UTF-8 or not a writable directory; the reason
 * goes to the logger. Handles on the same directory share one store and may be used
 * from any thread. Release the handle with `close_instance` exactly once.
 */
const void *new_instance(const char *dir);

/**
 * Install `logger` as the process-wide log sink, taking ownership of it. See
 * `NativeLogger` for the thread its callbacks run on.
 */
void set_logger(struct NativeLogger logger);

/**
 * Set the process-wide log level: 0 off, 1 error, 2 warn, 3 info, 4 debug, 5 verbose.
 * Any other value is logged and ignored.
 */
void set_log_level(int32_t log_level);

/**
 * Release a `RawBuffer` returned by `put_*`, `get_*` or `delete`, together with the
 * value or error it owns. Null is a no-op; releasing a buffer twice is undefined.
 */
void free_buffer(const void *ptr);

/**
 * Close a handle from `new_instance`. Null is a no-op. The handle must not be used or
 * closed again afterwards; the store itself stays open while other handles exist.
 */
void close_instance(const void *ptr);

/**
 * Delete every record in the store behind `ptr` and keep it usable. A failure, or a
 * null `ptr`, is logged.
 */
void clear_data(const void *ptr);

/**
 * Delete `key`; deleting a missing key succeeds. Release the returned buffer with
 * `free_buffer`.
 */
const struct RawBuffer *delete(const void *ptr, RawCStr key);

/**
 * Store `value` under `key`. The returned buffer carries no data, only a
 * possible error; release it with `free_buffer`.
 */
const struct RawBuffer *put_str(const void *ptr, RawCStr key, RawCStr value);

/**
 * Store `value` under `key`. The returned buffer carries no data, only a
 * possible error; release it with `free_buffer`.
 */
const struct RawBuffer *put_bool(const void *ptr, RawCStr key, bool value);

/**
 * Store `value` under `key`. The returned buffer carries no data, only a
 * possible error; release it with `free_buffer`.
 */
const struct RawBuffer *put_i32(const void *ptr, RawCStr key, int32_t value);

/**
 * Store `value` under `key`. The returned buffer carries no data, only a
 * possible error; release it with `free_buffer`.
 */
const struct RawBuffer *put_i64(const void *ptr, RawCStr key, int64_t value);

/**
 * Store `value` under `key`. The returned buffer carries no data, only a
 * possible error; release it with `free_buffer`.
 */
const struct RawBuffer *put_f32(const void *ptr, RawCStr key, float value);

/**
 * Store `value` under `key`. The returned buffer carries no data, only a
 * possible error; release it with `free_buffer`.
 */
const struct RawBuffer *put_f64(const void *ptr, RawCStr key, double value);

/**
 * Read the value stored under `key`. On success the returned buffer owns the
 * value in `raw_data`, on failure `err` is set; release it with `free_buffer`.
 */
const struct RawBuffer *get_str(const void *ptr, RawCStr key);

/**
 * Read the value stored under `key`. On success the returned buffer owns the
 * value in `raw_data`, on failure `err` is set; release it with `free_buffer`.
 */
const struct RawBuffer *get_bool(const void *ptr, RawCStr key);

/**
 * Read the value stored under `key`. On success the returned buffer owns the
 * value in `raw_data`, on failure `err` is set; release it with `free_buffer`.
 */
const struct RawBuffer *get_i32(const void *ptr, RawCStr key);

/**
 * Read the value stored under `key`. On success the returned buffer owns the
 * value in `raw_data`, on failure `err` is set; release it with `free_buffer`.
 */
const struct RawBuffer *get_i64(const void *ptr, RawCStr key);

/**
 * Read the value stored under `key`. On success the returned buffer owns the
 * value in `raw_data`, on failure `err` is set; release it with `free_buffer`.
 */
const struct RawBuffer *get_f32(const void *ptr, RawCStr key);

/**
 * Read the value stored under `key`. On success the returned buffer owns the
 * value in `raw_data`, on failure `err` is set; release it with `free_buffer`.
 */
const struct RawBuffer *get_f64(const void *ptr, RawCStr key);

/**
 * Read the value stored under `key`. On success the returned buffer owns the
 * value in `raw_data`, on failure `err` is set; release it with `free_buffer`.
 */
const struct RawBuffer *get_byte_array(const void *ptr, RawCStr key);

/**
 * Read the value stored under `key`. On success the returned buffer owns the
 * value in `raw_data`, on failure `err` is set; release it with `free_buffer`.
 */
const struct RawBuffer *get_i32_array(const void *ptr, RawCStr key);

/**
 * Read the value stored under `key`. On success the returned buffer owns the
 * value in `raw_data`, on failure `err` is set; release it with `free_buffer`.
 */
const struct RawBuffer *get_i64_array(const void *ptr, RawCStr key);

/**
 * Read the value stored under `key`. On success the returned buffer owns the
 * value in `raw_data`, on failure `err` is set; release it with `free_buffer`.
 */
const struct RawBuffer *get_f32_array(const void *ptr, RawCStr key);

/**
 * Read the value stored under `key`. On success the returned buffer owns the
 * value in `raw_data`, on failure `err` is set; release it with `free_buffer`.
 */
const struct RawBuffer *get_f64_array(const void *ptr, RawCStr key);

/**
 * Store the `len` elements at `value` under `key`. The returned buffer carries
 * no data, only a possible error; release it with `free_buffer`.
 */
const struct RawBuffer *put_byte_array(const void *ptr,
                                       RawCStr key,
                                       CByteArray value,
                                       uintptr_t len);

/**
 * Store the `len` elements at `value` under `key`. The returned buffer carries
 * no data, only a possible error; release it with `free_buffer`.
 */
const struct RawBuffer *put_i32_array(const void *ptr, RawCStr key, CI32Array value, uintptr_t len);

/**
 * Store the `len` elements at `value` under `key`. The returned buffer carries
 * no data, only a possible error; release it with `free_buffer`.
 */
const struct RawBuffer *put_i64_array(const void *ptr, RawCStr key, CI64Array value, uintptr_t len);

/**
 * Store the `len` elements at `value` under `key`. The returned buffer carries
 * no data, only a possible error; release it with `free_buffer`.
 */
const struct RawBuffer *put_f32_array(const void *ptr, RawCStr key, CF32Array value, uintptr_t len);

/**
 * Store the `len` elements at `value` under `key`. The returned buffer carries
 * no data, only a possible error; release it with `free_buffer`.
 */
const struct RawBuffer *put_f64_array(const void *ptr, RawCStr key, CF64Array value, uintptr_t len);
