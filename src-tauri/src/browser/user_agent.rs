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
    // Android phones from brands not covered above.
    ("Mozilla/5.0 (Linux; Android 15; V2413 Build/AP4A.250105.002; wv) AppleWebKit/537.36 (KHTML, like Gecko) Version/4.0 Chrome/152.0.7977.87 Mobile Safari/537.36", 450, 1000),
    ("Mozilla/5.0 (Linux; Android 15; V2427 Build/AP4A.250105.002) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/153.0.8010.36 Mobile Safari/537.36", 393, 870),
    ("Mozilla/5.0 (Linux; Android 15; V2408A Build/AP4A.250105.002) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/155.0.0.0 Mobile Safari/537.36", 450, 1000),
    ("Mozilla/5.0 (Linux; Android 15; PTP-N49 Build/AP4A.250105.002; wv) AppleWebKit/537.36 (KHTML, like Gecko) Version/4.0 Chrome/151.0.7922.199 Mobile Safari/537.36", 440, 980),
    ("Mozilla/5.0 (Linux; Android 15; ELP-NX9 Build/AP4A.250105.002) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/152.0.7977.87 Mobile Safari/537.36", 412, 915),
    ("Mozilla/5.0 (Linux; Android 15; CPH2649 Build/AP4A.250105.002) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/153.0.8010.36 Mobile Safari/537.36", 450, 1000),
    ("Mozilla/5.0 (Linux; Android 15; CPH2701 Build/AP4A.250105.002; wv) AppleWebKit/537.36 (KHTML, like Gecko) Version/4.0 Chrome/152.0.7977.87 Mobile Safari/537.36", 412, 915),
    ("Mozilla/5.0 (Linux; Android 15; RMX5010 Build/AP4A.250105.002; wv) AppleWebKit/537.36 (KHTML, like Gecko) Version/4.0 Chrome/151.0.7922.199 Mobile Safari/537.36", 451, 993),
    ("Mozilla/5.0 (Linux; Android 15; RMX5051 Build/AP4A.250105.002) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/152.0.7977.87 Mobile Safari/537.36", 393, 852),
    ("Mozilla/5.0 (Linux; Android 15; motorola edge 60 pro Build/AP4A.250105.002) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/153.0.8010.36 Mobile Safari/537.36", 425, 945),
    ("Mozilla/5.0 (Linux; Android 16; moto g power 5G - 2026 Build/BP2A.250605.015; wv) AppleWebKit/537.36 (KHTML, like Gecko) Version/4.0 Chrome/153.0.8010.36 Mobile Safari/537.36", 432, 960),
    ("Mozilla/5.0 (Linux; Android 15; ASUS_AI2501 Build/AP4A.250105.002) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/155.0.0.0 Mobile Safari/537.36", 432, 960),
    ("Mozilla/5.0 (Linux; Android 15; ASUS_AI2502 Build/AP4A.250105.002; wv) AppleWebKit/537.36 (KHTML, like Gecko) Version/4.0 Chrome/152.0.7977.87 Mobile Safari/537.36", 432, 960),
    ("Mozilla/5.0 (Linux; Android 16; A024 Build/BP2A.250605.015) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/155.0.0.0 Mobile Safari/537.36", 424, 928),
    ("Mozilla/5.0 (Linux; Android 15; A059P Build/AP4A.250105.002; wv) AppleWebKit/537.36 (KHTML, like Gecko) Version/4.0 Chrome/153.0.8010.36 Mobile Safari/537.36", 412, 915),
    ("Mozilla/5.0 (Linux; Android 15; TECNO CL7 Build/AP4A.250105.002; wv) AppleWebKit/537.36 (KHTML, like Gecko) Version/4.0 Chrome/152.0.7977.87 Mobile Safari/537.36", 386, 870),
    ("Mozilla/5.0 (Linux; Android 14; TECNO KL8 Build/SP1A.210812.016) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/138.0.7204.179 Mobile Safari/537.36", 360, 800),
    ("Mozilla/5.0 (Linux; Android 15; Infinix X6858 Build/AP4A.250105.002; wv) AppleWebKit/537.36 (KHTML, like Gecko) Version/4.0 Chrome/151.0.7922.199 Mobile Safari/537.36", 412, 915),
    ("Mozilla/5.0 (Linux; Android 14; Infinix X6861 Build/SP1A.210812.016) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/147.0.7727.137 Mobile Safari/537.36", 393, 886),
    ("Mozilla/5.0 (Linux; Android 15; nubia NX736J Build/AP4A.250105.002) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/153.0.8010.36 Mobile Safari/537.36", 450, 996),
];

/// Portrait `(width, height)` for a known phone user agent, `None` otherwise.
pub fn phone_screen_for_ua(ua: &str) -> Option<(u32, u32)> {
    PHONE_SCREENS
        .iter()
        .find(|(known, _, _)| *known == ua)
        .map(|(_, w, h)| (*w, *h))
}

/// Device pixel ratio for a phone screen width. Android reports the density
/// bucket as a clean value; picking it by physical width keeps the number
/// plausible (1080p at 360 dp → 3.0, 1080p at 432 dp → 2.5, Pixel 1080p at
/// 412 dp → 2.625) instead of the host desktop's 1.
pub fn phone_dpr_for_width(width: u32) -> f64 {
    match width {
        432 => 2.5,
        412 => 2.625,
        360 => 3.0,
        _ => 2.75,
    }
}

/// `(user-agent model token, unmasked WebGL vendor, unmasked WebGL renderer)`
/// for the phone presets: what the real device answers through
/// `WEBGL_debug_renderer_info`. Chrome on Android exposes the extension, so a
/// desktop GPU string ("ANGLE (AMD, Radeon ...)") under an Android UA is a
/// spoof tell; the matching mobile GPU is not.
const PHONE_GPUS: &[(&str, &str, &str)] = &[
    ("23127PN0CG", "Qualcomm", "Adreno (TM) 750"),
    ("24129PN74G", "Qualcomm", "Adreno (TM) 830"),
    ("25113PN0EC", "Qualcomm", "Adreno (TM) 840"),
    ("PLA110", "ARM", "Mali-G615 MC6"),
    ("PKM110", "ARM", "Mali-G615 MC6"),
    ("PJV110", "ARM", "Mali-G615 MC2"),
    ("SM-S911B", "Qualcomm", "Adreno (TM) 740"),
    ("SM-S921U1", "Qualcomm", "Adreno (TM) 750"),
    ("SM-S931B", "Qualcomm", "Adreno (TM) 830"),
    ("Pixel 10", "Imagination Technologies", "PowerVR D-Series DXT-48-1536"),
    ("Pixel 9", "ARM", "Mali-G715-Immortalis MC7"),
    ("Pixel 8", "ARM", "Mali-G715-Immortalis MC10"),
    ("V2413", "ARM", "Mali-G925-Immortalis MC12"),
    ("V2427", "Qualcomm", "Adreno (TM) 720"),
    ("V2408A", "Qualcomm", "Adreno (TM) 830"),
    ("PTP-N49", "Qualcomm", "Adreno (TM) 830"),
    ("ELP-NX9", "Qualcomm", "Adreno (TM) 735"),
    ("CPH2649", "Qualcomm", "Adreno (TM) 830"),
    ("CPH2701", "Qualcomm", "Adreno (TM) 735"),
    ("RMX5010", "Qualcomm", "Adreno (TM) 830"),
    ("RMX5051", "ARM", "Mali-G615 MC2"),
    ("motorola edge 60 pro", "ARM", "Mali-G615 MC6"),
    ("moto g power 5G - 2026", "ARM", "Mali-G57 MC2"),
    ("ASUS_AI2501", "Qualcomm", "Adreno (TM) 830"),
    ("ASUS_AI2502", "Qualcomm", "Adreno (TM) 830"),
    ("A024", "Qualcomm", "Adreno (TM) 825"),
    ("A059P", "Qualcomm", "Adreno (TM) 810"),
    ("TECNO CL7", "ARM", "Mali-G57 MC2"),
    ("TECNO KL8", "ARM", "Mali-G57 MC2"),
    ("Infinix X6858", "ARM", "Mali-G615 MC6"),
    ("Infinix X6861", "ARM", "Mali-G610 MC6"),
    ("nubia NX736J", "Qualcomm", "Adreno (TM) 830"),
];

/// `(vendor, renderer)` the device would report as its unmasked WebGL GPU.
pub fn phone_gpu_for_ua(ua: &str) -> Option<(&'static str, &'static str)> {
    PHONE_GPUS
        .iter()
        .find(|(token, _, _)| ua.contains(token))
        .map(|(_, vendor, renderer)| (*vendor, *renderer))
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

    #[test]
    fn every_preset_is_portrait() {
        for (ua, w, h) in PHONE_SCREENS {
            assert!(h > w, "screen must be portrait for {ua}");
        }
    }

    #[test]
    fn every_preset_maps_to_a_gpu() {
        for (ua, _, _) in PHONE_SCREENS {
            let (vendor, renderer) =
                phone_gpu_for_ua(ua).unwrap_or_else(|| panic!("no GPU mapping for {ua}"));
            assert!(!vendor.is_empty() && !renderer.is_empty());
        }
    }

    #[test]
    fn gpu_lookup_is_per_device() {
        assert_eq!(
            phone_gpu_for_ua(PHONE_SCREENS[0].0),
            Some(("Qualcomm", "Adreno (TM) 750"))
        );
        assert_eq!(
            phone_gpu_for_ua("Mozilla/5.0 (Linux; Android 15; Whatever)"),
            None
        );
    }

    #[test]
    fn dpr_follows_the_density_bucket() {
        assert_eq!(phone_dpr_for_width(412), 2.625);
        assert_eq!(phone_dpr_for_width(360), 3.0);
        assert_eq!(phone_dpr_for_width(432), 2.5);
        assert_eq!(phone_dpr_for_width(450), 2.75);
    }

    /// The frontend list (`src/lib/userAgents.ts`) is the user-visible source
    /// of truth; this mirror drives `--window-size` and the mobile emulation.
    /// A drift between the two silently drops the screen override, so the
    /// test parses the TS file and compares every entry.
    #[test]
    fn mirror_matches_the_frontend_device_list() {
        let ts = include_str!("../../../src/lib/userAgents.ts");
        let mut rest = ts;
        let mut seen = 0usize;
        while let Some(pos) = rest.find("userAgent:") {
            rest = &rest[pos + "userAgent:".len()..];
            // The interface declaration (`userAgent: string;`) has no quoted
            // value after it — skip anything that is not a UA string.
            let Some(quote) = rest.find('"') else { break };
            let after = &rest[quote + 1..];
            let Some(end) = after.find('"') else { break };
            let ua = &after[..end];
            rest = &after[end..];
            if !ua.starts_with("Mozilla/5.0 ") {
                continue;
            }

            // This entry's screen follows before the next `userAgent:`.
            let width = digits_after(rest, "width:");
            let height = digits_after(rest, "height:");
            assert_eq!(
                phone_screen_for_ua(ua),
                Some((width, height)),
                "frontend and backend disagree about {ua}"
            );
            seen += 1;
        }
        assert_eq!(
            seen,
            PHONE_SCREENS.len(),
            "frontend and backend device lists differ in size"
        );
    }

    fn digits_after(text: &str, marker: &str) -> u32 {
        let at = text
            .find(marker)
            .unwrap_or_else(|| panic!("missing `{marker}` in: {text}"));
        let digits: String = text[at + marker.len()..]
            .trim_start()
            .chars()
            .take_while(|c| c.is_ascii_digit())
            .collect();
        digits
            .parse()
            .unwrap_or_else(|_| panic!("no number after `{marker}`"))
    }
}
