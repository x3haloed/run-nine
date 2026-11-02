// Project-authored MIT OR Apache-2.0 adapter for external MIT ResourceDASM.
// Compile against the revision-pinned reference sources, never into RUN9.
#include "System.hh"
#include <cstring>
#include <iostream>
#include <stdexcept>
#include <string>

static void reply(unsigned char status, const std::string& data) {
  std::cout.put(status);
  for (int shift = 24; shift >= 0; shift -= 8) std::cout.put(data.size() >> shift);
  std::cout.write(data.data(), data.size());
  std::cout.flush();
}

int main() {
  static_assert(sizeof(ResourceDASM::CompressedResourceHeader) == 18);
  for (;;) {
    int attr = std::cin.get();
    if (attr == EOF) return 0;
    uint32_t size = 0;
    for (int i = 0; i < 4; i++) {
      int byte = std::cin.get();
      if (byte == EOF) return 2;
      size = (size << 8) | byte;
    }
    if (size > 64 * 1024 * 1024) return 3;
    std::string input(size, '\0');
    if (!std::cin.read(input.data(), size)) return 2;
    if (!(attr & 1) || input.size() < 4 || input.compare(0, 4, "\xA8\x9F\x65\x72")) {
      reply(0, "");
      continue;
    }
    try {
      if (input.size() < 18) throw std::runtime_error("short header");
      ResourceDASM::CompressedResourceHeader header;
      std::memcpy(&header, input.data(), sizeof(header));
      if (header.decompressed_size > 64 * 1024 * 1024) throw std::runtime_error("oracle output limit");
      std::string output;
      if (header.header_version == 8 && header.version.v8.dcmp_resource_id == 0)
        output = ResourceDASM::decompress_system0(header, input.data() + 18, size - 18);
      else if (header.header_version == 9 && header.version.v9.dcmp_resource_id == 3)
        output = ResourceDASM::decompress_system3(header, input.data() + 18, size - 18);
      else throw std::runtime_error("unsupported reference decoder");
      reply(1, output);
    } catch (const std::exception& error) {
      reply(2, error.what());
    }
  }
}
