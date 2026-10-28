# Based on bevy-horizon/scripts/build_nvk.sh.
ARG TOOLCHAIN_IMAGE=nostalre-switch-toolchain:work
FROM ${TOOLCHAIN_IMAGE}
ARG JOBS=2
RUN apt-get update && apt-get install -y --no-install-recommends \
    python3-pip python3-setuptools python3-wheel python3-mako python3-yaml python3-packaging \
    pkg-config flex bison clang libclang-dev llvm-15-dev libllvmspirvlib-15-dev \
    libclc-15-dev libclang-15-dev zlib1g-dev libexpat1-dev libzstd-dev \
    && rm -rf /var/lib/apt/lists/*
RUN pip3 install --break-system-packages meson
RUN git clone --depth 1 --branch vulkan-sdk-1.3.290.0 https://github.com/KhronosGroup/SPIRV-Tools.git /tmp/spirv-tools \
    && git clone --depth 1 --branch vulkan-sdk-1.3.290.0 https://github.com/KhronosGroup/SPIRV-Headers.git /tmp/spirv-tools/external/spirv-headers \
    && cmake -S /tmp/spirv-tools -B /tmp/spirv-tools/build -G Ninja -DCMAKE_BUILD_TYPE=Release \
       -DCMAKE_INSTALL_PREFIX=/usr/local -DSPIRV_SKIP_TESTS=ON \
    && cmake --build /tmp/spirv-tools/build -j ${JOBS} && cmake --install /tmp/spirv-tools/build \
    && rm -rf /tmp/spirv-tools
RUN rustup target add aarch64-unknown-linux-gnu \
    && cargo install --locked bindgen-cli cbindgen \
    && rm -rf /usr/local/cargo/registry
