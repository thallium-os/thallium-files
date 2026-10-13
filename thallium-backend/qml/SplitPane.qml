import QtQuick

// A second folder beside the main pane, for moving files between two places.
// It browses, opens, and gives and takes drops; selection, menus and the
// keyboard stay with the main pane.
Rectangle {
    id: pane

    property var bridge: null
    property bool showHidden: false
    property var timeText: function (seconds) { return "" }
    signal closeRequested
    signal dropped(var drop, string path)

    readonly property int revision: pane.bridge ? pane.bridge.modelRevision : 0
    readonly property var rows: {
        void pane.revision
        const kept = []
        if (!pane.bridge)
            return kept
        for (let i = 0; i < pane.bridge.entryCount(); ++i)
            if (pane.showHidden || !pane.bridge.entryName(i).startsWith("."))
                kept.push(i)
        // Folders first, as in the main pane.
        const folders = kept.filter(function (i) { return pane.bridge.entryIsDirectory(i) })
        return folders.concat(kept.filter(function (i) { return !pane.bridge.entryIsDirectory(i) }))
    }

    color: Theme.canvas

    Rectangle {
        width: 1
        height: parent.height
        color: Theme.line
    }

    Item {
        id: head
        x: 20
        y: 16
        width: parent.width - 40
        height: 40

        IconButton {
            id: upButton
            anchors.left: parent.left
            anchors.verticalCenter: parent.verticalCenter
            icon: "arrow-up"
            iconSize: 16
            hint: "Up"
            onClicked: pane.bridge.up()
        }
        Text {
            anchors.left: upButton.right
            anchors.leftMargin: 8
            anchors.right: closeButton.left
            anchors.rightMargin: 8
            anchors.verticalCenter: parent.verticalCenter
            text: {
                void pane.revision
                return pane.bridge ? pane.bridge.currentPath : ""
            }
            elide: Text.ElideLeft
            color: Theme.inkSoft
            font.family: Theme.family
            font.pixelSize: Theme.text
        }
        IconButton {
            id: closeButton
            anchors.right: parent.right
            anchors.verticalCenter: parent.verticalCenter
            icon: "x"
            iconSize: 15
            hint: "Close split view (F3)"
            onClicked: pane.closeRequested()
        }
    }

    CrossRule {
        id: headRule
        x: 20
        anchors.top: head.bottom
        anchors.topMargin: 12
        width: parent.width - 40
    }

    // Dropping on the pane, not on a folder in it, lands in its folder.
    DropArea {
        anchors.fill: list
        onDropped: function (drop) { pane.dropped(drop, pane.bridge.currentPath) }
    }

    ListView {
        id: list
        anchors.left: parent.left
        anchors.right: parent.right
        anchors.top: headRule.bottom
        anchors.bottom: parent.bottom
        anchors.margins: 14
        clip: true
        model: pane.rows
        boundsBehavior: Flickable.StopAtBounds

        delegate: Item {
            id: row
            required property int modelData
            readonly property int entry: row.modelData
            readonly property string name: {
                void pane.revision
                return pane.bridge.entryName(row.entry)
            }
            readonly property bool folder: {
                void pane.revision
                return pane.bridge.entryIsDirectory(row.entry)
            }

            width: ListView.view.width
            height: Theme.rowHeightDense

            Rectangle {
                anchors.fill: parent
                radius: Theme.radiusSmall
                color: rowMouse.containsMouse ? Theme.surfaceHigh : "transparent"
                border.width: rowDrop.containsDrag ? 1 : 0
                border.color: Theme.outline
            }
            FileTile {
                id: tile
                x: 10
                anchors.verticalCenter: parent.verticalCenter
                width: 18
                height: 18
                name: row.name
                folder: row.folder
            }
            Text {
                anchors.left: tile.right
                anchors.leftMargin: 10
                anchors.right: when.left
                anchors.rightMargin: 12
                anchors.verticalCenter: parent.verticalCenter
                text: row.name
                elide: Text.ElideMiddle
                color: row.folder ? Theme.ink : Theme.inkSoft
                font.family: Theme.family
                font.pixelSize: Theme.text
                font.weight: row.folder ? Font.Medium : Font.Normal
            }
            Text {
                id: when
                anchors.right: parent.right
                anchors.rightMargin: 10
                anchors.verticalCenter: parent.verticalCenter
                text: {
                    void pane.revision
                    return pane.timeText(pane.bridge.entryModified(row.entry))
                }
                color: Theme.inkDim
                font.family: Theme.family
                font.pixelSize: 12
            }

            DropArea {
                id: rowDrop
                anchors.fill: parent
                enabled: row.folder
                onDropped: function (drop) { pane.dropped(drop, pane.bridge.entryPath(row.entry)) }
            }

            // Dragged out, an entry carries its own URI; a plain drop moves it.
            Item {
                id: carrier
                Drag.dragType: Drag.Automatic
                Drag.supportedActions: Qt.CopyAction | Qt.MoveAction | Qt.LinkAction
                Drag.proposedAction: Qt.MoveAction
                Drag.mimeData: ({ "text/uri-list": "file://" + encodeURI(pane.bridge.entryPath(row.entry)) + "\r\n" })
                Drag.onDragFinished: carrier.Drag.active = false
            }

            MouseArea {
                id: rowMouse
                property point origin
                anchors.fill: parent
                hoverEnabled: true
                cursorShape: Qt.PointingHandCursor
                onPressed: function (mouse) { rowMouse.origin = Qt.point(mouse.x, mouse.y) }
                onPositionChanged: function (mouse) {
                    if (rowMouse.pressed && !carrier.Drag.active
                            && Math.abs(mouse.x - rowMouse.origin.x) + Math.abs(mouse.y - rowMouse.origin.y) > 8)
                        carrier.Drag.active = true
                }
                onDoubleClicked: {
                    if (row.folder)
                        pane.bridge.navigateRow(row.entry)
                    else
                        pane.bridge.openEntry(row.entry)
                }
            }
        }
    }

    Text {
        anchors.centerIn: list
        visible: list.count === 0
        text: "Empty folder"
        color: Theme.inkDim
        font.family: Theme.family
        font.pixelSize: Theme.text
    }
}
