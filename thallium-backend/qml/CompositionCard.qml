import QtQuick

// What a folder is made of, three ways: ORBIT (a ring per type, as long as
// its share), BARS (a sheared bar per type) and ELEMENTS (a tile per type,
// like a periodic table). The type of the selected entry stands out in all
// three.
//
// `kinds` holds one { tag, label, short, sym, count, color } per type.
Item {
    id: card
    property var kinds: []
    property string focusTag: ""
    property string mode: "orbit"

    readonly property int total: {
        let sum = 0
        for (let i = 0; i < card.kinds.length; ++i)
            sum += card.kinds[i].count
        return sum
    }
    function shareOf(kind) { return card.total > 0 ? kind.count / card.total : 0 }
    function percent(kind) { return (card.shareOf(kind) * 100).toFixed(1) + "%" }
    function pad2(n) { return n < 10 ? "0" + n : String(n) }
    function on(kind) { return card.focusTag === "" || kind.tag === card.focusTag }

    implicitHeight: tabs.height + body.height + 2

    ChamferPanel { anchors.fill: parent }

    // ---- Tabs ----
    Row {
        id: tabs
        x: 1
        y: 1
        width: parent.width - 2
        height: 34
        Repeater {
            model: [["orbit", "ORBIT"], ["bars", "BARS"], ["cells", "ELEMENTS"]]
            delegate: Rectangle {
                required property var modelData
                required property int index
                readonly property bool active: card.mode === modelData[0]
                width: tabs.width / 3
                height: tabs.height
                color: active ? Theme.cWash : "transparent"
                Text {
                    anchors.centerIn: parent
                    text: modelData[1]
                    color: active || tabMouse.containsMouse ? Theme.cGreen : Theme.cMuted
                    font.family: Theme.fontMono
                    font.pixelSize: 9
                    font.bold: true
                    font.letterSpacing: 2.5
                }
                Rectangle {
                    visible: index < 2
                    anchors.right: parent.right
                    width: 1
                    height: parent.height
                    color: Theme.cDead
                }
                Rectangle {
                    anchors.bottom: parent.bottom
                    width: parent.width
                    height: 2
                    color: Theme.cGreen
                    visible: active
                }
                MouseArea {
                    id: tabMouse
                    anchors.fill: parent
                    hoverEnabled: true
                    cursorShape: Qt.PointingHandCursor
                    onClicked: card.mode = modelData[0]
                }
            }
        }
    }
    Rectangle {
        anchors.top: tabs.bottom
        x: 1
        width: parent.width - 2
        height: 1
        color: Theme.cDead
    }

    Item {
        id: body
        anchors.top: tabs.bottom
        anchors.topMargin: 1
        x: 1
        width: parent.width - 2
        height: card.mode === "orbit" ? orbit.height : card.mode === "bars" ? bars.height : cells.height

        // ---- ORBIT ----
        Item {
            id: orbit
            visible: card.mode === "orbit"
            width: parent.width
            height: 140

            Canvas {
                id: rings
                x: 16
                anchors.verticalCenter: parent.verticalCenter
                width: 108
                height: 108
                antialiasing: true
                readonly property var kinds: card.kinds
                readonly property string focusTag: card.focusTag
                onKindsChanged: requestPaint()
                onFocusTagChanged: requestPaint()
                Component.onCompleted: requestPaint()

                onPaint: {
                    const ctx = getContext("2d")
                    ctx.reset()
                    const k = width / 140
                    ctx.scale(k, k)
                    const c = 70
                    // Crosshair notches.
                    ctx.strokeStyle = Theme.cDead
                    ctx.lineWidth = 1
                    ctx.beginPath()
                    ctx.moveTo(70, 0); ctx.lineTo(70, 8)
                    ctx.moveTo(70, 132); ctx.lineTo(70, 140)
                    ctx.moveTo(0, 70); ctx.lineTo(8, 70)
                    ctx.moveTo(132, 70); ctx.lineTo(140, 70)
                    ctx.stroke()
                    for (let i = 0; i < card.kinds.length; ++i) {
                        const kind = card.kinds[i]
                        const r = 62 - i * 12
                        ctx.globalAlpha = 1
                        ctx.lineWidth = 5
                        ctx.strokeStyle = Theme.cDead
                        ctx.beginPath()
                        ctx.arc(c, c, r, 0, Math.PI * 2)
                        ctx.stroke()
                        const share = card.shareOf(kind)
                        if (share <= 0)
                            continue
                        ctx.globalAlpha = card.on(kind) ? 1 : 0.45
                        const from = -Math.PI / 2
                        const to = from + share * Math.PI * 2
                        ctx.strokeStyle = kind.color
                        ctx.shadowColor = kind.color
                        ctx.shadowBlur = 3
                        ctx.beginPath()
                        ctx.arc(c, c, r, from, to, false)
                        ctx.stroke()
                        ctx.shadowBlur = 0
                        // The end node.
                        ctx.beginPath()
                        ctx.arc(c + r * Math.cos(to), c + r * Math.sin(to), 4.5, 0, Math.PI * 2)
                        ctx.fillStyle = Theme.cBase
                        ctx.fill()
                        ctx.lineWidth = 2
                        ctx.stroke()
                    }
                    ctx.globalAlpha = 1
                }
            }
            Column {
                anchors.centerIn: rings
                spacing: 1
                Text {
                    anchors.horizontalCenter: parent.horizontalCenter
                    text: card.pad2(card.total)
                    color: Theme.cFg
                    font.family: Theme.fontBrand
                    font.pixelSize: 16
                    font.weight: Font.ExtraBold
                }
                Text {
                    anchors.horizontalCenter: parent.horizontalCenter
                    text: card.total === 1 ? "ITEM" : "ITEMS"
                    color: Theme.cMuted
                    font.family: Theme.fontMono
                    font.pixelSize: 7
                    font.letterSpacing: 1.5
                }
            }
            Column {
                anchors.left: rings.right
                anchors.leftMargin: 14
                anchors.right: parent.right
                anchors.rightMargin: 10
                anchors.verticalCenter: parent.verticalCenter
                spacing: 6
                Repeater {
                    model: card.kinds
                    delegate: Rectangle {
                        required property var modelData
                        readonly property bool lit: modelData.tag === card.focusTag
                        width: parent.width
                        height: 24
                        color: lit ? Theme.cWash : "transparent"
                        Rectangle {
                            x: 6
                            anchors.verticalCenter: parent.verticalCenter
                            width: 8
                            height: 8
                            color: modelData.color
                        }
                        Text {
                            x: 22
                            anchors.verticalCenter: parent.verticalCenter
                            width: parent.width - 22 - pct.width - 12
                            elide: Text.ElideRight
                            text: modelData.short
                            color: lit ? Theme.cFg : Theme.cMuted
                            font.family: Theme.fontHuman
                            font.pixelSize: 12
                            font.weight: Font.DemiBold
                        }
                        Text {
                            id: pct
                            anchors.right: parent.right
                            anchors.rightMargin: 6
                            anchors.verticalCenter: parent.verticalCenter
                            text: card.percent(modelData)
                            color: Theme.cMuted
                            font.family: Theme.fontMono
                            font.pixelSize: 11
                        }
                    }
                }
            }
        }

        // ---- BARS ----
        Column {
            id: bars
            visible: card.mode === "bars"
            width: parent.width
            topPadding: 10
            bottomPadding: 12
            spacing: 1
            Repeater {
                model: card.kinds
                delegate: Rectangle {
                    required property var modelData
                    readonly property bool lit: modelData.tag === card.focusTag
                    x: 8
                    width: bars.width - 16
                    height: 38
                    color: lit ? Theme.cWash : "transparent"
                    Rectangle {
                        x: 8
                        y: 9
                        width: 7
                        height: 7
                        color: modelData.color
                    }
                    Text {
                        x: 23
                        y: 5
                        text: modelData.label.toUpperCase()
                        color: lit ? Theme.cFg : Theme.cMuted
                        font.family: Theme.fontMono
                        font.pixelSize: 10
                        font.bold: true
                        font.letterSpacing: 2.5
                    }
                    Text {
                        anchors.right: barPct.left
                        anchors.rightMargin: 8
                        y: 5
                        text: card.pad2(modelData.count) + " ITEMS"
                        color: Theme.cMuted
                        font.family: Theme.fontMono
                        font.pixelSize: 10
                    }
                    Text {
                        id: barPct
                        anchors.right: parent.right
                        anchors.rightMargin: 8
                        y: 4
                        width: 58
                        horizontalAlignment: Text.AlignRight
                        text: card.percent(modelData)
                        color: modelData.color
                        font.family: Theme.fontBrand
                        font.pixelSize: 12
                        font.weight: Font.DemiBold
                    }
                    SegmentBar {
                        x: 11
                        y: 23
                        width: parent.width - 22
                        height: 9
                        count: 20
                        lit: modelData.count > 0 ? Math.max(1, Math.round(card.shareOf(modelData) * 20)) : 0
                        color: modelData.color
                    }
                }
            }
        }

        // ---- ELEMENTS ----
        Grid {
            id: cells
            visible: card.mode === "cells"
            x: 14
            y: 14
            width: parent.width - 28
            height: implicitHeight + 28
            columns: 2
            spacing: 8
            Repeater {
                model: card.kinds
                delegate: Rectangle {
                    required property var modelData
                    readonly property bool lit: modelData.tag === card.focusTag
                    width: (cells.width - 8) / 2
                    height: 84
                    color: lit ? Qt.rgba(modelData.color.r, modelData.color.g, modelData.color.b, 0.08) : Theme.cBar
                    border.width: 1
                    border.color: lit ? modelData.color : Theme.cLine
                    Text {
                        x: 10
                        y: 8
                        text: card.pad2(modelData.count)
                        color: Theme.cMuted
                        font.family: Theme.fontMono
                        font.pixelSize: 9
                    }
                    Text {
                        anchors.right: parent.right
                        anchors.rightMargin: 10
                        y: 8
                        text: card.percent(modelData)
                        color: Theme.cMuted
                        font.family: Theme.fontMono
                        font.pixelSize: 9
                    }
                    Text {
                        x: 10
                        y: 22
                        text: modelData.sym
                        color: modelData.color
                        font.family: Theme.fontBrand
                        font.pixelSize: 26
                        font.weight: Font.ExtraBold
                    }
                    Text {
                        x: 10
                        y: 56
                        text: modelData.label.toUpperCase()
                        color: lit ? Theme.cFg : Theme.cMuted
                        font.family: Theme.fontMono
                        font.pixelSize: 8
                        font.letterSpacing: 2
                    }
                    Rectangle {
                        x: 10
                        y: 72
                        width: parent.width - 20
                        height: 2
                        color: Theme.cDead
                        Rectangle {
                            width: parent.width * card.shareOf(modelData)
                            height: 2
                            color: modelData.color
                        }
                    }
                }
            }
        }
    }
}
