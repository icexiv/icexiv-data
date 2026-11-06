// CPU-only binary stream adapter for Microsoft's generic BC6H signed decoder.
// Input: at most 8192 16-byte blocks. Output: 16 RGBA binary32 pixels per block.
// No TEX, SqPack, game path, graphics device, encoder or other target execution.
#include "DirectXTexP.h"
#include "BC.h"

int main() {
    static_assert(sizeof(float) == 4);
    uint8_t block[16];
    size_t blocks = 0;
    while (true) {
        const size_t n = std::fread(block, 1, sizeof(block), stdin);
        if (n == 0 && std::feof(stdin)) return 0;
        if (n != sizeof(block) || ++blocks > 8192) return 1;
        DirectX::XMVECTOR colors[16];
        DirectX::D3DXDecodeBC6HS(colors, block);
        for (const auto& color : colors) {
            DirectX::XMFLOAT4 value;
            DirectX::XMStoreFloat4(&value, color);
            if (std::fwrite(&value, 1, sizeof(value), stdout) != sizeof(value)) return 2;
        }
    }
}
