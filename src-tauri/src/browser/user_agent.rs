//! Phone user agents and their portrait screen size. Mirrors the device list
//! in `src/lib/userAgents.ts`: a phone UA also sets the viewport.

/// `(user agent, width, height)` for every phone preset.
const PHONE_SCREENS: &[(&str, u32, u32)] = &[
    ("Mozilla/5.0 (Linux; Android 16; 23127PN0CG Build/BP2A.250605.031.A3; wv) AppleWebKit/537.36 (KHTML, like Gecko) Version/4.0 Chrome/152.0.7977.87 Mobile Safari/537.36", 393, 852),
    ("Mozilla/5.0 (Linux; Android 16; 24129PN74G Build/BP2A.250605.031.A3; wv) AppleWebKit/537.36 (KHTML, like Gecko) Version/4.0 Chrome/153.0.8010.36 Mobile Safari/537.36", 412, 915),
    ("Mozilla/5.0 (Linux; Android 16; 25113PN0EC Build/BP2A.250605.031.A3; wv) AppleWebKit/537.36 (KHTML, like Gecko) Version/4.0 Chrome/151.0.7922.199 Mobile Safari/537.36", 412, 915),
    ("Mozilla/5.0 (Linux; Android 16; PLA110 Build/BP2A.250605.015; wv) AppleWebKit/537.36 (KHTML, like Gecko) Version/4.0 Chrome/138.0.7204.179 Mobile Safari/537.36", 412, 915),
    ("Mozilla/5.0 (Linux; Android 16; PKM110 Build/BP2A.250605.015) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/147.0.7727.137 Mobile Safari/537.36", 412, 915),
    ("Mozilla/5.0 (Linux; Android 14; PJV110 Build/SP1A.210812.016; wv) AppleWebKit/537.36 (KHTML, like Gecko) Version/4.0 Chrome/148.0.7778.178 Mobile Safari/537.36", 412, 915),
    ("Mozilla/5.0 (Linux; Android 14; SM-S911B Build/UP1A.231005.007; wv) AppleWebKit/537.36 (KHTML, like Gecko) Version/4.0 Chrome/153.0.8010.36 Mobile Safari/537.36", 360, 780),
    ("Mozilla/5.0 (Linux; Android 16; SM-S921U1 Build/BP4A.251205.006; wv) AppleWebKit/537.36 (KHTML, like Gecko) Version/4.0 Chrome/151.0.7922.199 Mobile Safari/537.36 com.ionicframework.tornv2301860", 360, 780),
    ("Mozilla/5.0 (Linux; Android 16; SM-S931B Build/BP2A.250605.031.A3; wv) AppleWebKit/537.36 (KHTML, like Gecko) Version/4.0 Chrome/153.0.8010.36 Mobile Safari/537.36", 360, 780),
    ("Mozilla/5.0 (Linux; Android 16; Pixel 10) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/155.0.0.0 Mobile Safari/537.36", 412, 915),
    ("Mozilla/5.0 (Linux; Android 17; Pixel 9 Build/CP2A.260805.005; wv) AppleWebKit/537.36 (KHTML, like Gecko) Version/4.0 Chrome/153.0.8010.36 Mobile Safari/537.36", 412, 915),
    ("Mozilla/5.0 (Linux; Android 15; Pixel 8 Build/AP4A.250105.002; wv) AppleWebKit/537.36 (KHTML, like Gecko) Version/4.0 Chrome/132.0.6834.163 Mobile Safari/537.36", 412, 915),
];

/// Portrait `(width, height)` for a known phone user agent, `None` otherwise.
pub fn phone_screen_for_ua(ua: &str) -> Option<(u32, u32)> {
    PHONE_SCREENS
        .iter()
        .find(|(known, _, _)| *known == ua)
        .map(|(_, w, h)| (*w, *h))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn known_phone_gets_portrait_size() {
        let (ua, w, h) = PHONE_SCREENS[0];
        assert_eq!(phone_screen_for_ua(ua), Some((w, h)));
        assert!(h > w);
        assert_eq!(phone_screen_for_ua("Mozilla/5.0 (X11; Linux x86_64)"), None);
    }
}
