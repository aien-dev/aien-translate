//! Languages offered, keyed by the ids translate.js uses.

pub struct Lang {
    /// translate.js language id, e.g. "chinese_simplified".
    pub id: &'static str,
    /// Name shown in the translate.js language picker.
    pub name: &'static str,
    /// English name, used in the prompt to the model.
    pub english: &'static str,
    /// ISO-style code, reported as translate.js `serviceId`.
    pub code: &'static str,
}

const fn l(
    id: &'static str,
    name: &'static str,
    english: &'static str,
    code: &'static str,
) -> Lang {
    Lang {
        id,
        name,
        english,
        code,
    }
}

pub const LANGS: &[Lang] = &[
    l("english", "English", "English", "en"),
    l(
        "chinese_simplified",
        "简体中文",
        "Simplified Chinese",
        "zh-CN",
    ),
    l(
        "chinese_traditional",
        "繁體中文",
        "Traditional Chinese",
        "zh-TW",
    ),
    l("japanese", "日本語", "Japanese", "ja"),
    l("korean", "한국어", "Korean", "ko"),
    l("spanish", "Español", "Spanish", "es"),
    l("french", "Français", "French", "fr"),
    l("deutsch", "Deutsch", "German", "de"),
    l("portuguese", "Português", "Portuguese", "pt"),
    l("italian", "Italiano", "Italian", "it"),
    l("russian", "Русский", "Russian", "ru"),
    l("ukrainian", "Українська", "Ukrainian", "uk"),
    l("polish", "Polski", "Polish", "pl"),
    l("dutch", "Nederlands", "Dutch", "nl"),
    l("swedish", "Svenska", "Swedish", "sv"),
    l("danish", "Dansk", "Danish", "da"),
    l("norwegian", "Norsk", "Norwegian", "no"),
    l("finnish", "Suomi", "Finnish", "fi"),
    l("czech", "Čeština", "Czech", "cs"),
    l("romanian", "Română", "Romanian", "ro"),
    l("hungarian", "Magyar", "Hungarian", "hu"),
    l("greek", "Ελληνικά", "Greek", "el"),
    l("turkish", "Türkçe", "Turkish", "tr"),
    l("arabic", "العربية", "Arabic", "ar"),
    l("persian", "فارسی", "Persian", "fa"),
    l("hindi", "हिन्दी", "Hindi", "hi"),
    l("bengali", "বাংলা", "Bengali", "bn"),
    l("thai", "ไทย", "Thai", "th"),
    l("vietnamese", "Tiếng Việt", "Vietnamese", "vi"),
    l("indonesian", "Bahasa Indonesia", "Indonesian", "id"),
    l("malay", "Bahasa Melayu", "Malay", "ms"),
];

pub fn find(id: &str) -> Option<&'static Lang> {
    let id = id.trim();
    LANGS
        .iter()
        .find(|l| l.id.eq_ignore_ascii_case(id) || l.code.eq_ignore_ascii_case(id))
}
