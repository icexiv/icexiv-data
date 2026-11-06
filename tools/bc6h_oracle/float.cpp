// Generic Microsoft DirectXMath CPU numeric oracle; no TEX or FFXIV parsing.
// Argument h: binary16 scalar stream; p: unsigned R11G11B10 packed u32 stream.
// Output is the library's chosen RGBA view (one-channel RGB replication/A=1).
#include "DirectXTexP.h"
#include <DirectXMath.h>
#include <DirectXPackedVector.h>

int main(int argc, char** argv) {
    if (argc != 2 || (std::strcmp(argv[1], "h") && std::strcmp(argv[1], "p"))) return 1;
    const bool half = argv[1][0] == 'h';
    const size_t stride = half ? 2 : 4;
    size_t pixels = 0;
    uint32_t word = 0;
    while (true) {
        const size_t n = std::fread(&word, 1, stride, stdin);
        if (n == 0 && std::feof(stdin)) return 0;
        if (n != stride || ++pixels > 1048576) return 2;
        DirectX::XMFLOAT4 value;
        if (half) {
            const float v = DirectX::PackedVector::XMConvertHalfToFloat(static_cast<uint16_t>(word));
            value = DirectX::XMFLOAT4(v, v, v, 1.0f);
        } else {
            const DirectX::PackedVector::XMFLOAT3PK packed(word);
            const auto color = DirectX::PackedVector::XMLoadFloat3PK(&packed);
            DirectX::XMStoreFloat4(&value, color);
            value.w = 1.0f;
        }
        if (std::fwrite(&value, 1, sizeof(value), stdout) != sizeof(value)) return 3;
    }
}
