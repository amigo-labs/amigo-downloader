import type {
  HostCaptchaApi,
  HostCryptoApi,
  HostHtmlDocument,
  HostHttpRequest,
  HostHttpResponse,
  HostJavascriptApi,
  HostUtilApi,
} from "./types.js";

export interface HostApi {
  http(request: HostHttpRequest): Promise<HostHttpResponse>;
  html: {
    parse(source: string, baseUrl?: string): HostHtmlDocument;
  };
  crypto: HostCryptoApi;
  util: HostUtilApi;
  javascript: HostJavascriptApi;
  captcha: HostCaptchaApi;
}
