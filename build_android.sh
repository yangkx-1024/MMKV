#!/bin/bash

if [[ -z $ANDROID_NDK_TOOLCHAINS_PATH ]]; then
    echo "ANDROID_NDK_TOOLCHAINS_PATH is not set"
    echo "set it to your NDK toolchains bin dir, looks like this:"
    echo "/path/to/ndk/toolchains/llvm/prebuilt/YOUR_PLATFORM/bin"
    exit 1
fi

# target.aarch64-linux-android
export CARGO_TARGET_AARCH64_LINUX_ANDROID_LINKER=$ANDROID_NDK_TOOLCHAINS_PATH/aarch64-linux-android33-clang

# target.armv7-linux-androideabi
export CARGO_TARGET_ARMV7_LINUX_ANDROIDEABI_LINKER=$ANDROID_NDK_TOOLCHAINS_PATH/armv7a-linux-androideabi33-clang

# target.x86_64-linux-android
export CARGO_TARGET_X86_64_LINUX_ANDROID_LINKER=$ANDROID_NDK_TOOLCHAINS_PATH/x86_64-linux-android33-clang

targets_dic=(
  "aarch64-linux-android:arm64-v8a"
  "armv7-linux-androideabi:armeabi-v7a"
  "x86_64-linux-android:x86_64"
)

for ITEM in "${targets_dic[@]}" ; do
  TARGET=${ITEM%%:*}
  ANDROID_TARGET=${ITEM##*:}
  echo "Building $TARGET......"
  rustup target add "$TARGET"
  echo "Building with default feature......"
  cargo build --target "$TARGET" --release
  cp target/"$TARGET"/release/libmmkv.so android/library/src/main/jniLibs/"$ANDROID_TARGET"/libmmkv.so
  echo "Building with feature encryption...."
  cargo build --features encryption --target "$TARGET" --release
  cp target/"$TARGET"/release/libmmkv.so android/library-encrypt/src/main/jniLibs/"$ANDROID_TARGET"/libmmkv.so
done
