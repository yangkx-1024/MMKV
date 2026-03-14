use crate::Logger;
use crate::MMKV;
use jni::objects::{
    JByteArray, JClass, JDoubleArray, JFloatArray, JIntArray, JLongArray, JObject, JPrimitiveArray,
    JString, JValue, TypeArray,
};
use jni::refs::Global;
use jni::strings::JNIString;
use jni::sys::{
    jboolean, jbyteArray, jdouble, jdoubleArray, jfloat, jfloatArray, jint, jintArray, jlong,
    jlongArray, jstring,
};
use jni::{Env, EnvUnowned, JavaVM, jni_sig, jni_str};
use std::fmt::{Debug, Formatter};
use std::sync::OnceLock;

const LOG_TAG: &str = "MMKV:Android";

static ANDROID_LOGGER_CLASS_NAME: &str = "net/yangkx/mmkv/log/LoggerWrapper";
static ANDROID_NATIVE_EXCEPTION: &str = "net/yangkx/mmkv/NativeException";
static ANDROID_KEY_NOT_FOUND_EXCEPTION: &str = "net/yangkx/mmkv/KeyNotFoundException";

#[inline]
fn env_str(env: &mut Env<'_>, name: JString) -> jni::errors::Result<String> {
    name.try_to_string(env)
}

fn get_mmkv<'obj>(env: &mut Env<'_>, obj: &JObject<'obj>) -> jni::errors::Result<&'obj MMKV> {
    let j_value = env.get_field(obj, jni_str!("nativeObj"), jni_sig!("J"))?;
    let handle = j_value.j()?;
    if handle == 0 {
        Err(jni::errors::Error::NullPtr("invalid mmkv ptr"))
    } else {
        unsafe {
            // SAFETY: we assume ffi caller passed valid MMKV ptr
            Ok(&*(handle as *mut MMKV))
        }
    }
}

fn jprimary_array_to_native<T: TypeArray>(
    env: &Env<'_>,
    array: JPrimitiveArray<T>,
    default: T,
) -> jni::errors::Result<Vec<T>> {
    let len = array.len(env)?;
    let mut vec = Vec::new();
    vec.resize(len, default);
    array.get_region(env, 0, vec.as_mut_slice())?;
    Ok(vec)
}

macro_rules! native_to_jarray {
    ($env:expr, $value:expr, $new_op:tt) => {{
        let array = $env.$new_op($value.len())?;
        array.set_region($env, 0, $value.as_slice())?;
        array.into_raw()
    }};
}

trait JniPutValue {
    fn put_into(
        self,
        env: &mut Env<'_>,
        mmkv: &MMKV,
        key: &str,
    ) -> jni::errors::Result<crate::Result<()>>;
}

macro_rules! impl_jni_put_passthrough {
    ($($ty:ty),+ $(,)?) => {
        $(
            impl JniPutValue for $ty {
                fn put_into(
                    self,
                    _: &mut Env<'_>,
                    mmkv: &MMKV,
                    key: &str,
                ) -> jni::errors::Result<crate::Result<()>> {
                    Ok(mmkv.put(key, self))
                }
            }
        )+
    };
}

impl_jni_put_passthrough!(jint, jboolean, jlong, jfloat, jdouble);

impl<'local> JniPutValue for JString<'local> {
    fn put_into(
        self,
        env: &mut Env<'_>,
        mmkv: &MMKV,
        key: &str,
    ) -> jni::errors::Result<crate::Result<()>> {
        let value = env_str(env, self)?;
        Ok(mmkv.put(key, value.as_str()))
    }
}

macro_rules! impl_jni_put_primitive_array {
    ($jni_type:ty, $default:expr) => {
        impl<'local> JniPutValue for $jni_type {
            fn put_into(
                self,
                env: &mut Env<'_>,
                mmkv: &MMKV,
                key: &str,
            ) -> jni::errors::Result<crate::Result<()>> {
                let values = jprimary_array_to_native(env, self, $default)?;
                Ok(mmkv.put(key, values.as_slice()))
            }
        }
    };
}

impl<'local> JniPutValue for JByteArray<'local> {
    fn put_into(
        self,
        env: &mut Env<'_>,
        mmkv: &MMKV,
        key: &str,
    ) -> jni::errors::Result<crate::Result<()>> {
        let vec = jprimary_array_to_native(env, self, 0)?;
        let byte_array: Vec<u8> = vec.into_iter().map(|item| item as u8).collect();
        Ok(mmkv.put(key, byte_array.as_slice()))
    }
}

impl_jni_put_primitive_array!(JIntArray<'local>, 0);
impl_jni_put_primitive_array!(JLongArray<'local>, 0);
impl_jni_put_primitive_array!(JFloatArray<'local>, 0.0);
impl_jni_put_primitive_array!(JDoubleArray<'local>, 0.0);

trait JniGetKind {
    type JniType;
    type Stored;

    fn get_from(mmkv: &MMKV, key: &str) -> crate::Result<Self::Stored>;
    fn build_jni(env: &mut Env<'_>, value: Self::Stored) -> jni::errors::Result<Self::JniType>;
}

struct JniStringKind;
struct JniIntKind;
struct JniBoolKind;
struct JniLongKind;
struct JniFloatKind;
struct JniDoubleKind;
struct JniByteArrayKind;
struct JniIntArrayKind;
struct JniLongArrayKind;
struct JniFloatArrayKind;
struct JniDoubleArrayKind;

macro_rules! impl_jni_get_passthrough {
    ($($kind:ty, $jni_type:ty, $stored_type:ty),+ $(,)?) => {
        $(
            impl JniGetKind for $kind {
                type JniType = $jni_type;
                type Stored = $stored_type;

                fn get_from(mmkv: &MMKV, key: &str) -> crate::Result<Self::Stored> {
                    mmkv.get::<$stored_type>(key)
                }

                fn build_jni(
                    _: &mut Env<'_>,
                    value: Self::Stored,
                ) -> jni::errors::Result<Self::JniType> {
                    Ok(value as $jni_type)
                }
            }
        )+
    };
}

impl JniGetKind for JniStringKind {
    type JniType = jstring;
    type Stored = String;

    fn get_from(mmkv: &MMKV, key: &str) -> crate::Result<Self::Stored> {
        mmkv.get::<String>(key)
    }

    fn build_jni(env: &mut Env<'_>, value: Self::Stored) -> jni::errors::Result<Self::JniType> {
        Ok(env.new_string(&value)?.into_raw())
    }
}

impl_jni_get_passthrough!(
    JniIntKind,
    jint,
    i32,
    JniBoolKind,
    jboolean,
    bool,
    JniLongKind,
    jlong,
    i64,
    JniFloatKind,
    jfloat,
    f32,
    JniDoubleKind,
    jdouble,
    f64
);

macro_rules! impl_jni_get_array {
    ($kind:ty, $jni_type:ty, $stored_type:ty, $new_op:ident) => {
        impl JniGetKind for $kind {
            type JniType = $jni_type;
            type Stored = Vec<$stored_type>;

            fn get_from(mmkv: &MMKV, key: &str) -> crate::Result<Self::Stored> {
                mmkv.get::<Vec<$stored_type>>(key)
            }

            fn build_jni(
                env: &mut Env<'_>,
                value: Self::Stored,
            ) -> jni::errors::Result<Self::JniType> {
                let value: Vec<$stored_type> = value;
                Ok(native_to_jarray!(env, value, $new_op))
            }
        }
    };
}

impl JniGetKind for JniByteArrayKind {
    type JniType = jbyteArray;
    type Stored = Vec<u8>;

    fn get_from(mmkv: &MMKV, key: &str) -> crate::Result<Self::Stored> {
        mmkv.get::<Vec<u8>>(key)
    }

    fn build_jni(env: &mut Env<'_>, value: Self::Stored) -> jni::errors::Result<Self::JniType> {
        let vec: Vec<i8> = value.into_iter().map(|item| item as i8).collect();
        Ok(native_to_jarray!(env, vec, new_byte_array))
    }
}

impl_jni_get_array!(JniIntArrayKind, jintArray, i32, new_int_array);
impl_jni_get_array!(JniLongArrayKind, jlongArray, i64, new_long_array);
impl_jni_get_array!(JniFloatArrayKind, jfloatArray, f32, new_float_array);
impl_jni_get_array!(JniDoubleArrayKind, jdoubleArray, f64, new_double_array);

macro_rules! impl_java_put {
    ($name:ident, $value_type:tt, $log_type:literal) => {
        #[unsafe(no_mangle)]
        #[allow(non_snake_case)]
        pub unsafe extern "C" fn $name(
            mut env: EnvUnowned,
            obj: JObject,
            key: JString,
            value: $value_type,
        ) {
            let _ = env
                .with_env_no_catch(|env| -> jni::errors::Result<()> {
                    let mmkv = get_mmkv(env, &obj)?;
                    let key = env_str(env, key)?;
                    match value.put_into(env, &mmkv, &key)? {
                        Err(e) => {
                            let log_str = format!(
                                "failed to put {} for key {}, reason {:?}",
                                $log_type, key, e
                            );
                            error!(LOG_TAG, "{}", &log_str);
                            env.throw_new(
                                JNIString::from(ANDROID_NATIVE_EXCEPTION),
                                JNIString::from(log_str),
                            )
                        }
                        Ok(()) => {
                            verbose!(LOG_TAG, "put {} for key '{}' success", $log_type, key);
                            Ok(())
                        }
                    }
                })
                .into_outcome();
        }
    };
}

macro_rules! impl_java_get {
    ($name:ident, $value_type:tt, $get_kind:ty, $log_type:literal, $default:expr) => {
        #[unsafe(no_mangle)]
        #[allow(non_snake_case)]
        pub unsafe extern "C" fn $name(
            mut env: EnvUnowned,
            obj: JObject,
            key: JString,
        ) -> $value_type {
            let outcome = env
                .with_env_no_catch(|env| -> jni::errors::Result<$value_type> {
                    let mmkv = get_mmkv(env, &obj)?;
                    let key = env_str(env, key)?;
                    match <$get_kind as JniGetKind>::get_from(&mmkv, &key) {
                        Ok(value) => {
                            verbose!(LOG_TAG, "found {} with key '{}'", $log_type, key);
                            <$get_kind as JniGetKind>::build_jni(env, value)
                        }
                        Err(e) => {
                            let log_str = format!(
                                "get {} for key '{}' failed, reason: {:?}",
                                $log_type, key, e
                            );
                            error!(LOG_TAG, "{}", &log_str);
                            env.throw_new(
                                JNIString::from(ANDROID_KEY_NOT_FOUND_EXCEPTION),
                                JNIString::from(log_str),
                            )?;
                            unreachable!()
                        }
                    }
                })
                .into_outcome();
            match outcome {
                jni::Outcome::Ok(v) => v,
                _ => $default,
            }
        }
    };
}

macro_rules! for_each_mmkv_type {
    ($macro:ident) => {
        $macro!(
            Java_net_yangkx_mmkv_MMKV_putString,
            Java_net_yangkx_mmkv_MMKV_getString,
            JString,
            jstring,
            JniStringKind,
            "string",
            std::ptr::null_mut()
        );
        $macro!(
            Java_net_yangkx_mmkv_MMKV_putInt,
            Java_net_yangkx_mmkv_MMKV_getInt,
            jint,
            jint,
            JniIntKind,
            "int",
            0
        );
        $macro!(
            Java_net_yangkx_mmkv_MMKV_putBool,
            Java_net_yangkx_mmkv_MMKV_getBool,
            jboolean,
            jboolean,
            JniBoolKind,
            "bool",
            false
        );
        $macro!(
            Java_net_yangkx_mmkv_MMKV_putLong,
            Java_net_yangkx_mmkv_MMKV_getLong,
            jlong,
            jlong,
            JniLongKind,
            "long",
            0
        );
        $macro!(
            Java_net_yangkx_mmkv_MMKV_putFloat,
            Java_net_yangkx_mmkv_MMKV_getFloat,
            jfloat,
            jfloat,
            JniFloatKind,
            "float",
            0.0
        );
        $macro!(
            Java_net_yangkx_mmkv_MMKV_putDouble,
            Java_net_yangkx_mmkv_MMKV_getDouble,
            jdouble,
            jdouble,
            JniDoubleKind,
            "double",
            0.0
        );
        $macro!(
            Java_net_yangkx_mmkv_MMKV_putByteArray,
            Java_net_yangkx_mmkv_MMKV_getByteArray,
            JByteArray,
            jbyteArray,
            JniByteArrayKind,
            "byte array",
            std::ptr::null_mut()
        );
        $macro!(
            Java_net_yangkx_mmkv_MMKV_putIntArray,
            Java_net_yangkx_mmkv_MMKV_getIntArray,
            JIntArray,
            jintArray,
            JniIntArrayKind,
            "int array",
            std::ptr::null_mut()
        );
        $macro!(
            Java_net_yangkx_mmkv_MMKV_putLongArray,
            Java_net_yangkx_mmkv_MMKV_getLongArray,
            JLongArray,
            jlongArray,
            JniLongArrayKind,
            "long array",
            std::ptr::null_mut()
        );
        $macro!(
            Java_net_yangkx_mmkv_MMKV_putFloatArray,
            Java_net_yangkx_mmkv_MMKV_getFloatArray,
            JFloatArray,
            jfloatArray,
            JniFloatArrayKind,
            "float array",
            std::ptr::null_mut()
        );
        $macro!(
            Java_net_yangkx_mmkv_MMKV_putDoubleArray,
            Java_net_yangkx_mmkv_MMKV_getDoubleArray,
            JDoubleArray,
            jdoubleArray,
            JniDoubleArrayKind,
            "double array",
            std::ptr::null_mut()
        );
    };
}

macro_rules! define_java_accessors {
    (
        $put_name:ident,
        $get_name:ident,
        $put_type:tt,
        $get_type:tt,
        $get_kind:ty,
        $log_type:literal,
        $default:expr
    ) => {
        impl_java_put!($put_name, $put_type, $log_type);
        impl_java_get!($get_name, $get_type, $get_kind, $log_type, $default);
    };
}

static JAVA_LOGGER_CLASS: OnceLock<Global<JClass<'static>>> = OnceLock::new();

struct AndroidLogger {
    jvm: JavaVM,
}

impl AndroidLogger {
    fn new(jvm: JavaVM) -> jni::errors::Result<Self> {
        jvm.attach_current_thread(|env| -> jni::errors::Result<()> {
            let clz = env.find_class(JNIString::from(ANDROID_LOGGER_CLASS_NAME))?;
            let global_ref = env.new_global_ref(clz)?;
            let _ = JAVA_LOGGER_CLASS.set(global_ref);
            Ok(())
        })?;
        Ok(AndroidLogger { jvm })
    }

    fn call_java(&self, method: &str, param: String) {
        self.jvm
            .attach_current_thread(|env| -> jni::errors::Result<()> {
                let class = JAVA_LOGGER_CLASS
                    .get()
                    .ok_or(jni::errors::Error::NullPtr("Call attachLogger first"))?;
                let param = env.new_string(&param)?;
                env.call_static_method(
                    class,
                    JNIString::from(method),
                    jni_sig!("(Ljava/lang/String;)V"),
                    &[JValue::Object(&param)],
                )?;
                Ok(())
            })
            .unwrap();
    }
}

impl Debug for AndroidLogger {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("AndroidLogger").finish()
    }
}

impl Logger for AndroidLogger {
    fn verbose(&self, log_str: String) {
        self.call_java("verbose", log_str)
    }

    fn info(&self, log_str: String) {
        self.call_java("info", log_str)
    }

    fn debug(&self, log_str: String) {
        self.call_java("debug", log_str)
    }

    fn warn(&self, log_str: String) {
        self.call_java("warn", log_str)
    }

    fn error(&self, log_str: String) {
        self.call_java("error", log_str)
    }
}

#[unsafe(no_mangle)]
#[allow(non_snake_case)]
pub unsafe extern "C" fn Java_net_yangkx_mmkv_MMKV_attachLogger(mut env: EnvUnowned, _: JClass) {
    let _ = env
        .with_env_no_catch(|env| -> jni::errors::Result<()> {
            let jvm = env.get_java_vm()?;
            MMKV::set_logger(Box::new(AndroidLogger::new(jvm)?));
            Ok(())
        })
        .into_outcome();
}

#[unsafe(no_mangle)]
#[allow(non_snake_case)]
pub unsafe extern "C" fn Java_net_yangkx_mmkv_MMKV_initialize(
    mut env: EnvUnowned,
    _: JClass,
    dir: JString,
    #[cfg(feature = "encryption")] key: JString,
) -> jlong {
    let outcome = env
        .with_env_no_catch(|env| -> jni::errors::Result<jlong> {
            let path: String = dir.try_to_string(env)?;
            #[cfg(feature = "encryption")]
            let key: String = key.try_to_string(env)?;
            match MMKV::new(
                &path,
                #[cfg(feature = "encryption")]
                &key,
            ) {
                Ok(mmkv) => Ok(Box::into_raw(Box::new(mmkv)) as jlong),
                Err(e) => {
                    let log_str = format!(
                        "failed to initialize MMKV for path '{}', reason {:?}",
                        path, e
                    );
                    error!(LOG_TAG, "{}", &log_str);
                    env.throw_new(
                        JNIString::from(ANDROID_NATIVE_EXCEPTION),
                        JNIString::from(log_str),
                    )?;
                    unreachable!()
                }
            }
        })
        .into_outcome();
    match outcome {
        jni::Outcome::Ok(v) => v,
        _ => 0,
    }
}

for_each_mmkv_type!(define_java_accessors);

#[unsafe(no_mangle)]
#[allow(non_snake_case)]
pub unsafe extern "C" fn Java_net_yangkx_mmkv_MMKV_delete(
    mut env: EnvUnowned,
    obj: JObject,
    key: JString,
) {
    let _ = env
        .with_env_no_catch(|env| -> jni::errors::Result<()> {
            let mmkv = get_mmkv(env, &obj)?;
            let key = env_str(env, key)?;
            match mmkv.delete(&key) {
                Ok(()) => {
                    verbose!(LOG_TAG, "delete key {} success", &key);
                    Ok(())
                }
                Err(e) => {
                    let log_str = format!("failed to delete key {}, reason: {:?}", &key, e);
                    error!(LOG_TAG, "{}", &log_str);
                    env.throw_new(
                        JNIString::from(ANDROID_KEY_NOT_FOUND_EXCEPTION),
                        JNIString::from(log_str),
                    )
                }
            }
        })
        .into_outcome();
}

#[unsafe(no_mangle)]
#[allow(non_snake_case)]
pub unsafe extern "C" fn Java_net_yangkx_mmkv_MMKV_setLogLevel(
    mut env: EnvUnowned,
    _: JClass,
    level: jint,
) {
    let _ = env
        .with_env_no_catch(|env| -> jni::errors::Result<()> {
            if let Ok(level) = level.try_into() {
                MMKV::set_log_level(level);
                Ok(())
            } else {
                env.throw_new(
                    JNIString::from(ANDROID_NATIVE_EXCEPTION),
                    JNIString::from(format!("invalid log level '{}'", level)),
                )
            }
        })
        .into_outcome();
}

#[unsafe(no_mangle)]
#[allow(non_snake_case)]
pub unsafe extern "C" fn Java_net_yangkx_mmkv_MMKV_clearData(mut env: EnvUnowned, obj: JObject) {
    let _ = env
        .with_env_no_catch(|env| -> jni::errors::Result<()> {
            let mmkv = get_mmkv(env, &obj)?;
            match mmkv.clear_data() {
                Ok(_) => Ok(()),
                Err(e) => {
                    let log_str = format!("failed to clear data, reason: {:?}", e);
                    error!(LOG_TAG, "{}", &log_str);
                    env.throw_new(
                        JNIString::from(ANDROID_NATIVE_EXCEPTION),
                        JNIString::from(log_str),
                    )
                }
            }
        })
        .into_outcome();
}

#[unsafe(no_mangle)]
#[allow(non_snake_case)]
pub unsafe extern "C" fn Java_net_yangkx_mmkv_MMKV_close(_: EnvUnowned, _: JClass, ptr: jlong) {
    let ptr = ptr as *mut MMKV;
    unsafe {
        // SAFETY: we assume ffi caller passed valid MMKV ptr
        // Drop instance
        let _ = Box::from_raw(ptr);
    }
}
