pragma Singleton

import QtQuick

// Every colour, size and font the interface uses, in Thallium's terms.
//
// The palette is Thallium's: the Everforest variants and accents of the
// shell's ThalliumPalette, chosen in Settings and stored in
// ~/.config/thallium/theme.conf. main.qml feeds that file's text to `conf`,
// so a pick in Settings repaints this window too. Without the file the
// defaults stand: Deep Space with the green accent.
//
// State is drawn in the accent: what is held or focused gets an accent wash
// and a full accent outline, never a mark on one side.
QtObject {
    id: theme

    // ---- Palette: kept in step with ThalliumPalette.qml ----
    readonly property var darkInk: ({
        fg: "#d3c6aa",
        red: "#e67e80", orange: "#e69875", yellow: "#dbbc7f", green: "#a7c080",
        aqua: "#83c092", blue: "#7fbbb3", purple: "#d699b6",
        grey0: "#7a8478", grey1: "#859289", grey2: "#9da9a0"
    })
    readonly property var lightInk: ({
        fg: "#5c6a72",
        red: "#f85552", orange: "#f57d26", yellow: "#dfa000", green: "#8da101",
        aqua: "#35a77c", blue: "#3a94c5", purple: "#df69ba",
        grey0: "#a6b0a0", grey1: "#939f91", grey2: "#829181"
    })

    function variant(id, dark, bg, base) {
        const p = {}
        const ink = dark ? theme.darkInk : theme.lightInk
        for (const a in ink)
            p[a] = ink[a]
        for (const b in bg)
            p[b] = bg[b]
        p.base = base || bg.bgDim
        return { id: id, dark: dark, pal: p }
    }

    readonly property var variants: [
        variant("thallium-deep-space", true, {
            bgDim: "#0d0d0f", bg0: "#1e2326", bg1: "#232a2e", bg2: "#232a2e",
            bg3: "#2e383c", bg4: "#3a464c", bg5: "#4f5b58"
        }, "#0d0d0f"),
        variant("everforest-dark-hard", true, {
            bgDim: "#1e2326", bg0: "#272e33", bg1: "#2e383c", bg2: "#374145",
            bg3: "#414b50", bg4: "#495156", bg5: "#4f5b58"
        }),
        variant("everforest-dark-medium", true, {
            bgDim: "#232a2e", bg0: "#2d353b", bg1: "#343f44", bg2: "#3d484d",
            bg3: "#475258", bg4: "#4f585e", bg5: "#56635f"
        }),
        variant("everforest-dark-soft", true, {
            bgDim: "#293136", bg0: "#333c43", bg1: "#3a464c", bg2: "#434f55",
            bg3: "#4d5960", bg4: "#555f66", bg5: "#5d6b66"
        }),
        variant("everforest-light-hard", false, {
            bgDim: "#f2efdf", bg0: "#fffbef", bg1: "#f8f5e4", bg2: "#f2efdf",
            bg3: "#edeada", bg4: "#e8e5d5", bg5: "#bec5b2"
        }),
        variant("everforest-light-medium", false, {
            bgDim: "#efebd4", bg0: "#fdf6e3", bg1: "#f4f0d9", bg2: "#efebd4",
            bg3: "#e6e2cc", bg4: "#e0dcc7", bg5: "#bdc3af"
        }),
        variant("everforest-light-soft", false, {
            bgDim: "#e5dfc5", bg0: "#f3ead3", bg1: "#eae4ca", bg2: "#e5dfc5",
            bg3: "#ddd8be", bg4: "#d8d3ba", bg5: "#b9c0ab"
        })
    ]
    readonly property var accentKeys: ["red", "orange", "yellow", "green", "aqua", "blue", "purple"]

    // The text of theme.conf: `variant = <id>` and `accent = <key>` lines.
    property string conf: ""
    readonly property string variantId: theme.confValue("variant")
    readonly property string accentKey: {
        const key = theme.confValue("accent")
        return theme.accentKeys.indexOf(key) >= 0 ? key : "green"
    }
    function confValue(name) {
        const lines = theme.conf.split("\n")
        for (let i = 0; i < lines.length; ++i) {
            const m = lines[i].match(/^\s*(\w+)\s*=\s*(\S+)/)
            if (m && m[1] === name)
                return m[2]
        }
        return ""
    }
    readonly property var active: {
        for (let i = 0; i < theme.variants.length; ++i)
            if (theme.variants[i].id === theme.variantId)
                return theme.variants[i]
        return theme.variants[0]
    }
    readonly property var pal: theme.active.pal
    readonly property bool dark: theme.active.dark

    // `c` may be a palette string; lighter(…, 1) turns it into a colour.
    function wash(c, alpha) {
        const k = Qt.lighter(c, 1.0)
        return Qt.rgba(k.r, k.g, k.b, alpha)
    }

    // ---- Accent: active, focused and held things ----
    readonly property color accent: theme.pal[theme.accentKey]
    readonly property color accentWash: theme.wash(theme.accent, 0.14)
    readonly property color danger: theme.pal.red

    // ---- Surfaces, deepest to highest ----
    readonly property color canvas: theme.pal.base
    readonly property color surface: theme.pal.bg0
    readonly property color surfaceHigh: theme.pal.bg1
    readonly property color selected: theme.accentWash
    readonly property color control: theme.pal.bg2
    readonly property color controlHigh: theme.pal.bg3
    readonly property color inset: theme.pal.base

    // ---- Lines ----
    readonly property color line: theme.pal.bg2
    readonly property color cross: theme.pal.bg5
    readonly property color outline: theme.accent
    readonly property color outlineSoft: theme.pal.bg4

    // ---- Ink ----
    readonly property color ink: theme.pal.fg
    readonly property color inkSoft: theme.wash(theme.pal.fg, 0.82)
    readonly property color inkDim: theme.pal.grey1
    readonly property color inkFaint: theme.pal.grey0

    // ---- File-type tiles ----
    readonly property color tile: theme.pal.bg5
    readonly property color tileInk: theme.pal.fg

    // ---- Shape ----
    readonly property int radius: 8
    readonly property int radiusSmall: 6
    readonly property int gap: 12
    readonly property int railWidth: 84
    readonly property int placesWidth: 212
    readonly property int previewWidth: 320
    readonly property int controlHeight: 34
    readonly property int rowHeight: 56
    readonly property int rowHeightDense: 34

    // ---- Type: one voice per role, never mixed ----
    // Human: names and readable text. Machine: labels, sizes, times, paths.
    readonly property string family: "Albert Sans"
    readonly property string mono: "JetBrains Mono"
    readonly property real letterSpace: 1.2
    readonly property int textSmall: 11
    readonly property int text: 13
    readonly property int textLarge: 15
    readonly property int title: 17
}
