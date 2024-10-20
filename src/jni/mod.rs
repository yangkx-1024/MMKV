use jni::objects::{
    GlobalRef, JByteArray, JClass, JDoubleArray, JFloatArray, JIntArray, JLongArray, JString,
    JValue,
};
use jni::sys::{
    jboolean, jbyteArray, jdouble, jdoubleArray, jfloat, jfloatArray, jint, jintArray, jlong,
    jlongArray, jsize, jstring,
};
use jni::{JNIEnv, JavaVM};
use std::fmt::{Debug, Formatter};
use std::sync::RwLock;

use crate::Logger;
use crate::MMKV;

const LOG_TAG: &str = "MMKV:Android";

static ANDROID_LOGGER_CLASS_NAME: &str = "net/yangkx/mmkv/log/LoggerWrapper";
static ANDROID_NATIVE_EXCEPTION: &str = "net/yangkx/mmkv/NativeException";
static ANDROID_KEY_NOT_FOUND_EXCEPTION: &str = "net/yangkx/mmkv/KeyNotFoundException";
static ANDROID_FIELD_NATIVE_OBJ: &str = "nativeObj";

#[inline]
fn env_str(env: &mut JNIEnv, name: JString) -> String {
    env.get_string(&name).unwrap().into()
}

fn get_mmkv_ptr(env: &mut JNIEnv, obj: &JClass) -> *mut MMKV {
    env.get_field(obj, ANDROID_FIELD_NATIVE_OBJ, "J")
        .unwrap()
        .j()
        .unwrap() as *mut MMKV
}

macro_rules! jarray_to_native {
    ($env:expr, $value:expr, $op:tt, $default:expr) => {{
        let len = $env.get_array_length(&$value).unwrap();
        let mut vec = Vec::new();
        vec.resize(len as usize, $default);
        $env.$op($value, 0, vec.as_mut_slice()).unwrap();
        vec
    }};
}

macro_rules! native_to_jarray {
    ($env:expr, $value:expr, $new_op:tt, $set_op:tt) => {{
        let array = $env.$new_op($value.len() as jsize).unwrap();
        $env.$set_op(&array, 0, $value.as_slice()).unwrap();
        array.into_raw()
    }};
}

macro_rules! mmkv_put {
    ($env:expr, $mmkv:ident, $key:expr, $value:expr, JString) => {{
        let value = env_str($env, $value);
        $mmkv.put(&$key, value.as_str())
    }};
    ($env:expr, $mmkv:ident, $key:expr, $value:expr, jint) => {
        $mmkv.put(&$key, $value)
    };
    ($env:expr, $mmkv:ident, $key:expr, $value:expr, jboolean) => {
        $mmkv.put(&$key, $value == 1u8)
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
        let vec = jarray_to_native!($env, $value, get_byte_array_region, 0);
        let byte_array: Vec<u8> = vec.into_iter().map(|item| item as u8).collect();
        $mmkv.put(&$key, byte_array.as_slice())
    }};
    ($env:expr, $mmkv:ident, $key:expr, $value:expr, JIntArray) => {{
        let vec = jarray_to_native!($env, $value, get_int_array_region, 0);
        $mmkv.put(&$key, vec.as_slice())
    }};
    ($env:expr, $mmkv:ident, $key:expr, $value:expr, JLongArray) => {{
        let vec = jarray_to_native!($env, $value, get_long_array_region, 0);
        $mmkv.put(&$key, vec.as_slice())
    }};
    ($env:expr, $mmkv:ident, $key:expr, $value:expr, JFloatArray) => {{
        let vec = jarray_to_native!($env, $value, get_float_array_region, 0.0);
        $mmkv.put(&$key, vec.as_slice())
    }};
    ($env:expr, $mmkv:ident, $key:expr, $value:expr, JDoubleArray) => {{
        let vec = jarray_to_native!($env, $value, get_double_array_region, 0.0);
        $mmkv.put(&$key, vec.as_slice())
    }};
}

macro_rules! mmkv_get {
    ($env:expr, $mmkv:ident, $key:expr, jstring) => {
        $mmkv
            .get::<String>(&$key)
            .map(|value| $env.new_string(value).unwrap().into_raw())
    };
    ($env:expr, $mmkv:ident, $key:expr, jint) => {
        $mmkv.get::<i32>(&$key)
    };
    ($env:expr, $mmkv:ident, $key:expr, jboolean) => {
        $mmkv
            .get::<bool>(&$key)
            .map(|value| if value { 1u8 } else { 0u8 })
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
        $mmkv.get::<Vec<u8>>(&$key).map(|value| {
            let vec: Vec<i8> = value.into_iter().map(|item| item as i8).collect();
            native_to_jarray!($env, vec, new_byte_array, set_byte_array_region)
        })
    };
    ($env:expr, $mmkv:ident, $key:expr, jintArray) => {
        $mmkv
            .get::<Vec<i32>>(&$key)
            .map(|value| native_to_jarray!($env, value, new_int_array, set_int_array_region))
    };
    ($env:expr, $mmkv:ident, $key:expr, jlongArray) => {
        $mmkv
            .get::<Vec<i64>>(&$key)
            .map(|value| native_to_jarray!($env, value, new_long_array, set_long_array_region))
    };
    ($env:expr, $mmkv:ident, $key:expr, jfloatArray) => {
        $mmkv
            .get::<Vec<f32>>(&$key)
            .map(|value| native_to_jarray!($env, value, new_float_array, set_float_array_region))
    };
    ($env:expr, $mmkv:ident, $key:expr, jdoubleArray) => {
        $mmkv
            .get::<Vec<f64>>(&$key)
            .map(|value| native_to_jarray!($env, value, new_double_array, set_double_array_region))
    };
}

macro_rules! impl_java_put {
    ($name:ident, $value_type:tt, $log_type:literal) => {
        #[no_mangle]
        #[allow(non_snake_case)]
        pub unsafe extern "C" fn $name(
            mut env: JNIEnv,
            obj: JClass,
            key: JString,
            value: $value_type,
        ) {
            let mmkv = get_mmkv_ptr(&mut env, &obj).as_ref().unwrap();
            let key = env_str(&mut env, key);
            match mmkv_put!(&mut env, mmkv, key, value, $value_type) {
                Err(e) => {
                    let log_str = format!(
                        "failed to put {} for key {}, reason {:?}",
                        $log_type, key, e
                    );
                    error!(LOG_TAG, "{}", &log_str);
                    env.throw_new(ANDROID_NATIVE_EXCEPTION, log_str)
                        .expect("throw");
                }
                Ok(()) => {
                    verbose!(LOG_TAG, "put {} for key '{}' success", $log_type, key);
                }
            };
        }
    };
}

macro_rules! impl_java_get {
    ($name:ident, $value_type:tt, $log_type:literal, $default:expr) => {
        #[no_mangle]
        #[allow(non_snake_case)]
        pub unsafe extern "C" fn $name(mut env: JNIEnv, obj: JClass, key: JString) -> $value_type {
            let mmkv = get_mmkv_ptr(&mut env, &obj).as_ref().unwrap();
            let key = env_str(&mut env, key);
            match mmkv_get!(&mut env, mmkv, key, $value_type) {
                Ok(value) => {
                    verbose!(LOG_TAG, "found {} with key '{}'", $log_type, key);
                    value
                }
                Err(e) => {
                    let log_str = format!(
                        "get {} for key '{}' failed, reason: {:?}",
                        $log_type, key, e
                    );
                    error!(LOG_TAG, "{}", &log_str);
                    env.throw_new(ANDROID_KEY_NOT_FOUND_EXCEPTION, log_str)
                        .expect("throw");
                    $default
                }
            }
        }
    };
}

static JAVA_CLASS: RwLock<Option<GlobalRef>> = RwLock::new(None);

struct AndroidLogger {
    jvm: JavaVM,
}

impl AndroidLogger {
    fn new(jvm: JavaVM) -> Self {
        let mut env = jvm.get_env().unwrap();
        let clz = env.find_class(ANDROID_LOGGER_CLASS_NAME).unwrap();
        let global_ref = env.new_global_ref(clz).unwrap();
        JAVA_CLASS.write().unwrap().replace(global_ref);
        AndroidLogger { jvm }
    }

    fn call_java(&self, method: &str, param: String) {
        let mut env = self.jvm.attach_current_thread_permanently().unwrap();
        let local_ref = {
            let lock = JAVA_CLASS.read().unwrap();
            env.new_local_ref(lock.as_ref().unwrap()).unwrap()
        };
        let class = JClass::from(local_ref);
        let param = env.new_string(param).unwrap();
        env.call_static_method(
            class,
            method,
            "(Ljava/lang/String;)V",
            &[JValue::Object(&param)],
        )
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

#[no_mangle]
pub unsafe extern "C" fn Java_net_yangkx_mmkv_MMKV_attachLogger(env: JNIEnv, _: JClass) {
    MMKV::set_logger(Box::new(AndroidLogger::new(env.get_java_vm().unwrap())));
}

#[no_mangle]
pub unsafe extern "C" fn Java_net_yangkx_mmkv_MMKV_initialize(
    mut env: JNIEnv,
    _: JClass,
    dir: JString,
    #[cfg(feature = "encryption")] key: JString,
) -> jlong {
    let path: String = env.get_string(&dir).unwrap().into();
    #[cfg(feature = "encryption")]
    let key: String = env.get_string(&key).unwrap().into();
    let mmkv = MMKV::new(
        &path,
        #[cfg(feature = "encryption")]
        &key,
    );
    Box::into_raw(Box::new(mmkv)) as jlong
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

impl_java_get!(Java_net_yangkx_mmkv_MMKV_getBool, jboolean, "bool", 0);

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

#[no_mangle]
pub unsafe extern "C" fn Java_net_yangkx_mmkv_MMKV_delete(
    mut env: JNIEnv,
    obj: JClass,
    key: JString,
) {
    let mmkv = get_mmkv_ptr(&mut env, &obj).as_ref().unwrap();
    let key = env_str(&mut env, key);
    match mmkv.delete(&key) {
        Ok(()) => verbose!(LOG_TAG, "delete key {} success", &key),
        Err(e) => {
            let log_str = format!("failed to delete key {}, reason: {:?}", &key, e);
            error!(LOG_TAG, "{}", &log_str);
            env.throw_new(ANDROID_KEY_NOT_FOUND_EXCEPTION, log_str)
                .expect("throw");
        }
    }
}

#[no_mangle]
pub unsafe extern "C" fn Java_net_yangkx_mmkv_MMKV_setLogLevel(
    mut env: JNIEnv,
    _: JClass,
    level: jint,
) {
    if let Ok(level) = level.try_into() {
        MMKV::set_log_level(level);
    } else {
        env.throw_new(
            ANDROID_NATIVE_EXCEPTION,
            format!("invalid log level '{}'", level),
        )
        .expect("throw");
    }
}

#[no_mangle]
pub unsafe extern "C" fn Java_net_yangkx_mmkv_MMKV_clearData(mut env: JNIEnv, obj: JClass) {
    let mmkv = get_mmkv_ptr(&mut env, &obj).as_ref().unwrap();
    mmkv.clear_data();
}

#[no_mangle]
pub unsafe extern "C" fn Java_net_yangkx_mmkv_MMKV_close(_: JNIEnv, _: JClass, ptr: jlong) {
    let ptr = ptr as *mut MMKV;
    drop(Box::from_raw(ptr));
}
