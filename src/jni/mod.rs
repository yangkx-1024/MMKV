/// Expose the JNI interface for android below
#[cfg(target_os = "android")]
#[allow(non_snake_case)]
pub mod android {
    extern crate android_log;
    extern crate jni;

    use jni::objects::{JClass, JString};
    use jni::sys::{jboolean, jint, jstring};
    use jni::JNIEnv;
    use log::info;

    use crate::MMKV;

    #[no_mangle]
    pub unsafe extern "C" fn Java_net_yangkx_mmkv_MMKV_initialize(
        mut env: JNIEnv,
        _: JClass,
        dir: JString,
        #[cfg(feature = "encryption")] key: JString,
    ) {
        android_log::init("MMKV").unwrap();
        let path: String = env.get_string(&dir).unwrap().into();
        #[cfg(feature = "encryption")]
        let key: String = env.get_string(&key).unwrap().into();
        MMKV::initialize(
            &path,
            #[cfg(feature = "encryption")]
            &key,
        );
        info!("{}", MMKV::dump());
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
        MMKV::put_str(&key, &value);
    }

    #[no_mangle]
    pub unsafe extern "C" fn Java_net_yangkx_mmkv_MMKV_putInt(
        mut env: JNIEnv,
        _: JClass,
        key: JString,
        value: jint,
    ) {
        let key: String = env.get_string(&key).unwrap().into();
        MMKV::put_i32(&key, value);
    }

    #[no_mangle]
    pub unsafe extern "C" fn Java_net_yangkx_mmkv_MMKV_putBool(
        mut env: JNIEnv,
        _: JClass,
        key: JString,
        value: jboolean,
    ) {
        let key: String = env.get_string(&key).unwrap().into();
        MMKV::put_bool(&key, value == 1u8);
    }

    #[no_mangle]
    pub unsafe extern "C" fn Java_net_yangkx_mmkv_MMKV_getString(
        mut env: JNIEnv,
        _: JClass,
        key: JString,
    ) -> jstring {
        let key: String = env.get_string(&key).unwrap().into();
        match MMKV::get_str(&key) {
            Some(str) => env.new_string(str).unwrap().into_raw(),
            None => {
                throw_key_not_found(&mut env, &key);
                env.new_string("").unwrap().into_raw()
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
            Some(value) => value,
            None => {
                throw_key_not_found(&mut env, &key);
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
            Some(value) => {
                if value {
                    1u8
                } else {
                    0u8
                }
            }
            None => {
                throw_key_not_found(&mut env, &key);
                0u8
            }
        }
    }

    fn throw_key_not_found(env: &mut JNIEnv, key: &str) {
        let _ = env.throw_new(
            "java/util/NoSuchElementException",
            format!("{} not found", key),
        );
    }
}
