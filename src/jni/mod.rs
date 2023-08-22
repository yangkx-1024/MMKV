/// Expose the JNI interface for android below
#[cfg(target_os = "android")]
#[allow(non_snake_case)]
pub mod android {
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

    use crate::Logger;
    use crate::MMKV;

    const LOG_TAG: &str = "MMKV:Android";

    static ANDROID_LOGGER_CLASS_NAME: &str = "net/yangkx/mmkv/log/LoggerWrapper";
    static ANDROID_NATIVE_EXCEPTION: &str = "net/yangkx/mmkv/NativeException";
    static ANDROID_KEY_NOT_FOUND_EXCEPTION: &str = "net/yangkx/mmkv/KeyNotFoundException";

    #[inline]
    fn env_str(env: &mut JNIEnv, name: JString) -> String {
        env.get_string(&name).unwrap().into()
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
        ($env:expr, $key:expr, $value:expr, JString) => {{
            let value = env_str($env, $value);
            MMKV::put_str(&$key, &value)
        }};
        ($env:expr, $key:expr, $value:expr, jint) => {{
            MMKV::put_i32(&$key, $value)
        }};
        ($env:expr, $key:expr, $value:expr, jboolean) => {{
            MMKV::put_bool(&$key, $value == 1u8)
        }};
        ($env:expr, $key:expr, $value:expr, jlong) => {{
            MMKV::put_i64(&$key, $value)
        }};
        ($env:expr, $key:expr, $value:expr, jfloat) => {{
            MMKV::put_f32(&$key, $value)
        }};
        ($env:expr, $key:expr, $value:expr, jdouble) => {{
            MMKV::put_f64(&$key, $value)
        }};
        ($env:expr, $key:expr, $value:expr, JByteArray) => {{
            let vec = jarray_to_native!($env, $value, get_byte_array_region, 0);
            let byte_array: Vec<u8> = vec.into_iter().map(|item| item as u8).collect();
            MMKV::put_byte_array(&$key, byte_array.as_slice())
        }};
        ($env:expr, $key:expr, $value:expr, JIntArray) => {{
            let vec = jarray_to_native!($env, $value, get_int_array_region, 0);
            MMKV::put_i32_array(&$key, vec.as_slice())
        }};
        ($env:expr, $key:expr, $value:expr, JLongArray) => {{
            let vec = jarray_to_native!($env, $value, get_long_array_region, 0);
            MMKV::put_i64_array(&$key, vec.as_slice())
        }};
        ($env:expr, $key:expr, $value:expr, JFloatArray) => {{
            let vec = jarray_to_native!($env, $value, get_float_array_region, 0.0);
            MMKV::put_f32_array(&$key, vec.as_slice())
        }};
        ($env:expr, $key:expr, $value:expr, JDoubleArray) => {{
            let vec = jarray_to_native!($env, $value, get_double_array_region, 0.0);
            MMKV::put_f64_array(&$key, vec.as_slice())
        }};
    }

    macro_rules! mmkv_get {
        ($env:expr, $key:expr, jstring) => {{
            let result =
                MMKV::get_str(&$key).map(|value| $env.new_string(value).unwrap().into_raw());
            result
        }};
        ($env:expr, $key:expr, jint) => {{
            MMKV::get_i32(&$key)
        }};
        ($env:expr, $key:expr, jboolean) => {{
            MMKV::get_bool(&$key).map(|value| if (value) { 1u8 } else { 0u8 })
        }};
        ($env:expr, $key:expr, jlong) => {{
            MMKV::get_i64(&$key)
        }};
        ($env:expr, $key:expr, jfloat) => {{
            MMKV::get_f32(&$key)
        }};
        ($env:expr, $key:expr, jdouble) => {{
            MMKV::get_f64(&$key)
        }};
        ($env:expr, $key:expr, jbyteArray) => {{
            MMKV::get_byte_array(&$key).map(|value| {
                let vec: Vec<i8> = value.into_iter().map(|item| item as i8).collect();
                native_to_jarray!($env, vec, new_byte_array, set_byte_array_region)
            })
        }};
        ($env:expr, $key:expr, jintArray) => {{
            MMKV::get_i32_array(&$key)
                .map(|value| native_to_jarray!($env, value, new_int_array, set_int_array_region))
        }};
        ($env:expr, $key:expr, jlongArray) => {{
            MMKV::get_i64_array(&$key)
                .map(|value| native_to_jarray!($env, value, new_long_array, set_long_array_region))
        }};
        ($env:expr, $key:expr, jfloatArray) => {{
            MMKV::get_f32_array(&$key).map(|value| {
                native_to_jarray!($env, value, new_float_array, set_float_array_region)
            })
        }};
        ($env:expr, $key:expr, jdoubleArray) => {{
            MMKV::get_f64_array(&$key).map(|value| {
                native_to_jarray!($env, value, new_double_array, set_double_array_region)
            })
        }};
    }

    macro_rules! impl_java_put {
        ($name:ident, $value_type:tt, $log_type:literal) => {
            #[no_mangle]
            pub unsafe extern "C" fn $name(
                mut env: JNIEnv,
                _: JClass,
                key: JString,
                value: $value_type,
            ) {
                let key = env_str(&mut env, key);
                match mmkv_put!(&mut env, key, value, $value_type) {
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
            pub unsafe extern "C" fn $name(
                mut env: JNIEnv,
                _: JClass,
                key: JString,
            ) -> $value_type {
                let key = env_str(&mut env, key);
                match mmkv_get!(&mut env, key, $value_type) {
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

    struct AndroidLogger {
        jvm: JavaVM,
        clz: GlobalRef,
    }

    impl AndroidLogger {
        fn new(jvm: JavaVM) -> Self {
            let mut env = jvm.get_env().unwrap();
            let clz = env.find_class(ANDROID_LOGGER_CLASS_NAME).unwrap();
            let global_ref = env.new_global_ref(clz).unwrap();

            AndroidLogger {
                jvm,
                clz: global_ref,
            }
        }

        fn call_java(&self, method: &str, param: &str) {
            let mut env = self.jvm.get_env().unwrap();
            let clz: JClass = JClass::from(env.new_local_ref(&self.clz).unwrap());
            let param = env.new_string(param).unwrap();
            env.call_static_method(
                clz,
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
        fn verbose(&self, log_str: &str) {
            self.call_java("verbose", log_str)
        }

        fn info(&self, log_str: &str) {
            self.call_java("info", log_str)
        }

        fn debug(&self, log_str: &str) {
            self.call_java("debug", log_str)
        }

        fn warn(&self, log_str: &str) {
            self.call_java("warn", log_str)
        }

        fn error(&self, log_str: &str) {
            self.call_java("error", log_str)
        }
    }

    #[no_mangle]
    pub unsafe extern "C" fn Java_net_yangkx_mmkv_MMKV_initialize(
        mut env: JNIEnv,
        _: JClass,
        dir: JString,
        #[cfg(feature = "encryption")] key: JString,
    ) {
        MMKV::set_logger(Box::new(AndroidLogger::new(env.get_java_vm().unwrap())));
        let path: String = env.get_string(&dir).unwrap().into();
        #[cfg(feature = "encryption")]
        let key: String = env.get_string(&key).unwrap().into();
        MMKV::initialize(
            &path,
            #[cfg(feature = "encryption")]
            &key,
        );
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
    pub unsafe extern "C" fn Java_net_yangkx_mmkv_MMKV_clearData(_: JNIEnv, _: JClass) {
        MMKV::clear_data();
    }

    #[no_mangle]
    pub unsafe extern "C" fn Java_net_yangkx_mmkv_MMKV_close(_: JNIEnv, _: JClass) {
        MMKV::close();
    }
}
