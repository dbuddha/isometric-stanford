import { access } from "node:fs/promises";
import { describe, expect, it } from "vitest";
import { startUploadServer } from "../src/node/upload-server.js";

describe("bounded layer upload server", () => {
  it("streams a registered layer through a private temporary file", async () => {
    let acceptedPath = "";
    const upload = await startUploadServer({
      async acceptFile(name, path, byteLength, width, height, pixelFormat): Promise<void> {
        acceptedPath = path;
        await expect(access(path)).resolves.toBeUndefined();
        expect({ byteLength, height, name, pixelFormat, width }).toEqual({
          byteLength: 16,
          height: 2,
          name: "color",
          pixelFormat: "rgba8",
          width: 2,
        });
      },
    });
    try {
      const response = await fetch(`${upload.url}/layer/color`, {
        body: new Uint8Array(16),
        headers: {
          "content-type": "application/octet-stream",
          "x-capture-height": "2",
          "x-capture-pixel-format": "rgba8",
          "x-capture-token": upload.token,
          "x-capture-width": "2",
        },
        method: "POST",
      });
      expect(response.status).toBe(204);
      expect(acceptedPath).not.toBe("");
      await expect(access(acceptedPath)).rejects.toMatchObject({ code: "ENOENT" });
    } finally {
      await upload.close();
    }
  });

  it("rejects a payload whose byte length contradicts its registration", async () => {
    const upload = await startUploadServer({
      async acceptFile(): Promise<void> {
        throw new Error("invalid payload must not reach the sink");
      },
    });
    try {
      const response = await fetch(`${upload.url}/layer/coverage`, {
        body: new Uint8Array(3),
        headers: {
          "content-type": "application/octet-stream",
          "x-capture-height": "2",
          "x-capture-pixel-format": "gray8",
          "x-capture-token": upload.token,
          "x-capture-width": "2",
        },
        method: "POST",
      });
      expect(response.status).toBe(400);
    } finally {
      await upload.close();
    }
  });
});
