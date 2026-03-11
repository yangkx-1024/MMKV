use crate::Logger;
use crate::MMKV;
use jni::objects::{
    JByteArray, JClass, JDoubleArray, JFloatArray, JIntArray, JLongArray, JPrimitiveArray, JString,
    JValue, TypeArray,
};
use jni::refs::Global;
use jni::strings::JNIString;
use jni::sys::{
    jboolean, jbyteArray, jdouble, jdoubleArray, jfloat, jfloatArray, jint, jintArray, jlong,
    jlongArray, jstring,
};
use jni::{jni_sig, jni_str, Env, EnvUnowned, JavaVM};
use std::fmt::{Debug, Formatter};
use std::sync::RwLock;

const LOG_TAG: &str = "MMKV:Android";

static ANDROID_LOGGER_CLASS_NAME: &str = "net/yangkx/mmkv/log/LoggerWrapper";
static ANDROID_NATIVE_EXCEPTION: &str = "net/yangkx/mmkv/NativeException";
static ANDROID_KEY_NOT_FOUND_EXCEPTION: &str = "net/yangkx/mmkv/KeyNotFoundException";

#[inline]
fn env_str(env: &mut Env<'_>, name: JString) -> jni::errors::Result<String> {
    name.try_to_string(env)
}

fn get_mmkv<'obj>(env: &mut Env<'_>, obj: &JClass<'obj>) -> jni::errors::Result<&'obj MMKV> {
    let j_value = env.get_field(obj, jni_str!("nativeObj"), jni_sig!("J"))?;
    let j = j_value.j()?;
    let ptr = j as *mut MMKV;
    unsafe {
        // SAFETY: we assume ffi caller passed valid MMKV ptr
        Ok(ptr.as_ref().unwrap())
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

macro_rules! mmkv_put {
    ($env:expr, $mmkv:ident, $key:expr, $value:expr, JString) => {
        $mmkv.put(&$key, env_str($env, $value)?.as_str())
    };
    ($env:expr, $mmkv:ident, $key:expr, $value:expr, jint) => {
        $mmkv.put(&$key, $value)
    };
    ($env:expr, $mmkv:ident, $key:expr, $value:expr, jboolean) => {
        $mmkv.put(&$key, $value)
    };
    ($env:expr, $mmkv:ident, $key:expr, $value:expr, jlong) => {
        $mmkv.put(&$key, $value)
    };
    ($env:expr, $mmkv:ident, $key:expr, $value:expr, jfloat) => {
        $mmkv.put(&$key, $value)
    };
    ($env:expr, $mmkv:ident, $key:expr, $value:expr, jdouble) => {
        $mmkv.put(&$key, $value)
    };
    ($env:expr, $mmkv:ident, $key:expr, $value:expr, JByteArray) => {{
        let vec = jprimary_array_to_native($env, $value, 0)?;
        let byte_array: Vec<u8> = vec.into_iter().map(|item| item as u8).collect();
        $mmkv.put(&$key, byte_array.as_slice())
    }};
    ($env:expr, $mmkv:ident, $key:expr, $value:expr, JIntArray) => {
        $mmkv.put(&$key, jprimary_array_to_native($env, $value, 0)?.as_slice())
    };
    ($env:expr, $mmkv:ident, $key:expr, $value:expr, JLongArray) => {
        $mmkv.put(&$key, jprimary_array_to_native($env, $value, 0)?.as_slice())
    };
    ($env:expr, $mmkv:ident, $key:expr, $value:expr, JFloatArray) => {
        $mmkv.put(
            &$key,
            jprimary_array_to_native($env, $value, 0.0)?.as_slice(),
        )
    };
    ($env:expr, $mmkv:ident, $key:expr, $value:expr, JDoubleArray) => {
        $mmkv.put(
            &$key,
            jprimary_array_to_native($env, $value, 0.0)?.as_slice(),
        )
    };
}

macro_rules! mmkv_get {
    ($env:expr, $mmkv:ident, $key:expr, jstring) => {
        $mmkv.get::<String>(&$key)
    };
    ($env:expr, $mmkv:ident, $key:expr, jint) => {
        $mmkv.get::<i32>(&$key)
    };
    ($env:expr, $mmkv:ident, $key:expr, jboolean) => {
        $mmkv.get::<bool>(&$key)
    };
    ($env:expr, $mmkv:ident, $key:expr, jlong) => {
        $mmkv.get::<i64>(&$key)
    };
    ($env:expr, $mmkv:ident, $key:expr, jfloat) => {
        $mmkv.get::<f32>(&$key)
    };
    ($env:expr, $mmkv:ident, $key:expr, jdouble) => {
        $mmkv.get::<f64>(&$key)
    };
    ($env:expr, $mmkv:ident, $key:expr, jbyteArray) => {
        $mmkv.get::<Vec<u8>>(&$key)
    };
    ($env:expr, $mmkv:ident, $key:expr, jintArray) => {
        $mmkv.get::<Vec<i32>>(&$key)
    };
    ($env:expr, $mmkv:ident, $key:expr, jlongArray) => {
        $mmkv.get::<Vec<i64>>(&$key)
    };
    ($env:expr, $mmkv:ident, $key:expr, jfloatArray) => {
        $mmkv.get::<Vec<f32>>(&$key)
    };
    ($env:expr, $mmkv:ident, $key:expr, jdoubleArray) => {
        $mmkv.get::<Vec<f64>>(&$key)
    };
}

macro_rules! transform_value {
    ($env:expr, jstring, $value:expr) => {
        $env.new_string($value)?.into_raw()
    };
    ($env:expr, jint, $value:expr) => {
        $value as jint
    };
    ($env:expr, jboolean, $value:expr) => {
        $value as jboolean
    };
    ($env:expr, jlong, $value:expr) => {
        $value as jlong
    };
    ($env:expr, jfloat, $value:expr) => {
        $value as jfloat
    };
    ($env:expr, jdouble, $value:expr) => {
        $value as jdouble
    };
    ($env:expr, jbyteArray, $value:expr) => {{
        let vec: Vec<i8> = $value.into_iter().map(|item| item as i8).collect();
        native_to_jarray!($env, vec, new_byte_array)
    }};
    ($env:expr, jintArray, $value:expr) => {
        native_to_jarray!($env, $value, new_int_array)
    };
    ($env:expr, jlongArray, $value:expr) => {
        native_to_jarray!($env, $value, new_long_array)
    };
    ($env:expr, jfloatArray, $value:expr) => {
        native_to_jarray!($env, $value, new_float_array)
    };
    ($env:expr, jdoubleArray, $value:expr) => {
        native_to_jarray!($env, $value, new_double_array)
    };
}

macro_rules! impl_java_put {
    ($name:ident, $value_type:tt, $log_type:literal) => {
        #[unsafe(no_mangle)]
        #[allow(non_snake_case)]
        pub unsafe extern "C" fn $name(
            mut env: EnvUnowned,
            obj: JClass,
            key: JString,
            value: $value_type,
        ) {
            let _ = env
                .with_env_no_catch(|env| -> jni::errors::Result<()> {
                    let mmkv = get_mmkv(env, &obj)?;
                    let key = env_str(env, key)?;
                    match mmkv_put!(env, mmkv, key, value, $value_type) {
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
    ($name:ident, $value_type:tt, $log_type:literal, $default:expr) => {
        #[unsafe(no_mangle)]
        #[allow(non_snake_case)]
        pub unsafe extern "C" fn $name(
            mut env: EnvUnowned,
            obj: JClass,
            key: JString,
        ) -> $value_type {
            let outcome = env
                .with_env_no_catch(|env| -> jni::errors::Result<$value_type> {
                    let mmkv = get_mmkv(env, &obj)?;
                    let key = env_str(env, key)?;
                    match mmkv_get!(env, mmkv, key, $value_type) {
                        Ok(value) => {
                            verbose!(LOG_TAG, "found {} with key '{}'", $log_type, key);
                            Ok(transform_value!(env, $value_type, value))
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

static JAVA_CLASS: RwLock<Option<Global<JClass<'static>>>> = RwLock::new(None);

struct AndroidLogger {
    jvm: JavaVM,
}

impl AndroidLogger {
    fn new(jvm: JavaVM) -> Self {
        jvm.attach_current_thread(|env| -> jni::errors::Result<()> {
            let clz = env.find_class(JNIString::from(ANDROID_LOGGER_CLASS_NAME))?;
            let global_ref = env.new_global_ref(clz)?;
            JAVA_CLASS.write().unwrap().replace(global_ref);
            Ok(())
        })
        .unwrap();
        AndroidLogger { jvm }
    }

    fn call_java(&self, method: &str, param: String) {
        self.jvm
            .attach_current_thread(|env| -> jni::errors::Result<()> {
                let local_ref = {
                    let lock = JAVA_CLASS.read().unwrap();
                    env.new_local_ref(lock.as_ref().unwrap())?
                };
                let param = env.new_string(&param)?;
                env.call_static_method(
                    local_ref,
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
            MMKV::set_logger(Box::new(AndroidLogger::new(jvm)));
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

impl_java_put!(Java_net_yangkx_mmkv_MMKV_putString, JString, "string");

impl_java_put!(Java_net_yangkx_mmkv_MMKV_putInt, jint, "i32");

impl_java_put!(Java_net_yangkx_mmkv_MMKV_putBool, jboolean, "bool");

impl_java_put!(Java_net_yangkx_mmkv_MMKV_putLong, jlong, "long");

impl_java_put!(Java_net_yangkx_mmkv_MMKV_putFloat, jfloat, "float");

impl_java_put!(Java_net_yangkx_mmkv_MMKV_putDouble, jdouble, "double");

impl_java_put!(
    Java_net_yangkx_mmkv_MMKV_putByteArray,
    JByteArray,
    "byte array"
);

impl_java_put!(
    Java_net_yangkx_mmkv_MMKV_putIntArray,
    JIntArray,
    "int array"
);

impl_java_put!(
    Java_net_yangkx_mmkv_MMKV_putLongArray,
    JLongArray,
    "long array"
);

impl_java_put!(
    Java_net_yangkx_mmkv_MMKV_putFloatArray,
    JFloatArray,
    "float array"
);

impl_java_put!(
    Java_net_yangkx_mmkv_MMKV_putDoubleArray,
    JDoubleArray,
    "double array"
);

impl_java_get!(
    Java_net_yangkx_mmkv_MMKV_getString,
    jstring,
    "string",
    std::ptr::null_mut()
);

impl_java_get!(Java_net_yangkx_mmkv_MMKV_getInt, jint, "int", 0);

impl_java_get!(Java_net_yangkx_mmkv_MMKV_getBool, jboolean, "bool", false);

impl_java_get!(Java_net_yangkx_mmkv_MMKV_getLong, jlong, "long", 0);

impl_java_get!(Java_net_yangkx_mmkv_MMKV_getFloat, jfloat, "float", 0.0);

impl_java_get!(Java_net_yangkx_mmkv_MMKV_getDouble, jdouble, "double", 0.0);

impl_java_get!(
    Java_net_yangkx_mmkv_MMKV_getByteArray,
    jbyteArray,
    "byte array",
    std::ptr::null_mut()
);

impl_java_get!(
    Java_net_yangkx_mmkv_MMKV_getIntArray,
    jintArray,
    "int array",
    std::ptr::null_mut()
);

impl_java_get!(
    Java_net_yangkx_mmkv_MMKV_getLongArray,
    jlongArray,
    "long array",
    std::ptr::null_mut()
);

impl_java_get!(
    Java_net_yangkx_mmkv_MMKV_getFloatArray,
    jfloatArray,
    "float array",
    std::ptr::null_mut()
);

impl_java_get!(
    Java_net_yangkx_mmkv_MMKV_getDoubleArray,
    jdoubleArray,
    "double array",
    std::ptr::null_mut()
);

#[unsafe(no_mangle)]
#[allow(non_snake_case)]
pub unsafe extern "C" fn Java_net_yangkx_mmkv_MMKV_delete(
    mut env: EnvUnowned,
    obj: JClass,
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
pub unsafe extern "C" fn Java_net_yangkx_mmkv_MMKV_clearData(mut env: EnvUnowned, obj: JClass) {
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
