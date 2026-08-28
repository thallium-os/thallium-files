pragma ComponentBehavior: Bound

import QtQuick
import QtQuick.Controls
import QtQuick.Layouts
import org.thallium.FileManager 1.0

ApplicationWindow {
    id: root

    readonly property color accent: "#6ad2df"
    readonly property color backgroundColor: "#181818"
    readonly property color panelColor: "#232323"
    readonly property color raisedColor: "#2c2c2c"
    readonly property color hoverColor: "#353535"
    readonly property color textColor: "#e8e8e8"
    readonly property color mutedColor: "#ababab"
    readonly property color selectedTextColor: "#102326"
    property alias backend: backendObject
    readonly property int entryCount: backendObject.entryCount() + backendObject.modelRevision * 0
    readonly property int selectedCount: backendObject.selectionCount() + backendObject.modelRevision * 0
    readonly property int taskCount: backendObject.taskCount() + backendObject.modelRevision * 0
    property bool editingLocation: false

    width: 1120
    height: 720
    minimumWidth: 760
    minimumHeight: 480
    visible: true
    color: backgroundColor
    title: folderTitle(backendObject.currentPath) + " — Thallium Files"

    function folderTitle(path) {
        if (!path || path === "/")
            return "Filesystem"
        const clean = path.endsWith("/") ? path.slice(0, -1) : path
        const parts = clean.split("/")
        return parts[parts.length - 1] || "Home"
    }

    function homeLocation(folder) {
        return folder.length === 0 ? backendObject.homePath : backendObject.homePath + "/" + folder
    }

    function modifiedText(seconds) {
        if (seconds <= 0)
            return "—"
        const value = new Date(seconds * 1000)
        const now = new Date()
        const sameDay = value.getFullYear() === now.getFullYear()
            && value.getMonth() === now.getMonth()
            && value.getDate() === now.getDate()
        if (sameDay)
            return "Today, " + value.toLocaleTimeString(Qt.locale(), "h:mm AP")
        return value.toLocaleDateString(Qt.locale(), "MMM d, yyyy")
    }

    BackendBridge { id: backendObject }

    Timer {
        interval: 100
        repeat: true
        running: true
        onTriggered: backendObject.poll()
    }

    background: Rectangle { color: root.backgroundColor }

    ColumnLayout {
        anchors.fill: parent
        spacing: 0

        Rectangle {
            Layout.fillWidth: true
            Layout.preferredHeight: 50
            color: root.backgroundColor

            RowLayout {
                anchors.fill: parent
                anchors.leftMargin: 14
                anchors.rightMargin: 12
                spacing: 4

                Label {
                    text: "▣"
                    color: root.accent
                    font.pixelSize: 18
                    Layout.rightMargin: 14
                }

                Repeater {
                    model: ["File", "Edit", "View", "Sort"]
                    delegate: ToolButton {
                        id: menuButton
                        required property string modelData
                        text: modelData
                        font.pixelSize: 14
                        palette.buttonText: root.mutedColor
                        background: Rectangle {
                            radius: 6
                            color: menuButton.hovered ? root.raisedColor : "transparent"
                        }
                    }
                }

                Item { Layout.fillWidth: true }

                Label {
                    text: root.selectedCount > 0 ? root.selectedCount + " selected" : ""
                    color: root.mutedColor
                    font.pixelSize: 13
                    Layout.rightMargin: 10
                }

                ToolButton {
                    id: refreshButton
                    text: "↻"
                    font.pixelSize: 19
                    palette.buttonText: root.accent
                    ToolTip.visible: hovered
                    ToolTip.text: "Refresh"
                    onClicked: root.backend.refresh()
                    background: Rectangle {
                        radius: 7
                        color: refreshButton.hovered ? root.raisedColor : "transparent"
                    }
                }
            }

            Rectangle {
                anchors.bottom: parent.bottom
                width: parent.width
                height: 1
                color: "#303030"
            }
        }

        RowLayout {
            Layout.fillWidth: true
            Layout.fillHeight: true
            spacing: 0

            Rectangle {
                Layout.preferredWidth: 245
                Layout.fillHeight: true
                color: root.panelColor

                ColumnLayout {
                    anchors.fill: parent
                    anchors.topMargin: 12
                    anchors.bottomMargin: 10
                    anchors.leftMargin: 10
                    anchors.rightMargin: 10
                    spacing: 2

                    Label {
                        text: "PLACES"
                        color: "#777777"
                        font.pixelSize: 10
                        font.weight: Font.DemiBold
                        leftPadding: 12
                        topPadding: 5
                        bottomPadding: 5
                    }

                    Repeater {
                        model: [
                            { label: "Home", icon: "⌂", folder: "" },
                            { label: "Documents", icon: "▤", folder: "Documents" },
                            { label: "Downloads", icon: "↓", folder: "Downloads" },
                            { label: "Music", icon: "♪", folder: "Music" },
                            { label: "Pictures", icon: "▧", folder: "Pictures" },
                            { label: "Videos", icon: "▶", folder: "Videos" }
                        ]

                        delegate: Rectangle {
                            id: placeDelegate
                            required property var modelData
                            readonly property string targetPath: root.homeLocation(modelData.folder)
                            readonly property bool active: root.backend.currentPath === targetPath

                            Layout.fillWidth: true
                            Layout.preferredHeight: 38
                            radius: 9
                            color: active ? root.hoverColor : (placeMouse.containsMouse ? "#2e2e2e" : "transparent")

                            RowLayout {
                                anchors.fill: parent
                                anchors.leftMargin: 12
                                anchors.rightMargin: 10
                                spacing: 11

                                Label {
                                    text: placeDelegate.modelData.icon
                                    color: placeDelegate.active ? root.accent : "#c9c9c9"
                                    font.pixelSize: 17
                                    Layout.preferredWidth: 18
                                    horizontalAlignment: Text.AlignHCenter
                                }
                                Label {
                                    text: placeDelegate.modelData.label
                                    color: placeDelegate.active ? root.accent : root.textColor
                                    font.pixelSize: 14
                                    Layout.fillWidth: true
                                }
                            }

                            MouseArea {
                                id: placeMouse
                                anchors.fill: parent
                                hoverEnabled: true
                                cursorShape: Qt.PointingHandCursor
                                onClicked: root.backend.navigate(placeDelegate.targetPath)
                            }
                        }
                    }

                    Rectangle {
                        Layout.fillWidth: true
                        Layout.preferredHeight: 1
                        Layout.topMargin: 10
                        Layout.bottomMargin: 10
                        color: "#3a3a3a"
                    }

                    Label {
                        text: "DEVICES"
                        color: "#777777"
                        font.pixelSize: 10
                        font.weight: Font.DemiBold
                        leftPadding: 12
                        bottomPadding: 5
                    }

                    Rectangle {
                        Layout.fillWidth: true
                        Layout.preferredHeight: 38
                        radius: 9
                        color: filesystemMouse.containsMouse ? "#2e2e2e" : "transparent"

                        RowLayout {
                            anchors.fill: parent
                            anchors.leftMargin: 12
                            anchors.rightMargin: 10
                            spacing: 11
                            Label { text: "◉"; color: "#c9c9c9"; font.pixelSize: 15; Layout.preferredWidth: 18 }
                            Label { text: "Filesystem"; color: root.textColor; font.pixelSize: 14; Layout.fillWidth: true }
                        }

                        MouseArea {
                            id: filesystemMouse
                            anchors.fill: parent
                            hoverEnabled: true
                            cursorShape: Qt.PointingHandCursor
                            onClicked: root.backend.navigate("/")
                        }
                    }

                    Item { Layout.fillHeight: true }

                    Label {
                        text: "THALLIUM 81"
                        color: "#626262"
                        font.pixelSize: 10
                        font.letterSpacing: 2
                        Layout.alignment: Qt.AlignHCenter
                        bottomPadding: 4
                    }
                }

                Rectangle {
                    anchors.right: parent.right
                    height: parent.height
                    width: 1
                    color: "#303030"
                }
            }

            Rectangle {
                Layout.fillWidth: true
                Layout.fillHeight: true
                color: root.backgroundColor

                ColumnLayout {
                    anchors.fill: parent
                    anchors.leftMargin: 22
                    anchors.rightMargin: 22
                    spacing: 0

                    Item {
                        Layout.fillWidth: true
                        Layout.preferredHeight: 56

                        RowLayout {
                            anchors.fill: parent
                            spacing: 4

                            ToolButton {
                                id: backButton
                                text: "‹"
                                font.pixelSize: 30
                                palette.buttonText: root.textColor
                                ToolTip.visible: hovered
                                ToolTip.text: "Back"
                                onClicked: root.backend.back()
                                background: Rectangle { radius: 7; color: backButton.hovered ? root.raisedColor : "transparent" }
                            }
                            ToolButton {
                                id: forwardButton
                                text: "›"
                                font.pixelSize: 30
                                palette.buttonText: root.textColor
                                ToolTip.visible: hovered
                                ToolTip.text: "Forward"
                                onClicked: root.backend.forward()
                                background: Rectangle { radius: 7; color: forwardButton.hovered ? root.raisedColor : "transparent" }
                            }
                            ToolButton {
                                id: upButton
                                text: "↑"
                                font.pixelSize: 20
                                palette.buttonText: root.textColor
                                ToolTip.visible: hovered
                                ToolTip.text: "Parent folder"
                                onClicked: root.backend.up()
                                background: Rectangle { radius: 7; color: upButton.hovered ? root.raisedColor : "transparent" }
                            }

                            TextField {
                                id: locationField
                                Layout.fillWidth: true
                                Layout.leftMargin: 6
                                text: root.backend.currentPath
                                readOnly: !root.editingLocation
                                selectByMouse: true
                                color: root.editingLocation ? root.textColor : root.accent
                                font.pixelSize: 15
                                font.weight: root.editingLocation ? Font.Normal : Font.DemiBold
                                leftPadding: 12
                                rightPadding: 12
                                onAccepted: {
                                    root.backend.navigate(text)
                                    root.editingLocation = false
                                }
                                onActiveFocusChanged: if (!activeFocus) root.editingLocation = false
                                background: Rectangle {
                                    radius: 8
                                    color: root.editingLocation ? root.raisedColor : "transparent"
                                    border.width: root.editingLocation ? 1 : 0
                                    border.color: root.accent
                                }

                                MouseArea {
                                    anchors.fill: parent
                                    enabled: !root.editingLocation
                                    cursorShape: Qt.PointingHandCursor
                                    onClicked: {
                                        root.editingLocation = true
                                        locationField.forceActiveFocus()
                                        locationField.selectAll()
                                    }
                                }
                            }

                            ToolButton {
                                id: editLocationButton
                                text: "✎"
                                font.pixelSize: 17
                                palette.buttonText: root.accent
                                ToolTip.visible: hovered
                                ToolTip.text: "Edit location"
                                onClicked: {
                                    root.editingLocation = true
                                    locationField.forceActiveFocus()
                                    locationField.selectAll()
                                }
                                background: Rectangle { radius: 7; color: editLocationButton.hovered ? root.raisedColor : "transparent" }
                            }
                        }

                        Rectangle { anchors.bottom: parent.bottom; width: parent.width; height: 1; color: root.accent }
                    }

                    Rectangle {
                        Layout.fillWidth: true
                        Layout.preferredHeight: 34
                        color: root.backgroundColor

                        RowLayout {
                            anchors.fill: parent
                            anchors.leftMargin: 12
                            anchors.rightMargin: 12
                            spacing: 0

                            Label { text: "Name"; color: root.textColor; font.weight: Font.DemiBold; Layout.fillWidth: true }
                            Label { text: "Modified"; color: root.textColor; font.weight: Font.DemiBold; Layout.preferredWidth: 185 }
                            Label { text: "Size"; color: root.textColor; font.weight: Font.DemiBold; Layout.preferredWidth: 110 }
                        }

                        Rectangle { anchors.bottom: parent.bottom; width: parent.width; height: 1; color: "#343434" }
                    }

                    ListView {
                        id: entryList
                        Layout.fillWidth: true
                        Layout.fillHeight: true
                        clip: true
                        boundsBehavior: Flickable.StopAtBounds
                        model: root.entryCount
                        spacing: 2

                        ScrollBar.vertical: ScrollBar { policy: ScrollBar.AsNeeded }

                        delegate: Rectangle {
                            id: entryDelegate
                            required property int index
                            readonly property bool selected: root.backend.entrySelected(index)

                            width: ListView.view.width
                            height: 38
                            radius: 6
                            color: selected ? root.accent : (entryMouse.containsMouse ? root.raisedColor : "transparent")

                            RowLayout {
                                anchors.fill: parent
                                anchors.leftMargin: 10
                                anchors.rightMargin: 12
                                spacing: 0

                                Label {
                                    text: root.backend.entryIsDirectory(entryDelegate.index) ? "▣" : "▪"
                                    color: entryDelegate.selected ? root.selectedTextColor : root.accent
                                    font.pixelSize: 15
                                    Layout.preferredWidth: 28
                                }
                                Label {
                                    text: root.backend.entryName(entryDelegate.index)
                                    color: entryDelegate.selected ? root.selectedTextColor : root.textColor
                                    font.pixelSize: 14
                                    elide: Text.ElideRight
                                    Layout.fillWidth: true
                                }
                                Label {
                                    text: root.modifiedText(root.backend.entryModified(entryDelegate.index))
                                    color: entryDelegate.selected ? root.selectedTextColor : root.mutedColor
                                    font.pixelSize: 13
                                    Layout.preferredWidth: 185
                                }
                                Label {
                                    text: root.backend.entryDetail(entryDelegate.index)
                                    color: entryDelegate.selected ? root.selectedTextColor : root.mutedColor
                                    font.pixelSize: 13
                                    Layout.preferredWidth: 110
                                }
                            }

                            MouseArea {
                                id: entryMouse
                                anchors.fill: parent
                                hoverEnabled: true
                                acceptedButtons: Qt.LeftButton | Qt.RightButton
                                cursorShape: Qt.PointingHandCursor
                                onClicked: root.backend.toggleSelection(entryDelegate.index)
                                onDoubleClicked: {
                                    if (root.backend.entryIsDirectory(entryDelegate.index))
                                        root.backend.navigateRow(entryDelegate.index)
                                }
                            }
                        }

                        Label {
                            anchors.centerIn: parent
                            visible: root.entryCount === 0
                            text: "This folder is empty"
                            color: root.mutedColor
                            font.pixelSize: 15
                        }
                    }

                    Rectangle {
                        Layout.fillWidth: true
                        Layout.preferredHeight: root.selectedCount > 0 ? 58 : 0
                        visible: height > 0
                        color: root.panelColor
                        radius: 9

                        RowLayout {
                            anchors.fill: parent
                            anchors.leftMargin: 12
                            anchors.rightMargin: 10
                            spacing: 8

                            Label {
                                text: root.selectedCount + " selected"
                                color: root.accent
                                font.weight: Font.DemiBold
                                Layout.preferredWidth: 82
                            }
                            TextField {
                                id: destinationField
                                Layout.fillWidth: true
                                text: root.backend.currentPath
                                placeholderText: "Destination folder"
                                color: root.textColor
                                leftPadding: 10
                                background: Rectangle { color: root.raisedColor; radius: 7; border.color: "#444444" }
                            }
                            Button {
                                id: copyButton
                                text: "Copy"
                                palette.buttonText: root.textColor
                                onClicked: root.backend.copySelected(destinationField.text)
                                background: Rectangle { radius: 7; color: copyButton.hovered ? root.hoverColor : root.raisedColor }
                            }
                            Button {
                                id: moveButton
                                text: "Move"
                                palette.buttonText: root.textColor
                                onClicked: root.backend.moveSelected(destinationField.text)
                                background: Rectangle { radius: 7; color: moveButton.hovered ? root.hoverColor : root.raisedColor }
                            }
                            Button {
                                id: trashButton
                                text: "Trash"
                                palette.buttonText: root.textColor
                                onClicked: root.backend.trashSelected()
                                background: Rectangle { radius: 7; color: trashButton.hovered ? root.hoverColor : root.raisedColor }
                            }
                            Button {
                                id: deleteButton
                                text: "Delete"
                                palette.buttonText: "#ff9b9b"
                                onClicked: deleteDialog.open()
                                background: Rectangle { radius: 7; color: deleteButton.hovered ? "#4a2929" : root.raisedColor }
                            }
                            ToolButton {
                                id: clearSelectionButton
                                text: "×"
                                font.pixelSize: 20
                                palette.buttonText: root.mutedColor
                                onClicked: root.backend.clearSelection()
                                background: Rectangle { radius: 7; color: clearSelectionButton.hovered ? root.hoverColor : "transparent" }
                            }
                        }
                    }

                    Rectangle {
                        Layout.fillWidth: true
                        Layout.preferredHeight: root.taskCount > 0 ? Math.min(142, taskList.contentHeight + 38) : 0
                        visible: height > 0
                        color: root.panelColor
                        radius: 9

                        ColumnLayout {
                            anchors.fill: parent
                            anchors.margins: 8
                            spacing: 4

                            Label { text: "File operations"; color: root.mutedColor; font.pixelSize: 12 }

                            ListView {
                                id: taskList
                                Layout.fillWidth: true
                                Layout.fillHeight: true
                                clip: true
                                model: root.taskCount
                                spacing: 4

                                delegate: RowLayout {
                                    id: taskDelegate
                                    required property int index
                                    width: ListView.view.width
                                    spacing: 9

                                    Label { text: root.backend.taskTitle(taskDelegate.index); color: root.textColor; Layout.fillWidth: true; elide: Text.ElideRight }
                                    ProgressBar {
                                        from: 0
                                        to: 10000
                                        value: root.backend.taskProgress(taskDelegate.index)
                                        Layout.preferredWidth: 180
                                    }
                                    Label { text: root.backend.taskStatus(taskDelegate.index); color: root.mutedColor; Layout.preferredWidth: 72 }
                                    Button {
                                        text: "Cancel"
                                        enabled: root.backend.taskActive(taskDelegate.index)
                                        onClicked: root.backend.cancelTask(root.backend.taskId(taskDelegate.index))
                                    }
                                }
                            }
                        }
                    }

                    Item {
                        Layout.fillWidth: true
                        Layout.preferredHeight: 28

                        RowLayout {
                            anchors.fill: parent
                            Label {
                                text: root.entryCount + (root.entryCount === 1 ? " item" : " items")
                                color: root.mutedColor
                                font.pixelSize: 12
                            }
                            Item { Layout.fillWidth: true }
                            Label {
                                visible: root.backend.lastError.length > 0
                                text: root.backend.lastError
                                color: "#ff8f8f"
                                font.pixelSize: 12
                                elide: Text.ElideRight
                                Layout.maximumWidth: 520
                            }
                        }
                    }
                }
            }
        }
    }

    Dialog {
        id: deleteDialog
        x: Math.round((root.width - width) / 2)
        y: Math.round((root.height - height) / 2)
        width: 420
        title: "Delete permanently?"
        modal: true
        standardButtons: Dialog.Yes | Dialog.No
        onAccepted: root.backend.deleteSelected()
        background: Rectangle { color: root.panelColor; radius: 10; border.color: "#464646" }
        contentItem: Label {
            text: "The selected items will be permanently deleted."
            color: root.textColor
            wrapMode: Text.WordWrap
            leftPadding: 16
            rightPadding: 16
            topPadding: 12
            bottomPadding: 12
        }
    }
}
