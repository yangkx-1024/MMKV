#!/bin/bash

# Build static libs
for TARGET in \
        aarch64-apple-ios x86_64-apple-ios aarch64-apple-ios-sim \
        x86_64-apple-darwin aarch64-apple-darwin
do
    rustup target add $TARGET
    # Apple's App Sandbox disallows SysV semaphores; use POSIX semaphores instead
    cargo build -r --target=$TARGET
done

HEADER="include"
mkdir $HEADER
cbindgen src/clib/mod.rs -l c > $HEADER/mmkv.h
touch $HEADER/module.modulemap
echo "module MMKV {
  header \"mmkv.h\"
  export *
}" > $HEADER/module.modulemap

# Create XCFramework
FRAMEWORK="ios/MMKV/Sources/MMKV.xcframework"
rm -rf $FRAMEWORK
LIBNAME=libmmkv.a
mkdir mac-lipo ios-sim-lipo
IOS_SIM_LIPO=ios-sim-lipo/$LIBNAME
MAC_LIPO=mac-lipo/$LIBNAME
lipo -create -output $IOS_SIM_LIPO \
        target/aarch64-apple-ios-sim/release/$LIBNAME \
        target/x86_64-apple-ios/release/$LIBNAME
lipo -create -output $MAC_LIPO \
        target/aarch64-apple-darwin/release/$LIBNAME \
        target/x86_64-apple-darwin/release/$LIBNAME
xcodebuild -create-xcframework \
        -library $IOS_SIM_LIPO -headers $HEADER \
        -library $MAC_LIPO -headers $HEADER \
        -library target/aarch64-apple-ios/release/$LIBNAME -headers $HEADER \
        -output $FRAMEWORK

# Cleanup
rm -rf ios-sim-lipo mac-lipo $HEADER

