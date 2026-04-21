import { afterEach, describe, expect, it } from "vitest";
import { ccf, detect, dlc, rsdf } from "../../src/container/index.js";
import { base64Encode, hexEncode } from "../../src/extraction/encoding.js";
import {
  clearHostApi,
  createMockHostApi,
  setHostApi,
  type HostHttpResponse,
} from "../../src/host/index.js";

function encode(text: string): Uint8Array {
  return new TextEncoder().encode(text);
}

describe("detect", () => {
  afterEach(() => clearHostApi());

  it("detects a hex-only payload as RSDF", () => {
    const host = createMockHostApi().api;
    setHostApi(host);
    const sample = encode("deadbeef1234567890abcdef");
    expect(detect(sample)).toBe("rsdf");
  });

  it("detects a base64 payload as DLC", () => {
    const host = createMockHostApi().api;
    setHostApi(host);
    const sample = encode("a".repeat(120) + "==");
    expect(detect(sample)).toBe("dlc");
  });

  it("returns null on binary payload without magic", () => {
    const host = createMockHostApi().api;
    setHostApi(host);
    const bytes = new Uint8Array([0, 1, 2, 3, 4, 5]);
    expect(detect(bytes)).toBeNull();
  });
});

describe("rsdf.parse", () => {
  afterEach(() => clearHostApi());

  it("decodes URLs via host crypto and base64", () => {
    const urls = ["https://a.test/1", "https://a.test/2"];
    const plainText = urls.map((url) => base64Encode(url)).join("\n");
    const host = createMockHostApi({
      crypto: {
        aesCbcDecrypt: () => new TextEncoder().encode(plainText),
      },
    }).api;
    setHostApi(host);
    const result = rsdf.parse(hexEncode(new Uint8Array([1, 2, 3])));
    expect(result).toEqual(urls);
  });

  it("surfaces ContainerDecryptionFailed when crypto fails", () => {
    const host = createMockHostApi({
      crypto: {
        aesCbcDecrypt: () => {
          throw new Error("bad key");
        },
      },
    }).api;
    setHostApi(host);
    expect(() => rsdf.parse("deadbeef")).toThrow(/AES decryption failed/);
  });
});

describe("ccf.parse", () => {
  afterEach(() => clearHostApi());

  it("parses decrypted XML into CcfLinks", () => {
    const xml = `<package name="pkg"><link><url>https://a.test/1</url><filename>f1.bin</filename><filesize>42</filesize></link></package>`;
    const host = createMockHostApi({
      crypto: {
        aesCbcDecrypt: () => new TextEncoder().encode(xml),
      },
    }).api;
    setHostApi(host);
    const container = ccf.parse(new Uint8Array([1, 2, 3]));
    expect(container.packageName).toBe("pkg");
    expect(container.links).toHaveLength(1);
    expect(container.links[0]?.url).toBe("https://a.test/1");
    expect(container.links[0]?.size).toBe(42);
  });
});

describe("dlc.parse", () => {
  afterEach(() => clearHostApi());

  it("posts service key, decrypts payload, parses links", async () => {
    const innerXml = `<package name="MyPkg" date="2024-01-01"><file><url>https://a.test/x</url><filename>x.bin</filename><size>123</size></file></package>`;
    const innerXmlBase64 = base64Encode(new TextEncoder().encode(innerXml));
    const encryptedPayload = new Uint8Array([0xaa, 0xbb, 0xcc]);
    const encryptedPayloadBase64 = base64Encode(encryptedPayload);

    const serviceKeyBytes = new Uint8Array(16).map((_, index) => index + 1);
    const serviceKeyBase64 = base64Encode(serviceKeyBytes);

    const httpResponses: HostHttpResponse[] = [
      {
        status: 200,
        url: "https://dlc.service.test/jdownloader",
        redirectLocation: null,
        headers: {},
        body: new TextEncoder().encode(`<rc>${serviceKeyBase64}</rc>`),
      },
    ];
    let call = 0;
    const host = createMockHostApi({
      http: () => httpResponses[call++]!,
      crypto: {
        aesCbcDecrypt: (data, key, iv) => {
          expect(Array.from(key)).toEqual(Array.from(serviceKeyBytes));
          expect(Array.from(iv)).toEqual(Array.from(serviceKeyBytes));
          expect(Array.from(data)).toEqual(Array.from(encryptedPayload));
          return new TextEncoder().encode(innerXmlBase64);
        },
      },
    }).api;
    setHostApi(host);
    const serviceSuffix = "x".repeat(88);
    const container = await dlc.parse(encryptedPayloadBase64 + serviceSuffix, {
      keyExchangeEndpoint: "https://dlc.service.test/jdownloader",
    });
    expect(container.packageName).toBe("MyPkg");
    expect(container.uploadDate).toBe("2024-01-01");
    expect(container.links).toHaveLength(1);
    expect(container.links[0]?.url).toBe("https://a.test/x");
    expect(container.links[0]?.size).toBe(123);
  });

  it("fails when service returns non-2xx", async () => {
    const host = createMockHostApi({
      http: () => ({
        status: 500,
        url: "https://service.test/",
        redirectLocation: null,
        headers: {},
        body: new Uint8Array(),
      }),
    }).api;
    setHostApi(host);
    const payload = "a".repeat(90) + "b".repeat(88);
    await expect(
      dlc.parse(payload, { keyExchangeEndpoint: "https://service.test/" }),
    ).rejects.toMatchObject({ code: "ContainerDecryptionFailed" });
  });
});
