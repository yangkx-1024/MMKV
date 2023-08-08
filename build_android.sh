#!/bin/bash

# target.aarch64-linux-android
export CARGO_TARGET_AARCH64_LINUX_ANDROID_LINKER=$ANDROID_NDK_BIN/aarch64-linux-android30-clang

# target.armv7-linux-androideabi
export CARGO_TARGET_ARMV7_LINUX_ANDROIDEABI_LINKER=$ANDROID_NDK_BIN/armv7a-linux-androideabi30-clang

# target.x86_64-linux-android
export CARGO_TARGET_X86_64_LINUX_ANDROID_LINKER=$ANDROID_NDK_BIN/x86_64-linux-android30-clang

echo "Building with default feature......"
echo "Building aarch64-linux-android......"
cargo build --target aarch64-linux-android --release
cp target/aarch64-linux-android/release/libmmkv.so android/library/src/main/jniLibs/arm64-v8a/libmmkv.so

echo "Building armv7-linux-androideabi......"
cargo build --target armv7-linux-androideabi --release
cp target/armv7-linux-androideabi/release/libmmkv.so android/library/src/main/jniLibs/armeabi-v7a/libmmkv.so

echo "Building x86_64-linux-android......"
cargo build --target x86_64-linux-android --release
cp target/x86_64-linux-android/release/libmmkv.so android/library/src/main/jniLibs/x86_64/libmmkv.so

echo "Building with feature encryption...."
echo "Building aarch64-linux-android......"
cargo build --features encryption --target aarch64-linux-android --release
cp target/aarch64-linux-android/release/libmmkv.so android/library-encrypt/src/main/jniLibs/arm64-v8a/libmmkv.so

echo "Building armv7-linux-androideabi......"
cargo build --features encryption --target armv7-linux-androideabi --release
cp target/armv7-linux-androideabi/release/libmmkv.so android/library-encrypt/src/main/jniLibs/armeabi-v7a/libmmkv.so

echo "Building x86_64-linux-android......"
cargo build --features encryption --target x86_64-linux-android --release
cp target/x86_64-linux-android/release/libmmkv.so android/library-encrypt/src/main/jniLibs/x86_64/libmmkv.so