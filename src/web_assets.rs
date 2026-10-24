// Pinned KaTeX assets, embedded for fully offline use.
pub fn asset(path: &str) -> Option<(&'static str, &'static [u8])> {
    match path {
        "katex/fonts/KaTeX_AMS-Regular.woff2" => Some((
            "font/woff2",
            include_bytes!("../web/vendor/katex/fonts/KaTeX_AMS-Regular.woff2"),
        )),
        "katex/fonts/KaTeX_Caligraphic-Bold.woff2" => Some((
            "font/woff2",
            include_bytes!("../web/vendor/katex/fonts/KaTeX_Caligraphic-Bold.woff2"),
        )),
        "katex/fonts/KaTeX_Caligraphic-Regular.woff2" => Some((
            "font/woff2",
            include_bytes!("../web/vendor/katex/fonts/KaTeX_Caligraphic-Regular.woff2"),
        )),
        "katex/fonts/KaTeX_Fraktur-Bold.woff2" => Some((
            "font/woff2",
            include_bytes!("../web/vendor/katex/fonts/KaTeX_Fraktur-Bold.woff2"),
        )),
        "katex/fonts/KaTeX_Fraktur-Regular.woff2" => Some((
            "font/woff2",
            include_bytes!("../web/vendor/katex/fonts/KaTeX_Fraktur-Regular.woff2"),
        )),
        "katex/fonts/KaTeX_Main-Bold.woff2" => Some((
            "font/woff2",
            include_bytes!("../web/vendor/katex/fonts/KaTeX_Main-Bold.woff2"),
        )),
        "katex/fonts/KaTeX_Main-BoldItalic.woff2" => Some((
            "font/woff2",
            include_bytes!("../web/vendor/katex/fonts/KaTeX_Main-BoldItalic.woff2"),
        )),
        "katex/fonts/KaTeX_Main-Italic.woff2" => Some((
            "font/woff2",
            include_bytes!("../web/vendor/katex/fonts/KaTeX_Main-Italic.woff2"),
        )),
        "katex/fonts/KaTeX_Main-Regular.woff2" => Some((
            "font/woff2",
            include_bytes!("../web/vendor/katex/fonts/KaTeX_Main-Regular.woff2"),
        )),
        "katex/fonts/KaTeX_Math-BoldItalic.woff2" => Some((
            "font/woff2",
            include_bytes!("../web/vendor/katex/fonts/KaTeX_Math-BoldItalic.woff2"),
        )),
        "katex/fonts/KaTeX_Math-Italic.woff2" => Some((
            "font/woff2",
            include_bytes!("../web/vendor/katex/fonts/KaTeX_Math-Italic.woff2"),
        )),
        "katex/fonts/KaTeX_SansSerif-Bold.woff2" => Some((
            "font/woff2",
            include_bytes!("../web/vendor/katex/fonts/KaTeX_SansSerif-Bold.woff2"),
        )),
        "katex/fonts/KaTeX_SansSerif-Italic.woff2" => Some((
            "font/woff2",
            include_bytes!("../web/vendor/katex/fonts/KaTeX_SansSerif-Italic.woff2"),
        )),
        "katex/fonts/KaTeX_SansSerif-Regular.woff2" => Some((
            "font/woff2",
            include_bytes!("../web/vendor/katex/fonts/KaTeX_SansSerif-Regular.woff2"),
        )),
        "katex/fonts/KaTeX_Script-Regular.woff2" => Some((
            "font/woff2",
            include_bytes!("../web/vendor/katex/fonts/KaTeX_Script-Regular.woff2"),
        )),
        "katex/fonts/KaTeX_Size1-Regular.woff2" => Some((
            "font/woff2",
            include_bytes!("../web/vendor/katex/fonts/KaTeX_Size1-Regular.woff2"),
        )),
        "katex/fonts/KaTeX_Size2-Regular.woff2" => Some((
            "font/woff2",
            include_bytes!("../web/vendor/katex/fonts/KaTeX_Size2-Regular.woff2"),
        )),
        "katex/fonts/KaTeX_Size3-Regular.woff2" => Some((
            "font/woff2",
            include_bytes!("../web/vendor/katex/fonts/KaTeX_Size3-Regular.woff2"),
        )),
        "katex/fonts/KaTeX_Size4-Regular.woff2" => Some((
            "font/woff2",
            include_bytes!("../web/vendor/katex/fonts/KaTeX_Size4-Regular.woff2"),
        )),
        "katex/fonts/KaTeX_Typewriter-Regular.woff2" => Some((
            "font/woff2",
            include_bytes!("../web/vendor/katex/fonts/KaTeX_Typewriter-Regular.woff2"),
        )),
        "katex/katex.min.css" => Some((
            "text/css; charset=utf-8",
            include_bytes!("../web/vendor/katex/katex.min.css"),
        )),
        "katex/katex.min.js" => Some((
            "text/javascript; charset=utf-8",
            include_bytes!("../web/vendor/katex/katex.min.js"),
        )),
        "katex/LICENSE" => Some((
            "text/plain; charset=utf-8",
            include_bytes!("../web/vendor/katex/LICENSE"),
        )),
        _ => None,
    }
}
