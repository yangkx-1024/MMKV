#!/bin/bash
# Fail on the first error instead of carrying on and shipping a half-built framework: a
# failed target build used to leave the previous target's .a in place and still exit 0.
set -euo pipefail

# Both tools shape the generated C header, so a release must not pick up whatever version
# happens to be current on the day it runs. Bump these deliberately.
CARGO_EXPAND_VERSION="1.0.126"
CBINDGEN_VERSION="0.29.4"

cargo install cargo-expand --version "$CARGO_EXPAND_VERSION" --locked
cargo install cbindgen --version "$CBINDGEN_VERSION" --locked
# Build static libs
for TARGET in \
        aarch64-apple-ios x86_64-apple-ios aarch64-apple-ios-sim \
        x86_64-apple-darwin aarch64-apple-darwin
do
    echo "Build for $TARGET..."
    rustup target add $TARGET
    cargo build -r --locked --target=$TARGET
    strip target/"$TARGET"/release/libmmkv.a
done

HEADER="include"
mkdir -p $HEADER
cargo expand ffi > $HEADER/mod.rs
cbindgen --config cbindgen.toml $HEADER/mod.rs -o src/ffi/rust_mmkv.h
cp src/ffi/rust_mmkv.h $HEADER/rust_mmkv.h
rm $HEADER/mod.rs
touch $HEADER/module.modulemap
echo "module RustMMKV {
  header \"rust_mmkv.h\"
  export *
}" > $HEADER/module.modulemap

# Create XCFramework
FRAMEWORK="ios/MMKV/RustMMKV.xcframework"
rm -rf $FRAMEWORK
LIBNAME=libmmkv.a
mkdir -p mac-lipo ios-sim-lipo
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

pushd ios/MMKV/ || exit 1;
zip -rm9 RustMMKV.xcframework.zip RustMMKV.xcframework
popd || exit 1;

# Cleanup
rm -rf ios-sim-lipo mac-lipo $HEADER

