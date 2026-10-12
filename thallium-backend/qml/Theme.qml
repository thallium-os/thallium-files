pragma Singleton

import QtQuick

// Every colour, size and font the interface uses. The palette is strictly
// monochrome: state is shown by lightness and outline, never by hue.
QtObject {
    // ---- Surfaces, darkest to lightest ----
    readonly property color canvas: "#17151B"
    readonly property color surface: "#20212A"
    readonly property color surfaceHigh: "#272832"
    readonly property color selected: "#30313A"
    readonly property color control: "#2B2C35"
    readonly property color controlHigh: "#3A3B44"
    readonly property color inset: "#14131A"

    // ---- Lines ----
    readonly property color line: "#2A2832"
    readonly property color cross: "#524E56"
    readonly property color outline: "#55565F"
    readonly property color outlineSoft: "#383943"

    // ---- Ink ----
    readonly property color ink: "#ECECF0"
    readonly property color inkSoft: "#C6C6CD"
    readonly property color inkDim: "#8E8E97"
    readonly property color inkFaint: "#5C5D66"

    // ---- File-type tiles ----
    readonly property color tile: "#878689"
    readonly property color tileInk: "#25252B"

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

    // ---- Type ----
    readonly property string family: regular.name.length > 0 ? regular.name : "sans-serif"
    readonly property string mono: "JetBrains Mono, DejaVu Sans Mono, monospace"
    readonly property int textSmall: 11
    readonly property int text: 13
    readonly property int textLarge: 15
    readonly property int title: 17

    readonly property FontLoader light: FontLoader { source: "fonts/ChakraPetch-Light.ttf" }
    readonly property FontLoader regular: FontLoader { source: "fonts/ChakraPetch-Regular.ttf" }
    readonly property FontLoader medium: FontLoader { source: "fonts/ChakraPetch-Medium.ttf" }
    readonly property FontLoader semibold: FontLoader { source: "fonts/ChakraPetch-SemiBold.ttf" }
}
