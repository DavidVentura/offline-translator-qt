FROM ubuntu:24.04

# linuxdeploy resolves the app's libraries and QML imports against whatever is
# installed here, so this image needs the runtime side of everything the binary
# and Main.qml touch, plus qmlimportscanner and objdump for the deploy/prune
# passes. The AppImage inherits this image's glibc as its minimum version.
RUN apt-get update && DEBIAN_FRONTEND=noninteractive apt-get install -y --no-install-recommends \
        binutils \
        ca-certificates \
        curl \
        file \
        fontconfig \
        gstreamer1.0-plugins-base \
        gstreamer1.0-plugins-good \
        gstreamer1.0-plugins-bad \
        gstreamer1.0-pulseaudio \
        libfontconfig1 \
        libgl1 \
        libpulse0 \
        libqt5multimedia5-plugins \
        libqt5svg5 \
        qml-module-qt-labs-settings \
        qml-module-qtgraphicaleffects \
        qml-module-qtmultimedia \
        qml-module-qtqml-models2 \
        qml-module-qtquick-controls \
        qml-module-qtquick-controls2 \
        qml-module-qtquick-dialogs \
        qml-module-qtquick-layouts \
        qml-module-qtquick-window2 \
        qml-module-qtquick2 \
        qml-module-qtsensors \
        qt5-qmake \
        qtchooser \
        qtdeclarative5-dev-tools \
        qtwayland5 \
    && rm -rf /var/lib/apt/lists/*

ENV QMAKE=/usr/lib/qt5/bin/qmake
