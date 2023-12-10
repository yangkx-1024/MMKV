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
    # git \
    git \
    # JVM \
    openjdk-17-jdk-headless \
    # Android \
    build-essential libssl-dev libreadline-dev libcurl4 \
    binutils gnupg2 libc6-dev libcurl4-openssl-dev libedit2 \
    libgcc-9-dev libpython3.8 libsqlite3-0 libstdc++-9-dev \
    libxml2-dev libz3-dev pkg-config tzdata zlib1g-dev \
    && apt-get clean \
    && rm -r /var/lib/apt/lists/*

ENV JAVA_HOME="/usr/lib/jvm/java-17-openjdk-amd64"

# Android
ARG sdk_version=commandlinetools-linux-6200805_latest.zip
ARG android_home=/opt/android/sdk
ARG android_api=android-34
ARG android_build_tools=34.0.0
ARG android_ndk=false
ARG ndk_version=26.1.10909125
ARG cmake=3.22.1
RUN mkdir -p ${android_home} && \
    wget --quiet --output-document=/tmp/${sdk_version} https://dl.google.com/android/repository/${sdk_version} && \
    unzip -q /tmp/${sdk_version} -d ${android_home} && \
    rm /tmp/${sdk_version}

ENV ANDROID_HOME=${android_home}
ENV ANDROID_NDK_TOOLCHAINS_PATH=${android_home}/nkd/${ndk_version}/toolchains/llvm/prebuilt/linux-x86_64/bin
ENV PATH=${ANDROID_HOME}/emulator:${ANDROID_HOME}/tools:${ANDROID_HOME}/tools/bin:${ANDROID_HOME}/platform-tools:${PATH}

RUN mkdir ~/.android && echo '### User Sources for Android SDK Manager' > ~/.android/repositories.cfg
RUN yes | sdkmanager --sdk_root=$ANDROID_HOME --licenses
RUN sdkmanager --sdk_root=$ANDROID_HOME --install \
  "platform-tools" \
  "build-tools;${android_build_tools}" \
  "platforms;${android_api}"
RUN if [ "$android_ndk" = true ] ; \
  then \
    echo "Installing Android NDK ($ndk_version, cmake: $cmake)"; \
    sdkmanager --sdk_root="$ANDROID_HOME" --install \
    "ndk;${ndk_version}" \
    "cmake;${cmake}" ; \
  else \
    echo "Skipping NDK installation"; \
  fi
# End Android

# Swift
ARG SWIFT_SIGNING_KEY=A62AE125BBBFBB96A6E042EC925CC1CCED3D1561
ARG SWIFT_PLATFORM=ubuntu20.04
ARG SWIFT_BRANCH=swift-5.9.1-release
ARG SWIFT_VERSION=swift-5.9.1-RELEASE
ARG SWIFT_WEBROOT=https://download.swift.org

ENV SWIFT_SIGNING_KEY=$SWIFT_SIGNING_KEY \
    SWIFT_PLATFORM=$SWIFT_PLATFORM \
    SWIFT_BRANCH=$SWIFT_BRANCH \
    SWIFT_VERSION=$SWIFT_VERSION \
    SWIFT_WEBROOT=$SWIFT_WEBROOT

RUN set -e; \
    ARCH_NAME="$(dpkg --print-architecture)"; \
    url=; \
    case "${ARCH_NAME##*-}" in \
        'amd64') \
            OS_ARCH_SUFFIX=''; \
            ;; \
        'arm64') \
            OS_ARCH_SUFFIX='-aarch64'; \
            ;; \
        *) echo >&2 "error: unsupported architecture: '$ARCH_NAME'"; exit 1 ;; \
    esac; \
    SWIFT_WEBDIR="$SWIFT_WEBROOT/$SWIFT_BRANCH/$(echo $SWIFT_PLATFORM | tr -d .)$OS_ARCH_SUFFIX" \
    && SWIFT_BIN_URL="$SWIFT_WEBDIR/$SWIFT_VERSION/$SWIFT_VERSION-$SWIFT_PLATFORM$OS_ARCH_SUFFIX.tar.gz" \
    && SWIFT_SIG_URL="$SWIFT_BIN_URL.sig" \
    # - Download the GPG keys, Swift toolchain, and toolchain signature, and verify.
    && export GNUPGHOME="$(mktemp -d)" \
    && curl -fsSL "$SWIFT_BIN_URL" -o swift.tar.gz "$SWIFT_SIG_URL" -o swift.tar.gz.sig \
    && gpg --batch --quiet --keyserver hkp://keyserver.ubuntu.com:80 --recv-keys "$SWIFT_SIGNING_KEY" \
    && gpg --batch --verify swift.tar.gz.sig swift.tar.gz \
    # - Unpack the toolchain, set libs permissions, and clean up.
    && tar -xzf swift.tar.gz --directory / --strip-components=1 \
    && chmod -R o+r /usr/lib/swift \
    && rm -rf "$GNUPGHOME" swift.tar.gz.sig swift.tar.gz
# End Swift

ENV RUSTUP_HOME=/usr/local/rustup \
    CARGO_HOME=/usr/local/cargo \
    PATH=/usr/local/cargo/bin:$PATH
RUN set -eux \
    && curl https://sh.rustup.rs -sSf | sh -s -- -y --no-modify-path --profile minimal \
    && chmod -R a+w $RUSTUP_HOME $CARGO_HOME

RUN echo "############################### Versions #####################################" && \
    java -version &&  \
    javac -version && \
    echo "" && \
    cargo --version && \
    rustc --version && \
    echo "" && \
    swift --version && \
    echo "############################### Versions #####################################"
