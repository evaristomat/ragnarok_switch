ARG TOOLCHAIN_IMAGE=nostalre-switch-toolchain:work
FROM ${TOOLCHAIN_IMAGE}
RUN apt-get update && DEBIAN_FRONTEND=noninteractive apt-get install -y --no-install-recommends \
    pkg-config libasound2-dev libudev-dev libxkbcommon-dev libxkbcommon-x11-dev libwayland-dev \
    libx11-dev libxcursor-dev libxrandr-dev libxi-dev libvulkan1 mesa-vulkan-drivers \
    xvfb xauth x11-apps imagemagick \
    && rm -rf /var/lib/apt/lists/*
