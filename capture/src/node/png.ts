import { createHash } from "node:crypto";
import { open } from "node:fs/promises";
import { Readable } from "node:stream";
import { createDeflate, deflateSync } from "node:zlib";

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

export interface WrittenPng {
  byteLength: number;
  sha256: string;
}

async function writeComplete(
  handle: Awaited<ReturnType<typeof open>>,
  bytes: Uint8Array,
): Promise<void> {
  let offset = 0;
  while (offset < bytes.length) {
    const { bytesWritten } = await handle.write(bytes, offset, bytes.length - offset, null);
    if (bytesWritten === 0) {
      throw new Error("PNG output stopped before all bytes were written");
    }
    offset += bytesWritten;
  }
}

function validatePayload(
  pixels: Uint8Array,
  width: number,
  height: number,
  colorType: 0 | 6,
): number {
  const channels = colorType === 0 ? 1 : 4;
  const expectedLength = width * height * channels;
  if (pixels.length !== expectedLength || width <= 0 || height <= 0) {
    throw new Error("raw PNG payload does not match its dimensions");
  }
  return channels;
}

export function encodePng(
  pixels: Uint8Array,
  width: number,
  height: number,
  colorType: 0 | 6,
): Buffer {
  const channels = validatePayload(pixels, width, height, colorType);
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

export async function writePngFile(
  path: string,
  pixels: Uint8Array,
  width: number,
  height: number,
  colorType: 0 | 6,
): Promise<WrittenPng> {
  const channels = validatePayload(pixels, width, height, colorType);
  const stride = width * channels;
  function* rows(): Generator<Uint8Array> {
    for (let row = 0; row < height; row += 1) {
      yield pixels.subarray(row * stride, (row + 1) * stride);
    }
  }
  return await writePngRows(path, rows(), width, height, colorType);
}

async function writePngRows(
  path: string,
  rows: Iterable<Uint8Array> | AsyncIterable<Uint8Array>,
  width: number,
  height: number,
  colorType: 0 | 6,
): Promise<WrittenPng> {
  const channels = colorType === 0 ? 1 : 4;
  const stride = width * channels;
  const header = Buffer.alloc(13);
  header.writeUInt32BE(width, 0);
  header.writeUInt32BE(height, 4);
  header[8] = 8;
  header[9] = colorType;

  const digest = createHash("sha256");
  let byteLength = 0;
  const handle = await open(path, "wx", 0o600);
  const writeChunk = async (bytes: Uint8Array): Promise<void> => {
    digest.update(bytes);
    await writeComplete(handle, bytes);
    byteLength += bytes.length;
  };

  try {
    await writeChunk(PNG_SIGNATURE);
    await writeChunk(chunk("IHDR", header));

    async function* scanlines(): AsyncGenerator<Buffer> {
      let rowCount = 0;
      for await (const row of rows) {
        if (rowCount >= height || row.length !== stride) {
          throw new Error("raw PNG row stream violates its dimensions");
        }
        const scanline = Buffer.allocUnsafe(stride + 1);
        scanline[0] = 0;
        scanline.set(row, 1);
        rowCount += 1;
        yield scanline;
      }
      if (rowCount !== height) {
        throw new Error("raw PNG row stream ended before its declared height");
      }
    }

    const compressed = Readable.from(scanlines()).pipe(
      createDeflate({ chunkSize: 64 * 1024, level: 9 }),
    );
    for await (const bytes of compressed) {
      await writeChunk(chunk("IDAT", Buffer.from(bytes)));
    }
    await writeChunk(chunk("IEND", Buffer.alloc(0)));
    await handle.sync();
  } finally {
    await handle.close();
  }

  return { byteLength, sha256: digest.digest("hex") };
}

export async function writePngFileFromRaw(
  outputPath: string,
  rawPath: string,
  width: number,
  height: number,
  colorType: 0 | 6,
): Promise<WrittenPng> {
  const channels = colorType === 0 ? 1 : 4;
  const stride = width * channels;
  const source = await open(rawPath, "r");
  async function* rows(): AsyncGenerator<Buffer> {
    try {
      for (let row = 0; row < height; row += 1) {
        const bytes = Buffer.allocUnsafe(stride);
        const { bytesRead } = await source.read(bytes, 0, stride, row * stride);
        if (bytesRead !== stride) {
          throw new Error("raw PNG source ended before its declared dimensions");
        }
        yield bytes;
      }
      const trailing = Buffer.allocUnsafe(1);
      const { bytesRead } = await source.read(trailing, 0, 1, height * stride);
      if (bytesRead !== 0) {
        throw new Error("raw PNG source exceeds its declared dimensions");
      }
    } finally {
      await source.close();
    }
  }
  return await writePngRows(outputPath, rows(), width, height, colorType);
}
