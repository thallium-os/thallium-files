import QtQuick

// The disk as a 270° dial: 48 radial ticks (every sixth one longer) lit up
// to the used share, a thin outer scale marked 0 · 50 · 100 with a pointer at
// the value, and a dotted inner ring around the readout. Below it, a
// sheared bar and the USED | FREE | TOTAL figures.
Item {
    id: gauge
    property real used: 0
    property real total: 0
    // Formats a byte count; the window passes its own so every size in the
    // app reads the same.
    property var format: function (bytes) { return String(bytes) }

    readonly property real share: gauge.total > 0 ? Math.max(0, Math.min(1, gauge.used / gauge.total)) : 0
    readonly property real free: Math.max(0, gauge.total - gauge.used)

    // "18.1 / 53.5 GB": the unit once when both sides share it.
    function pair(left, right) {
        const a = gauge.format(left), b = gauge.format(right)
        const cut = a.lastIndexOf(" ")
        if (cut > 0 && a.slice(cut) === b.slice(b.lastIndexOf(" ")))
            return a.slice(0, cut) + " / " + b
        return a + " / " + b
    }

    implicitHeight: panel.height

    ChamferPanel {
        id: panel
        width: parent.width
        height: readout.y + readout.height + 1
    }
    CornerBrackets { anchors.fill: panel; anchors.margins: 7 }

    Item {
        id: dialBox
        anchors.horizontalCenter: parent.horizontalCenter
        y: 18
        width: 190
        height: 178

        Canvas {
            id: dial
            anchors.fill: parent
            antialiasing: true
            renderStrategy: Canvas.Cooperative
            readonly property real share: gauge.share
            onShareChanged: requestPaint()
            Component.onCompleted: requestPaint()

            onPaint: {
                const ctx = getContext("2d")
                ctx.reset()
                // The reference dial is drawn in a 180 × 168 box.
                const k = width / 180
                ctx.scale(k, k)
                const cx = 90, cy = 90
                const start = -135, sweep = 270
                const at = start + sweep * dial.share
                function point(angle, radius) {
                    const a = angle * Math.PI / 180
                    return [cx + radius * Math.sin(a), cy - radius * Math.cos(a)]
                }
                function arc(radius, from, to) {
                    // Canvas angles run from three o'clock; ours from twelve.
                    ctx.beginPath()
                    ctx.arc(cx, cy, radius, (from - 90) * Math.PI / 180, (to - 90) * Math.PI / 180, false)
                }

                // Outer scale, then its lit stretch.
                ctx.lineWidth = 1
                ctx.strokeStyle = Theme.cLine
                arc(76, start, start + sweep)
                ctx.stroke()
                if (dial.share > 0) {
                    ctx.lineWidth = 1.5
                    ctx.strokeStyle = Theme.cGreen
                    arc(76, start, at)
                    ctx.stroke()
                }

                // Dotted ring and the readout's well.
                ctx.lineWidth = 1
                ctx.strokeStyle = Theme.cLine
                for (let d = 0; d < 360; d += 4) {
                    arc(44, d, d + 1)
                    ctx.stroke()
                }
                ctx.beginPath()
                ctx.arc(cx, cy, 38, 0, Math.PI * 2)
                ctx.fillStyle = Theme.cBase
                ctx.fill()
                ctx.strokeStyle = Theme.cDead
                ctx.stroke()

                // Ticks: dead first, then the lit ones with their glow.
                const ticks = 48
                const lit = Math.round(dial.share * ticks)
                ctx.lineWidth = 2.4
                for (let pass = 0; pass < 2; ++pass) {
                    ctx.strokeStyle = pass === 0 ? Theme.cDead : Theme.cGreen
                    ctx.shadowColor = pass === 0 ? "transparent" : Qt.rgba(Theme.cGreen.r, Theme.cGreen.g, Theme.cGreen.b, 0.75)
                    ctx.shadowBlur = pass === 0 ? 0 : 3 * k
                    for (let i = 0; i < ticks; ++i) {
                        if ((i < lit) !== (pass === 1))
                            continue
                        const angle = start + sweep * (i + 0.5) / ticks
                        const major = i % 6 === 0
                        const a = point(angle, major ? 52 : 56)
                        const b = point(angle, major ? 68 : 66)
                        ctx.beginPath()
                        ctx.moveTo(a[0], a[1])
                        ctx.lineTo(b[0], b[1])
                        ctx.stroke()
                    }
                }
                ctx.shadowBlur = 0
                ctx.shadowColor = "transparent"

                // The pointer sits just outside the scale, aimed inward.
                const tip = point(at, 77)
                const left = point(at - 4, 84)
                const right = point(at + 4, 84)
                ctx.beginPath()
                ctx.moveTo(tip[0], tip[1])
                ctx.lineTo(left[0], left[1])
                ctx.lineTo(right[0], right[1])
                ctx.closePath()
                ctx.fillStyle = Theme.cGreen
                ctx.fill()

                // Scale labels.
                ctx.fillStyle = Theme.cMuted
                ctx.font = "500 8px '" + Theme.fontMono + "'"
                ctx.textAlign = "center"
                ctx.textBaseline = "middle"
                const zero = point(start, 88), half = point(0, 86), full = point(start + sweep, 88)
                ctx.fillText("0", zero[0], zero[1])
                ctx.fillText("50", half[0], half[1])
                ctx.fillText("100", full[0], full[1])
            }
        }

        Column {
            anchors.horizontalCenter: parent.horizontalCenter
            y: 66
            spacing: 4
            Text {
                anchors.horizontalCenter: parent.horizontalCenter
                text: "USED"
                color: Theme.cMuted
                font.family: Theme.fontMono
                font.pixelSize: 8
                font.letterSpacing: 2.5
            }
            Row {
                anchors.horizontalCenter: parent.horizontalCenter
                spacing: 2
                Text {
                    id: figure
                    text: Math.round(gauge.share * 100)
                    color: Theme.cFg
                    font.family: Theme.fontBrand
                    font.pixelSize: 30
                    font.weight: Font.ExtraBold
                }
                Text {
                    anchors.baseline: figure.baseline
                    text: "%"
                    color: Theme.cGreen
                    font.family: Theme.fontBrand
                    font.pixelSize: 13
                    font.weight: Font.DemiBold
                }
            }
            Text {
                anchors.horizontalCenter: parent.horizontalCenter
                text: gauge.pair(gauge.used, gauge.total)
                color: Theme.cMuted
                font.family: Theme.fontMono
                font.pixelSize: 9
                font.letterSpacing: 1
            }
        }
    }

    SegmentBar {
        id: capBar
        anchors.top: dialBox.bottom
        anchors.topMargin: 4
        anchors.left: parent.left
        anchors.right: parent.right
        anchors.leftMargin: 19
        anchors.rightMargin: 19
        height: 5
        count: 32
        share: gauge.share
    }

    Rectangle {
        id: readout
        anchors.top: capBar.bottom
        anchors.topMargin: 14
        anchors.left: parent.left
        anchors.right: parent.right
        anchors.leftMargin: 1
        anchors.rightMargin: 1
        height: 58
        color: Qt.rgba(Theme.cBase.r, Theme.cBase.g, Theme.cBase.b, 0.35)
        Rectangle { width: parent.width; height: 1; color: Theme.cDead }

        Row {
            anchors.fill: parent
            anchors.leftMargin: 15
            anchors.rightMargin: 15
            anchors.topMargin: 12
            spacing: 10
            Repeater {
                model: [
                    { label: "■ USED", value: gauge.format(gauge.used), lit: true },
                    { label: "□ FREE", value: gauge.format(gauge.free), lit: false },
                    { label: "TOTAL", value: gauge.format(gauge.total), lit: false }
                ]
                delegate: Row {
                    required property var modelData
                    required property int index
                    spacing: 10
                    Rectangle {
                        visible: index > 0
                        width: 1
                        height: 32
                        color: Theme.cDead
                    }
                    Column {
                        width: (readout.width - 30 - 20 - 22) / 3
                        spacing: 4
                        Text {
                            text: modelData.label
                            color: modelData.lit ? Theme.cGreen : Theme.cMuted
                            font.family: Theme.fontMono
                            font.pixelSize: 8
                            font.letterSpacing: 2
                        }
                        Text {
                            width: parent.width
                            elide: Text.ElideRight
                            text: modelData.value
                            color: Theme.cFg
                            font.family: Theme.fontBrand
                            font.pixelSize: 13
                            font.weight: Font.DemiBold
                        }
                    }
                }
            }
        }
    }
}
