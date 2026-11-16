// Paper theme colors, ported from the Paper Zed theme (light + dark).
// Stored as 0xRRGGBBAA u32 values; convert at the use site with
// `gpui::rgba(...)` (or `gpui::rgb(...)` for fully-opaque colors).

// ========================================
// DARK — "Paper Dark"
// ========================================

// Backgrounds: DARKER → LIGHTER
pub const DARK_BG_PRIMARY: u32 = 0x20201eff; // 3rd darkest — brighter than SECONDARY, darker than TERTIARY and DARKER
pub const DARK_BG_SECONDARY: u32 = 0x171715ff; // Darkest — darkest background
pub const DARK_BG_TERTIARY: u32 = 0x242422ff; // 2nd brightest — brighter than PRIMARY and SECONDARY, darker than DARKER
pub const DARK_BG_DARKER: u32 = 0x252523ff; // Brightest — lightest background

// Borders: DARKER → LIGHTER
pub const DARK_BORDER_COLOR: u32 = 0x30302eff; // 2nd darkest
pub const DARK_MENU_BORDER_COLOR: u32 = 0x292927ff; // Darkest
pub const DARK_COMPONENT_BORDER_COLOR: u32 = 0x3a3a37ff; // Brightest
pub const DARK_CHROME_BORDER_COLOR: u32 = 0x2e2e2cff; // 3rd darkest — between menu and regular borders

// Menus
pub const DARK_MENU_BG: u32 = DARK_BG_SECONDARY; // Same brightness as SECONDARY — darkest background
pub const DARK_MENU_HOVER_BG: u32 = 0x2a2a28ff; // Brighter than MENU_BG, darker than regular BORDER

// Text: DARKER → LIGHTER
pub const DARK_TEXT_PRIMARY: u32 = 0xd2d2ceff; // Brightest — primary text
pub const DARK_TEXT_SECONDARY: u32 = 0x858580ff; // Darker than PRIMARY
pub const DARK_EMPTY_TEXT_PRIMARY: u32 = 0x686864ff; // Darker than SECONDARY
pub const DARK_EMPTY_TEXT_SECONDARY: u32 = 0x555552ff; // Darkest — least prominent text

pub const DARK_SHADOW_COLOR: u32 = 0x00000040; // Darkest possible — semi-transparent black
pub const DARK_ITEM_SHADOW_COLOR: u32 = 0x00000024; // Lighter than SHADOW_COLOR — more subtle shadow

// High-contrast action buttons
pub const DARK_BUTTON_BACKGROUND: u32 = 0xe2e2ddff; // Very bright — brighter than all dark backgrounds
pub const DARK_BUTTON_TEXT: u32 = 0x20201eff; // Same brightness as BG_PRIMARY
pub const DARK_BUTTON_BORDER_COLOR: u32 = 0x777772ff; // Darker than button background, lighter than secondary text

// Accents
pub const DARK_ACCENT: u32 = 0x7897caff; // Darker than ACCENT_BORDER
pub const DARK_ACCENT_BORDER: u32 = 0x89a7d8ff; // Brighter than ACCENT

// Status tones (success / warning / error / info + tints for backgrounds)
pub const DARK_SUCCESS: u32 = 0x819b76ff; // Medium brightness — green
pub const DARK_SUCCESS_BG: u32 = 0x819b7614; // Same hue as SUCCESS, very transparent
pub const DARK_WARNING: u32 = 0xb99958ff; // Medium brightness — gold
pub const DARK_WARNING_BG: u32 = 0xb9995814; // Same hue as WARNING, very transparent
pub const DARK_ERROR: u32 = 0xc17b75ff; // Medium brightness — red
pub const DARK_ERROR_BG: u32 = 0xc17b7514; // Same hue as ERROR, very transparent
pub const DARK_INFO: u32 = 0x7897caff; // Same brightness as ACCENT
pub const DARK_INFO_BG: u32 = 0x7897ca14; // Same hue as INFO, very transparent

// Version-control / history accents
pub const DARK_VCS_MODIFIED: u32 = 0x7b8eabff; // Medium brightness
pub const DARK_VCS_ADDED: u32 = 0x819b76ff; // Same brightness as SUCCESS
pub const DARK_VCS_DELETED: u32 = 0xa87872ff; // Slightly darker than ERROR
pub const DARK_VCS_RENAMED: u32 = 0x927fa0ff; // Medium brightness — purple
pub const DARK_VCS_CONFLICT: u32 = 0x9d845aff; // Medium brightness — gold
pub const DARK_VCS_IGNORED: u32 = 0x777772ff; // Darker than most other VCS accents

// Diff accents
pub const DARK_DIFF_PLUS: u32 = 0x819b76ff; // Same brightness as SUCCESS
pub const DARK_DIFF_MINUS: u32 = 0xc17b75ff; // Same brightness as ERROR

// Editor chrome
pub const DARK_CURSOR: u32 = 0x7897caff; // Same brightness as ACCENT
pub const DARK_SELECTION: u32 = 0x7897ca2e; // Transparent — subtle accent overlay
pub const DARK_ACTIVE_LINE: u32 = 0x29292780; // Dark — semi-transparent background overlay
pub const DARK_LINE_NUMBER: u32 = 0x686864ff; // Same brightness as EMPTY_TEXT_PRIMARY
pub const DARK_INVISIBLE: u32 = 0x484844ff; // Darker than LINE_NUMBER
pub const DARK_SEARCH_MATCH: u32 = 0x536b8f66; // Dark — semi-transparent blue overlay
pub const DARK_SEARCH_ACTIVE_MATCH: u32 = 0x8a744766; // Brighter than SEARCH_MATCH

// Extra accent hues
pub const DARK_ACCENT_1: u32 = 0x7698ccff; // Medium brightness — blue
pub const DARK_ACCENT_2: u32 = 0xc18a73ff; // Brighter — orange
pub const DARK_ACCENT_3: u32 = 0x80a980ff; // Medium brightness — green
pub const DARK_ACCENT_4: u32 = 0xa486b8ff; // Brighter — purple
pub const DARK_ACCENT_5: u32 = 0xb1a367ff; // Brighter — yellow
pub const DARK_ACCENT_6: u32 = 0x6da1a8ff; // Medium brightness — teal

// Extra cursor hues
pub const DARK_PLAYER_2: u32 = 0x997bb2ff; // Medium brightness — purple
pub const DARK_PLAYER_3: u32 = 0xb99958ff; // Medium brightness — gold
pub const DARK_PLAYER_4: u32 = 0x819b76ff; // Medium brightness — green

// ========================================
// LIGHT — "Paper Light"
// ========================================

// Backgrounds: DARKER → LIGHTER
pub const LIGHT_BG_PRIMARY: u32 = 0xf6f6f3ff; // Brightest — lightest background
pub const LIGHT_BG_SECONDARY: u32 = 0xf0f0edff; // Darkest — darkest background
pub const LIGHT_BG_TERTIARY: u32 = 0xf3f3f0ff; // 2nd brightest — darker than PRIMARY, brighter than DARKER
pub const LIGHT_BG_DARKER: u32 = 0xf1f1eeff; // 2nd darkest — brighter than SECONDARY, darker than TERTIARY

// Borders: DARKER → LIGHTER
pub const LIGHT_BORDER_COLOR: u32 = 0xe5e5e2ff; // 2nd darkest
pub const LIGHT_MENU_BORDER_COLOR: u32 = 0xecece9ff; // Brightest — lightest border
pub const LIGHT_COMPONENT_BORDER_COLOR: u32 = 0xd8d8d4ff; // Darkest — most prominent border
pub const LIGHT_CHROME_BORDER_COLOR: u32 = 0xe3e3e2ff; // 3rd darkest — between component and regular borders

// Menus
pub const LIGHT_MENU_BG: u32 = LIGHT_BG_SECONDARY; // Same brightness as SECONDARY — darkest background
pub const LIGHT_MENU_HOVER_BG: u32 = 0xecece9ff; // Brighter than MENU_BG

// Text: DARKER → LIGHTER
pub const LIGHT_TEXT_PRIMARY: u32 = 0x30302fff; // Darkest — primary text
pub const LIGHT_TEXT_SECONDARY: u32 = 0x858583ff; // Brighter than PRIMARY
pub const LIGHT_EMPTY_TEXT_PRIMARY: u32 = 0xaaa9a6ff; // Brighter than SECONDARY
pub const LIGHT_EMPTY_TEXT_SECONDARY: u32 = 0xb8b8b5ff; // Brightest — least prominent text

pub const LIGHT_SHADOW_COLOR: u32 = 0x00000012; // Darker shadow — more opaque
pub const LIGHT_ITEM_SHADOW_COLOR: u32 = 0x00000008; // Lighter shadow — more transparent

// High-contrast action buttons
pub const LIGHT_BUTTON_BACKGROUND: u32 = 0x30302fff; // Dark — same brightness as TEXT_PRIMARY
pub const LIGHT_BUTTON_TEXT: u32 = 0xf6f6f3ff; // Bright — same brightness as BG_PRIMARY
pub const LIGHT_BUTTON_BORDER_COLOR: u32 = 0x555553ff; // Brighter than button background

// Accents
pub const LIGHT_ACCENT: u32 = 0x5477b8ff; // Darker than ACCENT_BORDER
pub const LIGHT_ACCENT_BORDER: u32 = 0x456ab0ff; // Darker than ACCENT

// Status tones
pub const LIGHT_SUCCESS: u32 = 0x718d67ff; // Medium brightness — green
pub const LIGHT_SUCCESS_BG: u32 = 0x718d6714; // Same hue as SUCCESS, very transparent
pub const LIGHT_WARNING: u32 = 0xb28a43ff; // Medium brightness — gold
pub const LIGHT_WARNING_BG: u32 = 0xb28a4314; // Same hue as WARNING, very transparent
pub const LIGHT_ERROR: u32 = 0xb96f69ff; // Medium brightness — red
pub const LIGHT_ERROR_BG: u32 = 0xb96f6914; // Same hue as ERROR, very transparent
pub const LIGHT_INFO: u32 = 0x5477b8ff; // Same brightness as ACCENT
pub const LIGHT_INFO_BG: u32 = 0x5477b814; // Same hue as INFO, very transparent

// Version-control / history accents
pub const LIGHT_VCS_MODIFIED: u32 = 0x7d91b5ff; // Medium brightness
pub const LIGHT_VCS_ADDED: u32 = 0x718d67ff; // Same brightness as SUCCESS
pub const LIGHT_VCS_DELETED: u32 = 0xad625cff; // Darker than ERROR
pub const LIGHT_VCS_RENAMED: u32 = 0x8a779cff; // Medium brightness — purple
pub const LIGHT_VCS_CONFLICT: u32 = 0x9a742fff; // Medium brightness — gold
pub const LIGHT_VCS_IGNORED: u32 = 0x999996ff; // Medium brightness — gray

// Diff accents
pub const LIGHT_DIFF_PLUS: u32 = 0x718d67ff; // Same brightness as SUCCESS
pub const LIGHT_DIFF_MINUS: u32 = 0xb96f69ff; // Same brightness as ERROR

// Editor chrome
pub const LIGHT_CURSOR: u32 = 0x5477b8ff; // Same brightness as ACCENT
pub const LIGHT_SELECTION: u32 = 0x5477b82e; // Transparent — subtle accent overlay
pub const LIGHT_ACTIVE_LINE: u32 = 0xf1f1ee80; // Same color as BG_DARKER, semi-transparent
pub const LIGHT_LINE_NUMBER: u32 = 0xa2a2a0ff; // Darker than EMPTY_TEXT_PRIMARY
pub const LIGHT_INVISIBLE: u32 = 0xc6c6c3ff; // Brighter than LINE_NUMBER
pub const LIGHT_SEARCH_MATCH: u32 = 0xd9dfeb99; // Light — semi-transparent blue overlay
pub const LIGHT_SEARCH_ACTIVE_MATCH: u32 = 0xe7d9ad99; // Light — semi-transparent gold overlay

// Extra accent hues
pub const LIGHT_ACCENT_1: u32 = 0x6686b8ff; // Medium brightness — blue
pub const LIGHT_ACCENT_2: u32 = 0xb17d68ff; // Medium brightness — orange
pub const LIGHT_ACCENT_3: u32 = 0x729b72ff; // Medium brightness — green
pub const LIGHT_ACCENT_4: u32 = 0x9474a8ff; // Medium brightness — purple
pub const LIGHT_ACCENT_5: u32 = 0xa2945cff; // Medium brightness — yellow
pub const LIGHT_ACCENT_6: u32 = 0x5d9299ff; // Medium brightness — teal

// Extra cursor hues
pub const LIGHT_PLAYER_2: u32 = 0x8a68a6ff; // Medium brightness — purple
pub const LIGHT_PLAYER_3: u32 = 0xb28a43ff; // Medium brightness — gold
pub const LIGHT_PLAYER_4: u32 = 0x718d67ff; // Medium brightness — green
