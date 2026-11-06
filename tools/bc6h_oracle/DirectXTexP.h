// Standalone CPU-oracle build context, not an implementation of BC decoding.
// The unmodified Microsoft BC6HBC7.cpp includes this in place of the library's
// platform/private umbrella header. BC.h and DirectXMath are upstream originals.
#pragma once
#include <algorithm>
#include <cassert>
#include <cfloat>
#include <climits>
#include <cmath>
#include <cstddef>
#include <cstdint>
#include <cstdio>
#include <cstdlib>
#include <cstring>
#include <utility>
#define UNREFERENCED_PARAMETER(value) (void)(value)
// Upstream DirectXTex.h enum value needed to compile the unused BC7 encoder.
namespace DirectX { constexpr uint32_t TEX_COMPRESS_BC7_QUICK = 0x100000; }
