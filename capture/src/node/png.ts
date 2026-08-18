import { deflateSync } from "node:zlib";

const PNG_SIGNATURE = Buffer.from([137, 80, 78, 71, 13, 10, 26, 10]);
const CRC_TABLE = Array.from({ length: 256 }, (_, index) => {
  let value = index;
  for (let bit = 0; bit < 8; bit += 1) {
    value = (value & 1) === 1 ? 0xedb88320 ^ (value >>> 1) : value >>> 1;
  }
  return value >>> 0;
});

function crc32(bytes: Buffer): number {
  let crc = 0xffffffff;
  for (const byte of bytes) {
    const lookup = (crc ^ byte) & 0xff;
    crc = (CRC_TABLE[lookup] ?? 0) ^ (crc >>> 8);
  }
  return (crc ^ 0xffffffff) >>> 0;
}

function chunk(kind: string, data: Buffer): Buffer {
  const type = Buffer.from(kind, "ascii");
  const output = Buffer.allocUnsafe(12 + data.length);
  output.writeUInt32BE(data.length, 0);
  type.copy(output, 4);
  data.copy(output, 8);
  output.writeUInt32BE(crc32(output.subarray(4, 8 + data.length)), 8 + data.length);
  return output;
}

export function encodePng(
  pixels: Uint8Array,
  width: number,
  height: number,
  colorType: 0 | 6,
): Buffer {
  const channels = colorType === 0 ? 1 : 4;
  const expectedLength = width * height * channels;
  if (pixels.length !== expectedLength || width <= 0 || height <= 0) {
    throw new Error("raw PNG payload does not match its dimensions");
  }
  const stride = width * channels;
  const scanlines = Buffer.allocUnsafe(height * (stride + 1));
  for (let row = 0; row < height; row += 1) {
    const target = row * (stride + 1);
    scanlines[target] = 0;
    scanlines.set(pixels.subarray(row * stride, (row + 1) * stride), target + 1);
  }
  const header = Buffer.alloc(13);
  header.writeUInt32BE(width, 0);
  header.writeUInt32BE(height, 4);
  header[8] = 8;
  header[9] = colorType;
  return Buffer.concat([
    PNG_SIGNATURE,
    chunk("IHDR", header),
    chunk("IDAT", deflateSync(scanlines, { level: 9 })),
    chunk("IEND", Buffer.alloc(0)),
  ]);
}
