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
  // Android phones from brands not covered above.
  {
    id: "vivo-x200-pro",
    name: "Vivo X200 Pro",
    userAgent:
      "Mozilla/5.0 (Linux; Android 15; V2413 Build/AP4A.250105.002; wv) AppleWebKit/537.36 (KHTML, like Gecko) Version/4.0 Chrome/152.0.7977.87 Mobile Safari/537.36",
    screen: { width: 450, height: 1000 },
  },
  {
    id: "vivo-v50",
    name: "Vivo V50",
    userAgent:
      "Mozilla/5.0 (Linux; Android 15; V2427 Build/AP4A.250105.002) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/153.0.8010.36 Mobile Safari/537.36",
    screen: { width: 393, height: 870 },
  },
  {
    id: "iqoo-13",
    name: "iQOO 13",
    userAgent:
      "Mozilla/5.0 (Linux; Android 15; V2408A Build/AP4A.250105.002) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/155.0.0.0 Mobile Safari/537.36",
    screen: { width: 450, height: 1000 },
  },
  {
    id: "honor-magic-7-pro",
    name: "Honor Magic 7 Pro",
    userAgent:
      "Mozilla/5.0 (Linux; Android 15; PTP-N49 Build/AP4A.250105.002; wv) AppleWebKit/537.36 (KHTML, like Gecko) Version/4.0 Chrome/151.0.7922.199 Mobile Safari/537.36",
    screen: { width: 440, height: 980 },
  },
  {
    id: "honor-200-pro",
    name: "Honor 200 Pro",
    userAgent:
      "Mozilla/5.0 (Linux; Android 15; ELP-NX9 Build/AP4A.250105.002) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/152.0.7977.87 Mobile Safari/537.36",
    screen: { width: 412, height: 915 },
  },
  {
    id: "oneplus-13",
    name: "OnePlus 13",
    userAgent:
      "Mozilla/5.0 (Linux; Android 15; CPH2649 Build/AP4A.250105.002) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/153.0.8010.36 Mobile Safari/537.36",
    screen: { width: 450, height: 1000 },
  },
  {
    id: "oneplus-nord-5",
    name: "OnePlus Nord 5",
    userAgent:
      "Mozilla/5.0 (Linux; Android 15; CPH2701 Build/AP4A.250105.002; wv) AppleWebKit/537.36 (KHTML, like Gecko) Version/4.0 Chrome/152.0.7977.87 Mobile Safari/537.36",
    screen: { width: 412, height: 915 },
  },
  {
    id: "realme-gt-7-pro",
    name: "Realme GT 7 Pro",
    userAgent:
      "Mozilla/5.0 (Linux; Android 15; RMX5010 Build/AP4A.250105.002; wv) AppleWebKit/537.36 (KHTML, like Gecko) Version/4.0 Chrome/151.0.7922.199 Mobile Safari/537.36",
    screen: { width: 451, height: 993 },
  },
  {
    id: "realme-14-pro-plus",
    name: "Realme 14 Pro+",
    userAgent:
      "Mozilla/5.0 (Linux; Android 15; RMX5051 Build/AP4A.250105.002) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/152.0.7977.87 Mobile Safari/537.36",
    screen: { width: 393, height: 852 },
  },
  {
    id: "motorola-edge-60-pro",
    name: "Motorola Edge 60 Pro",
    userAgent:
      "Mozilla/5.0 (Linux; Android 15; motorola edge 60 pro Build/AP4A.250105.002) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/153.0.8010.36 Mobile Safari/537.36",
    screen: { width: 425, height: 945 },
  },
  {
    id: "moto-g-power-2026",
    name: "Moto G Power 5G (2026)",
    userAgent:
      "Mozilla/5.0 (Linux; Android 16; moto g power 5G - 2026 Build/BP2A.250605.015; wv) AppleWebKit/537.36 (KHTML, like Gecko) Version/4.0 Chrome/153.0.8010.36 Mobile Safari/537.36",
    screen: { width: 432, height: 960 },
  },
  {
    id: "asus-rog-phone-9-pro",
    name: "Asus ROG Phone 9 Pro",
    userAgent:
      "Mozilla/5.0 (Linux; Android 15; ASUS_AI2501 Build/AP4A.250105.002) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/155.0.0.0 Mobile Safari/537.36",
    screen: { width: 432, height: 960 },
  },
  {
    id: "asus-zenfone-12-ultra",
    name: "Asus Zenfone 12 Ultra",
    userAgent:
      "Mozilla/5.0 (Linux; Android 15; ASUS_AI2502 Build/AP4A.250105.002; wv) AppleWebKit/537.36 (KHTML, like Gecko) Version/4.0 Chrome/152.0.7977.87 Mobile Safari/537.36",
    screen: { width: 432, height: 960 },
  },
  {
    id: "nothing-phone-3",
    name: "Nothing Phone (3)",
    userAgent:
      "Mozilla/5.0 (Linux; Android 16; A024 Build/BP2A.250605.015) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/155.0.0.0 Mobile Safari/537.36",
    screen: { width: 424, height: 928 },
  },
  {
    id: "nothing-phone-3a-pro",
    name: "Nothing Phone (3a) Pro",
    userAgent:
      "Mozilla/5.0 (Linux; Android 15; A059P Build/AP4A.250105.002; wv) AppleWebKit/537.36 (KHTML, like Gecko) Version/4.0 Chrome/153.0.8010.36 Mobile Safari/537.36",
    screen: { width: 412, height: 915 },
  },
  {
    id: "tecno-camon-40-pro",
    name: "Tecno Camon 40 Pro",
    userAgent:
      "Mozilla/5.0 (Linux; Android 15; TECNO CL7 Build/AP4A.250105.002; wv) AppleWebKit/537.36 (KHTML, like Gecko) Version/4.0 Chrome/152.0.7977.87 Mobile Safari/537.36",
    screen: { width: 386, height: 870 },
  },
  {
    id: "tecno-spark-30-pro",
    name: "Tecno Spark 30 Pro",
    userAgent:
      "Mozilla/5.0 (Linux; Android 14; TECNO KL8 Build/SP1A.210812.016) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/138.0.7204.179 Mobile Safari/537.36",
    screen: { width: 360, height: 800 },
  },
  {
    id: "infinix-note-50-pro-plus",
    name: "Infinix Note 50 Pro+",
    userAgent:
      "Mozilla/5.0 (Linux; Android 15; Infinix X6858 Build/AP4A.250105.002; wv) AppleWebKit/537.36 (KHTML, like Gecko) Version/4.0 Chrome/151.0.7922.199 Mobile Safari/537.36",
    screen: { width: 412, height: 915 },
  },
  {
    id: "infinix-zero-40",
    name: "Infinix Zero 40",
    userAgent:
      "Mozilla/5.0 (Linux; Android 14; Infinix X6861 Build/SP1A.210812.016) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/147.0.7727.137 Mobile Safari/537.36",
    screen: { width: 393, height: 886 },
  },
  {
    id: "nubia-z70-ultra",
    name: "Nubia Z70 Ultra",
    userAgent:
      "Mozilla/5.0 (Linux; Android 15; nubia NX736J Build/AP4A.250105.002) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/153.0.8010.36 Mobile Safari/537.36",
    screen: { width: 450, height: 996 },
  },
];

/** Finds the device whose user agent matches exactly, if any. */
export function phoneByUserAgent(ua: string | null | undefined): PhoneDevice | undefined {
  if (!ua) return undefined;
  return PHONE_DEVICES.find((d) => d.userAgent === ua);
}
