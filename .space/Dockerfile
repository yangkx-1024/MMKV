FROM ubuntu:20.04

ENV LANG=C.UTF-8

MAINTAINER Kexuan Yang <kexuan.yang@gmail.com>

# Support various rvm, nvm etc stuff which requires executing profile scripts (-l)
SHELL ["/bin/bash", "-lc"]
CMD ["/bin/bash", "-l"]

# Set debconf to run non-interactively
RUN echo 'debconf debconf/frontend select Noninteractive' | debconf-set-selections

RUN apt-get update && apt-get install -y apt-utils apt-transport-https software-properties-common

# Newest git
RUN apt-add-repository ppa:git-core/ppa -y && apt-get update

RUN set -ex -o pipefail && apt-get install -y \
    # Useful utilities \
    curl unzip wget socat man-db rsync moreutils vim lsof ssh \
    git build-essential libssl-dev libreadline-dev libcurl4 \
    openjdk-17-jdk-headless binutils gnupg2 libc6-dev tzdata \
    libcurl4-openssl-dev libedit2 libgcc-9-dev libpython3.8 zlib1g-dev \
    libstdc++-9-dev libsqlite3-0 libxml2-dev libz3-dev pkg-config && \
    apt-get clean && \
    rm -r /var/lib/apt/lists/*

ENV JAVA_HOME="/usr/lib/jvm/java-17-openjdk-amd64"

# Android
ARG sdk_version=commandlinetools-linux-10406996_latest.zip
ARG android_api=android-34
ARG android_build_tools=34.0.0
ARG android_ndk=false
ARG ndk_version=26.1.10909125
ARG cmake=3.22.1
ARG android_system_image=system-images;${android_api};google_apis_playstore;x86_64

ENV ANDROID_HOME=/opt/android/sdk
    ANDROID_NDK_ROOT=${ANDROID_HOME}/ndk/${ndk_version}
    ANDROID_NDK_TOOLCHAINS_PATH=${ANDROID_NDK_ROOT}/toolchains/llvm/prebuilt/linux-x86_64/bin
    ANDROID_AVD_HOME=/opt/android/avd
    EMULATOR_NAME=nexus
    EMULATOR_TIMEOUT=360

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
    echo "" && \
    swift --version && \
    echo "############################### Versions #####################################"
