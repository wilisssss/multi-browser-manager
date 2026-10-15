/** A phone the profile can impersonate: its user agent and portrait screen. */
export interface PhoneDevice {
  id: string;
  name: string;
  userAgent: string;
  /** Portrait size in CSS pixels, applied automatically for phone UAs. */
  screen: { width: number; height: number };
}

export const PHONE_DEVICES: PhoneDevice[] = [
  {
    id: "xiaomi-14",
    name: "Xiaomi 14",
    userAgent:
      "Mozilla/5.0 (Linux; Android 16; 23127PN0CG Build/BP2A.250605.031.A3; wv) AppleWebKit/537.36 (KHTML, like Gecko) Version/4.0 Chrome/152.0.7977.87 Mobile Safari/537.36",
    screen: { width: 393, height: 852 },
  },
  {
    id: "xiaomi-15",
    name: "Xiaomi 15",
    userAgent:
      "Mozilla/5.0 (Linux; Android 16; 24129PN74G Build/BP2A.250605.031.A3; wv) AppleWebKit/537.36 (KHTML, like Gecko) Version/4.0 Chrome/153.0.8010.36 Mobile Safari/537.36",
    screen: { width: 412, height: 915 },
  },
  {
    id: "xiaomi-17",
    name: "Xiaomi 17",
    userAgent:
      "Mozilla/5.0 (Linux; Android 16; 25113PN0EC Build/BP2A.250605.031.A3; wv) AppleWebKit/537.36 (KHTML, like Gecko) Version/4.0 Chrome/151.0.7922.199 Mobile Safari/537.36",
    screen: { width: 412, height: 915 },
  },
  {
    id: "oppo-reno-14",
    name: "Oppo Reno 14",
    userAgent:
      "Mozilla/5.0 (Linux; Android 16; PLA110 Build/BP2A.250605.015; wv) AppleWebKit/537.36 (KHTML, like Gecko) Version/4.0 Chrome/138.0.7204.179 Mobile Safari/537.36",
    screen: { width: 412, height: 915 },
  },
  {
    id: "oppo-reno-13",
    name: "Oppo Reno 13",
    userAgent:
      "Mozilla/5.0 (Linux; Android 16; PKM110 Build/BP2A.250605.015) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/147.0.7727.137 Mobile Safari/537.36",
    screen: { width: 412, height: 915 },
  },
  {
    id: "oppo-reno-12",
    name: "Oppo Reno 12",
    userAgent:
      "Mozilla/5.0 (Linux; Android 14; PJV110 Build/SP1A.210812.016; wv) AppleWebKit/537.36 (KHTML, like Gecko) Version/4.0 Chrome/148.0.7778.178 Mobile Safari/537.36",
    screen: { width: 412, height: 915 },
  },
  {
    id: "samsung-s23",
    name: "Samsung S23",
    userAgent:
      "Mozilla/5.0 (Linux; Android 14; SM-S911B Build/UP1A.231005.007; wv) AppleWebKit/537.36 (KHTML, like Gecko) Version/4.0 Chrome/153.0.8010.36 Mobile Safari/537.36",
    screen: { width: 360, height: 780 },
  },
  {
    id: "samsung-s24",
    name: "Samsung S24",
    userAgent:
      "Mozilla/5.0 (Linux; Android 16; SM-S921U1 Build/BP4A.251205.006; wv) AppleWebKit/537.36 (KHTML, like Gecko) Version/4.0 Chrome/151.0.7922.199 Mobile Safari/537.36 com.ionicframework.tornv2301860",
    screen: { width: 360, height: 780 },
  },
  {
    id: "samsung-s25",
    name: "Samsung S25",
    userAgent:
      "Mozilla/5.0 (Linux; Android 16; SM-S931B Build/BP2A.250605.031.A3; wv) AppleWebKit/537.36 (KHTML, like Gecko) Version/4.0 Chrome/153.0.8010.36 Mobile Safari/537.36",
    screen: { width: 360, height: 780 },
  },
  {
    id: "pixel-10",
    name: "Google Pixel 10",
    userAgent:
      "Mozilla/5.0 (Linux; Android 16; Pixel 10) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/155.0.0.0 Mobile Safari/537.36",
    screen: { width: 412, height: 915 },
  },
  {
    id: "pixel-9",
    name: "Google Pixel 9",
    userAgent:
      "Mozilla/5.0 (Linux; Android 17; Pixel 9 Build/CP2A.260805.005; wv) AppleWebKit/537.36 (KHTML, like Gecko) Version/4.0 Chrome/153.0.8010.36 Mobile Safari/537.36",
    screen: { width: 412, height: 915 },
  },
  {
    id: "pixel-8",
    name: "Google Pixel 8",
    userAgent:
      "Mozilla/5.0 (Linux; Android 15; Pixel 8 Build/AP4A.250105.002; wv) AppleWebKit/537.36 (KHTML, like Gecko) Version/4.0 Chrome/132.0.6834.163 Mobile Safari/537.36",
    screen: { width: 412, height: 915 },
  },
];

/** Finds the device whose user agent matches exactly, if any. */
export function phoneByUserAgent(ua: string | null | undefined): PhoneDevice | undefined {
  if (!ua) return undefined;
  return PHONE_DEVICES.find((d) => d.userAgent === ua);
}
