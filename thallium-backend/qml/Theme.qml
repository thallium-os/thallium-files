pragma Singleton

import QtQuick

// Every colour, size and font the interface uses.
//
// Colours come from one of two places, as chosen in the Theme sheet:
//  - Thallium: the Everforest variants and accents of the shell's
//    ThalliumPalette, picked in Settings and stored in
//    ~/.config/thallium/theme.conf (main.qml feeds that file to `conf`).
//  - Custom: five colours the user picks (`base`); every other shade is
//    derived from them, as in Knowledge Tree's theme editor.
//
// State is drawn in the primary colour: what is held or focused gets a
// wash and a full outline, never a mark on one side.
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

    // ---- Custom themes ----
    // The five colours a user edits; everything else is derived from them.
    readonly property var editable: [
        { key: "background", label: "Background" },
        { key: "primary", label: "Primary" },
        { key: "secondary", label: "Secondary" },
        { key: "highlight", label: "Highlight" },
        { key: "text", label: "Text" }
    ]
    readonly property var presets: [
        { name: "Monochrome", colors: { background: "#17151b", primary: "#c6c6cd", secondary: "#878689", highlight: "#ececf0", text: "#ececf0" } },
        { name: "Oasis", colors: { background: "#07181b", primary: "#2dd4bf", secondary: "#5eead4", highlight: "#34d399", text: "#ecfeff" } },
        { name: "Forest Focus", colors: { background: "#0a0f0a", primary: "#4ade80", secondary: "#86efac", highlight: "#34d399", text: "#f0fdf4" } },
        { name: "Pastel Mocha", colors: { background: "#f9f3ed", primary: "#b07d62", secondary: "#8a9e82", highlight: "#c9a96e", text: "#3a2d23" } }
    ]

    property bool custom: false
    property var base: theme.presets[0].colors

    // `c` may be a palette string; lighter(…, 1) turns it into a colour.
    function wash(c, alpha) {
        const k = Qt.lighter(c, 1.0)
        return Qt.rgba(k.r, k.g, k.b, alpha)
    }
    // Linear blend: t = 0 gives a, t = 1 gives b.
    function mix(a, b, t) {
        const x = Qt.lighter(a, 1.0)
        const y = Qt.lighter(b, 1.0)
        return Qt.rgba(x.r + (y.r - x.r) * t, x.g + (y.g - x.g) * t, x.b + (y.b - x.b) * t, 1)
    }
    function luminance(c) {
        const k = Qt.lighter(c, 1.0)
        return 0.299 * k.r + 0.587 * k.g + 0.114 * k.b
    }
    function isColor(text) { return /^#[0-9a-fA-F]{6}$/.test(String(text)) }

    // Surfaces step from the background toward the text, so the same
    // recipe works on dark and light backgrounds alike.
    function derive(b) {
        const bg = b.background, ink = b.text
        return {
            dark: theme.luminance(bg) < 0.5,
            accent: b.primary, folder: b.secondary, highlight: b.highlight,
            danger: theme.luminance(bg) < 0.5 ? "#e67e80" : "#d64545",
            canvas: bg, inset: bg,
            surface: theme.mix(bg, ink, 0.05), surfaceHigh: theme.mix(bg, ink, 0.09),
            control: theme.mix(bg, ink, 0.08), controlHigh: theme.mix(bg, ink, 0.15),
            line: theme.mix(bg, ink, 0.08), outlineSoft: theme.mix(bg, ink, 0.18),
            cross: theme.mix(bg, ink, 0.30),
            ink: ink, inkSoft: theme.mix(ink, bg, 0.15),
            inkDim: theme.mix(ink, bg, 0.40), inkFaint: theme.mix(ink, bg, 0.55),
            tile: theme.mix(bg, ink, 0.32), tileInk: ink,
            chart: [b.primary, b.secondary, b.highlight, theme.mix(b.primary, bg, 0.45), theme.mix(ink, bg, 0.55)]
        }
    }
    // The accent, then the palette's other hues, then grey: one colour per
    // chart segment, each far enough from its neighbours to tell apart.
    function chartHues(p, accentKey) {
        // Hues too close to the accent to sit beside it.
        const kin = { green: ["aqua"], aqua: ["green", "blue"], blue: ["aqua"],
                      yellow: ["orange"], orange: ["yellow", "red"], red: ["orange", "purple"],
                      purple: ["red"] }
        const near = [accentKey].concat(kin[accentKey] || [])
        const rest = ["purple", "yellow", "blue", "green", "red", "orange", "aqua"]
                .filter(function (k) { return near.indexOf(k) < 0 })
        return [p[accentKey], p[rest[0]], p[rest[1]], p[rest[2]], p.grey1]
    }
    function fromPalette(p, accentKey, dark) {
        return {
            dark: dark,
            accent: p[accentKey], folder: p.bg5, highlight: p[accentKey],
            danger: p.red,
            canvas: p.base, inset: p.base,
            surface: p.bg0, surfaceHigh: p.bg1,
            control: p.bg2, controlHigh: p.bg3,
            line: p.bg2, outlineSoft: p.bg4, cross: p.bg5,
            ink: p.fg, inkSoft: theme.wash(p.fg, 0.82),
            inkDim: p.grey1, inkFaint: p.grey0,
            tile: p.bg5, tileInk: p.fg,
            chart: theme.chartHues(p, accentKey)
        }
    }
    readonly property var colors: theme.custom ? theme.derive(theme.base)
                                               : theme.fromPalette(theme.pal, theme.accentKey, theme.active.dark)
    readonly property bool dark: theme.colors.dark

    // ---- Accent: active, focused and held things ----
    readonly property color accent: theme.colors.accent
    readonly property color accentWash: theme.wash(theme.accent, 0.14)
    readonly property color highlight: theme.colors.highlight
    readonly property color danger: theme.colors.danger

    // ---- Surfaces, deepest to highest ----
    readonly property color canvas: theme.colors.canvas
    readonly property color surface: theme.colors.surface
    readonly property color surfaceHigh: theme.colors.surfaceHigh
    readonly property color selected: theme.accentWash
    readonly property color control: theme.colors.control
    readonly property color controlHigh: theme.colors.controlHigh
    readonly property color inset: theme.colors.inset

    // ---- Lines ----
    readonly property color line: theme.colors.line
    readonly property color cross: theme.colors.cross
    readonly property color outline: theme.accent
    readonly property color outlineSoft: theme.colors.outlineSoft

    // ---- Ink ----
    readonly property color ink: theme.colors.ink
    readonly property color inkSoft: theme.colors.inkSoft
    readonly property color inkDim: theme.colors.inkDim
    readonly property color inkFaint: theme.colors.inkFaint

    // ---- File tiles: folders in the secondary colour ----
    readonly property color tile: theme.colors.tile
    readonly property color folderTile: theme.colors.folder
    readonly property color tileInk: theme.colors.tileInk

    // ---- Charts: one colour per segment ----
    readonly property var chart: theme.colors.chart

    // ---- Shape ----
    readonly property int radius: 8
    readonly property int radiusSmall: 6
    readonly property int gap: 12
    readonly property int railWidth: 62
    readonly property int railOpenWidth: 212
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
