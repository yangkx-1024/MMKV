FROM ubuntu:20.04

ENV LANG=C.UTF-8

LABEL maintainer="Kexuan Yang <kexuan.yang@gmail.com>"

# Set debconf to run non-interactively
RUN echo 'debconf debconf/frontend select Noninteractive' | debconf-set-selections

RUN apt-get update && apt-get install -y \
    openjdk-17-jdk-headless \
    curl unzip wget vim ssh git gnupg2 build-essential

RUN apt-get clean && rm -r /var/lib/apt/lists/*

# Install OpenJDK Java 17 SDK
RUN ln -s $(readlink -f /usr/bin/javac | sed "s:bin/javac::") /usr/lib/jvm/java-default
ENV JAVA_HOME /usr/lib/jvm/java-default
# END OpenJDK Java 17 SDK

# Android
ARG sdk_version=commandlinetools-linux-10406996_latest.zip
ARG android_api=android-34
ARG android_build_tools=34.0.0
ARG android_ndk=false
ARG ndk_version=26.1.10909125
ARG cmake=3.22.1
ARG android_system_image=system-images;${android_api};google_apis_playstore;x86_64

ENV ANDROID_HOME=/opt/android/sdk
ENV ANDROID_NDK_ROOT=${ANDROID_HOME}/ndk/${ndk_version}
ENV ANDROID_NDK_TOOLCHAINS_PATH=${ANDROID_NDK_ROOT}/toolchains/llvm/prebuilt/linux-x86_64/bin
ENV ANDROID_AVD_HOME=/opt/android/avd
ENV EMULATOR_NAME=nexus
ENV EMULATOR_TIMEOUT=360

RUN mkdir -p ${ANDROID_HOME} && \
    wget --quiet --output-document=/tmp/${sdk_version} https://dl.google.com/android/repository/${sdk_version} && \
    unzip -q /tmp/${sdk_version} -d ${ANDROID_HOME} && \
    mkdir ${ANDROID_HOME}/cmdline-tools/latest && \
    cd ${ANDROID_HOME}/cmdline-tools && \
    ls | grep -v latest | xargs mv -t latest && \
    rm /tmp/${sdk_version}
ENV PATH=${ANDROID_HOME}/emulator:${ANDROID_HOME}/cmdline-tools/latest/bin:${ANDROID_HOME}/platform-tools:${PATH}

RUN mkdir ~/.android && echo '### User Sources for Android SDK Manager' > ~/.android/repositories.cfg
RUN yes | sdkmanager --sdk_root=$ANDROID_HOME --licenses
RUN sdkmanager --sdk_root=${ANDROID_HOME} --install \
    "platform-tools" \
    "build-tools;${android_build_tools}" \
    "platforms;${android_api}" \
    "emulator" \
    "${android_system_image}"
RUN mkdir -p ${ANDROID_AVD_HOME} && \
    echo "no" | avdmanager --verbose create avd --force -n \
    $EMULATOR_NAME --abi "google_apis_playstore/x86_64" -k "${android_system_image}"
RUN if [ "$android_ndk" = true ] ; \
    then \
      echo "Installing Android NDK ($ndk_version, cmake: $cmake)"; \
      sdkmanager --sdk_root=${ANDROID_HOME} --install \
      "ndk;${ndk_version}" \
      "cmake;${cmake}" ; \
    else \
      echo "Skipping NDK installation"; \
    fi
# End Android

# Rust
ENV RUSTUP_HOME=/usr/local/rustup \
    CARGO_HOME=/usr/local/cargo \
    PATH=/usr/local/cargo/bin:$PATH
RUN set -eux \
    && curl https://sh.rustup.rs -sSf | sh -s -- -y --no-modify-path --profile minimal \
    && chmod -R a+w $RUSTUP_HOME $CARGO_HOME
# End Rust

RUN echo "############################### Versions #####################################" && \
    java -version &&  \
    javac -version && \
    echo "" && \
    cargo --version && \
    rustc --version && \
    echo "############################### Versions #####################################"
