/// Expose the JNI interface for android below
#[cfg(target_os = "android")]
#[allow(non_snake_case)]
pub mod android {
    extern crate jni;

    use jni::objects::{GlobalRef, JClass, JString, JValue};
    use jni::sys::{jboolean, jint, jstring};
    use jni::{JNIEnv, JavaVM};
    use std::fmt::{Debug, Formatter};

    use crate::MMKV;
    use crate::{Error, Logger};

    const LOG_TAG: &str = "MMKV:Android";

    struct AndroidLogger {
        jvm: JavaVM,
        clz: GlobalRef,
    }
    static ANDROID_LOGGER_CLASS_NAME: &str = "net/yangkx/mmkv/log/LoggerWrapper";

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

    #[no_mangle]
    pub unsafe extern "C" fn Java_net_yangkx_mmkv_MMKV_putString(
        mut env: JNIEnv,
        _: JClass,
        key: JString,
        value: JString,
    ) {
        let key: String = env.get_string(&key).unwrap().into();
        let value: String = env.get_string(&value).unwrap().into();
        match MMKV::put_str(&key, &value) {
            Err(e) => throw_put_failed(&mut env, &key, e),
            Ok(()) => {
                verbose!(LOG_TAG, "put string for key '{}'", key);
            }
        };
    }

    #[no_mangle]
    pub unsafe extern "C" fn Java_net_yangkx_mmkv_MMKV_putInt(
        mut env: JNIEnv,
        _: JClass,
        key: JString,
        value: jint,
    ) {
        let key: String = env.get_string(&key).unwrap().into();
        match MMKV::put_i32(&key, value) {
            Err(e) => throw_put_failed(&mut env, &key, e),
            _ => {
                verbose!(LOG_TAG, "put int for key '{}' success", key);
            }
        };
    }

    #[no_mangle]
    pub unsafe extern "C" fn Java_net_yangkx_mmkv_MMKV_putBool(
        mut env: JNIEnv,
        _: JClass,
        key: JString,
        value: jboolean,
    ) {
        let key: String = env.get_string(&key).unwrap().into();
        match MMKV::put_bool(&key, value == 1u8) {
            Err(e) => throw_put_failed(&mut env, &key, e),
            _ => {
                verbose!(LOG_TAG, "put bool for key '{}' success", key);
            }
        };
    }

    #[no_mangle]
    pub unsafe extern "C" fn Java_net_yangkx_mmkv_MMKV_getString(
        mut env: JNIEnv,
        _: JClass,
        key: JString,
    ) -> jstring {
        let key: String = env.get_string(&key).unwrap().into();
        match MMKV::get_str(&key) {
            Ok(str) => {
                verbose!(LOG_TAG, "found string with key '{}'", key);
                env.new_string(str).unwrap().into_raw()
            }
            Err(e) => {
                throw_key_not_found(&mut env, &key, e);
                std::ptr::null_mut()
            }
        }
    }

    #[no_mangle]
    pub unsafe extern "C" fn Java_net_yangkx_mmkv_MMKV_getInt(
        mut env: JNIEnv,
        _: JClass,
        key: JString,
    ) -> jint {
        let key: String = env.get_string(&key).unwrap().into();
        match MMKV::get_i32(&key) {
            Ok(value) => {
                verbose!(LOG_TAG, "found int with key '{}'", key);
                value
            }
            Err(e) => {
                throw_key_not_found(&mut env, &key, e);
                0
            }
        }
    }

    #[no_mangle]
    pub unsafe extern "C" fn Java_net_yangkx_mmkv_MMKV_getBool(
        mut env: JNIEnv,
        _: JClass,
        key: JString,
    ) -> jboolean {
        let key: String = env.get_string(&key).unwrap().into();
        match MMKV::get_bool(&key) {
            Ok(value) => {
                verbose!(LOG_TAG, "found bool with key '{}'", key);
                if value {
                    1
                } else {
                    0
                }
            }
            Err(e) => {
                throw_key_not_found(&mut env, &key, e);
                0
            }
        }
    }

    #[no_mangle]
    pub unsafe extern "C" fn Java_net_yangkx_mmkv_MMKV_setLogLevel(
        _: JNIEnv,
        _: JClass,
        level: jint,
    ) {
        MMKV::set_log_level(level);
    }

    #[no_mangle]
    pub unsafe extern "C" fn Java_net_yangkx_mmkv_MMKV_clearData(_: JNIEnv, _: JClass) {
        MMKV::clear_data();
    }

    #[no_mangle]
    pub unsafe extern "C" fn Java_net_yangkx_mmkv_MMKV_close(_: JNIEnv, _: JClass) {
        MMKV::close();
    }

    fn throw_key_not_found(env: &mut JNIEnv, key: &str, e: Error) {
        let log_str = format!("get key '{}' failed, reason: {:?}", key, e);
        error!(LOG_TAG, "{}", &log_str);
        env.throw_new("net/yangkx/mmkv/KeyNotFoundException", log_str)
            .expect("throw");
    }

    fn throw_put_failed(env: &mut JNIEnv, key: &str, e: Error) {
        let log_str = format!("failed to put key {}, reason {:?}", key, e);
        error!(LOG_TAG, "{}", &log_str);
        env.throw_new("net/yangkx/mmkv/NativeException", log_str)
            .expect("throw");
    }
}
