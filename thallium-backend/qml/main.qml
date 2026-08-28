pragma ComponentBehavior: Bound

import QtQuick
import QtQuick.Controls
import org.thallium.FileManager 1.0

// Thallium Files.
//
// Thallium the element is named for the bright green line in its emission
// spectrum. That line is the only chroma in this interface: it marks the
// active view and live work, and nothing else. Everything that carries
// structure is graphite. Every path, byte count and timestamp is set in a
// monospace face, because in a file manager the data is the content.
ApplicationWindow {
    id: root

    // Surfaces: one hue, lightness steps only.
    readonly property color canvas: "#0E0F10"
    readonly property color raised: "#17191A"
    readonly property color raisedHigh: "#1D2021"
    readonly property color hover: "#202324"
    readonly property color inset: "#0A0B0C"
    // Borders: low-opacity so they define edges without being seen.
    readonly property color line: "#14FFFFFF"
    readonly property color lineSoft: "#0BFFFFFF"
    readonly property color lineStrong: "#26FFFFFF"
    // Text: four levels, used consistently.
    readonly property color ink: "#E6E8E7"
    readonly property color inkMuted: "#9AA0A0"
    readonly property color inkFaint: "#6B7272"
    readonly property color inkGhost: "#454B4B"
    // The emission line.
    readonly property color spectral: "#5FDC82"
    readonly property color spectralDim: "#2E5FDC82"
    readonly property color destructive: "#E0645C"

    readonly property string sansFamily: "Inter, Adwaita Sans, Cantarell, Noto Sans, DejaVu Sans"
    readonly property string monoFamily: "JetBrains Mono, IBM Plex Mono, DejaVu Sans Mono, monospace"

    readonly property int unit: 4
    readonly property int railWidth: 52
    readonly property int sidebarWidth: 268
    readonly property int rowHeight: 34

    property string view: "browse"
    property string query: ""
    property var homeFolders: []

    // Re-read the imperative bridge whenever it stamps a new revision.
    readonly property int revision: backendObject.modelRevision
    readonly property int selectedCount: {
        void root.revision
        return backendObject.selectionCount()
    }
    readonly property int taskCount: {
        void root.revision
        return backendObject.taskCount()
    }

    // Directories first, then case-insensitive name, then the search filter.
    readonly property var rows: {
        void root.revision
        const needle = root.query.toLowerCase()
        const total = backendObject.entryCount()
        const kept = []
        for (let i = 0; i < total; ++i) {
            const name = backendObject.entryName(i)
            if (needle.length === 0 || name.toLowerCase().indexOf(needle) >= 0)
                kept.push({ row: i, name: name, folder: backendObject.entryIsDirectory(i) })
        }
        kept.sort(function (a, b) {
            if (a.folder !== b.folder)
                return a.folder ? -1 : 1
            return a.name.localeCompare(b.name)
        })
        return kept.map(function (entry) { return entry.row })
    }

    readonly property var tasks: {
        void root.revision
        const out = []
        const total = backendObject.taskCount()
        for (let i = 0; i < total; ++i)
            out.push(i)
        return out
    }

    readonly property var crumbs: {
        const path = backendObject.currentPath
        if (!path || path === "/")
            return [{ label: "/", path: "/" }]
        const parts = path.split("/").filter(function (part) { return part.length > 0 })
        const out = [{ label: "/", path: "/" }]
        let walked = ""
        for (let i = 0; i < parts.length; ++i) {
            walked += "/" + parts[i]
            out.push({ label: parts[i], path: walked })
        }
        return out
    }

    // The standard places, narrowed to the ones home actually contains once
    // we have seen home. Never invent a directory that is not there.
    readonly property var places: {
        const known = ["Desktop", "Documents", "Downloads", "Music", "Pictures", "Videos"]
        const seen = root.homeFolders
        const out = [{ label: "Home", folder: "" }]
        for (let i = 0; i < known.length; ++i) {
            if (seen.length === 0 || seen.indexOf(known[i]) >= 0)
                out.push({ label: known[i], folder: known[i] })
        }
        return out
    }

    width: 1240
    height: 780
    minimumWidth: 880
    minimumHeight: 520
    visible: true
    color: canvas
    title: leafName(backendObject.currentPath) + " — Thallium Files"

    function leafName(path) {
        if (!path || path === "/")
            return "Filesystem"
        const clean = path.endsWith("/") ? path.slice(0, -1) : path
        const parts = clean.split("/")
        return parts[parts.length - 1] || "/"
    }

    function placePath(folder) {
        return folder.length === 0 ? backendObject.homePath : backendObject.homePath + "/" + folder
    }

    function modifiedText(seconds) {
        if (seconds <= 0)
            return "—"
        const value = new Date(seconds * 1000)
        const now = new Date()
        if (value.getFullYear() === now.getFullYear()
                && value.getMonth() === now.getMonth()
                && value.getDate() === now.getDate())
            return value.toLocaleTimeString(Qt.locale(), "HH:mm")
        if (value.getFullYear() === now.getFullYear())
            return value.toLocaleDateString(Qt.locale(), "dd MMM  HH:mm")
        return value.toLocaleDateString(Qt.locale(), "dd MMM yyyy")
    }

    BackendBridge { id: backendObject }

    Timer {
        interval: 100
        repeat: true
        running: true
        onTriggered: backendObject.poll()
    }

    // Remember what home holds, so the places list stays truthful.
    Connections {
        target: backendObject
        function onCurrentPathChanged() {
            if (backendObject.currentPath !== backendObject.homePath)
                return
            const found = []
            const total = backendObject.entryCount()
            for (let i = 0; i < total; ++i) {
                if (backendObject.entryIsDirectory(i))
                    found.push(backendObject.entryName(i))
            }
            root.homeFolders = found
        }
    }

    // ---- Small shared pieces -------------------------------------------

    component Glyph: Text {
        color: root.inkMuted
        font.family: root.sansFamily
        font.pixelSize: 15
        horizontalAlignment: Text.AlignHCenter
        verticalAlignment: Text.AlignVCenter
    }

    component SectionTitle: Text {
        color: root.ink
        font.family: root.sansFamily
        font.pixelSize: 17
        font.weight: Font.DemiBold
        font.letterSpacing: -0.2
    }

    component Overline: Text {
        color: root.inkGhost
        font.family: root.sansFamily
        font.pixelSize: 10
        font.weight: Font.DemiBold
        font.letterSpacing: 0.9
    }

    component Mono: Text {
        color: root.inkFaint
        font.family: root.monoFamily
        font.pixelSize: 12
        elide: Text.ElideRight
        verticalAlignment: Text.AlignVCenter
    }

    component IconAction: Rectangle {
        id: iconAction
        property string glyph: ""
        property string hint: ""
        property bool enabled: true
        signal activated

        implicitWidth: 28
        implicitHeight: 28
        radius: 6
        color: mouse.containsMouse && iconAction.enabled ? root.hover : "transparent"

        Text {
            anchors.centerIn: parent
            text: iconAction.glyph
            color: iconAction.enabled
                   ? (mouse.containsMouse ? root.ink : root.inkMuted)
                   : root.inkGhost
            font.family: root.sansFamily
            font.pixelSize: 14
        }

        MouseArea {
            id: mouse
            anchors.fill: parent
            hoverEnabled: true
            cursorShape: iconAction.enabled ? Qt.PointingHandCursor : Qt.ArrowCursor
            onClicked: if (iconAction.enabled) iconAction.activated()
        }

        ToolTip.visible: mouse.containsMouse && iconAction.hint.length > 0
        ToolTip.text: iconAction.hint
        ToolTip.delay: 500
    }

    component TextAction: Rectangle {
        id: textAction
        property string label: ""
        property bool danger: false
        signal activated

        implicitWidth: caption.implicitWidth + 22
        implicitHeight: 26
        radius: 6
        color: textMouse.containsMouse ? root.hover : root.raisedHigh
        border.width: 1
        border.color: textMouse.containsMouse ? root.lineStrong : root.line

        Text {
            id: caption
            anchors.centerIn: parent
            text: textAction.label
            color: textAction.danger ? root.destructive : root.ink
            font.family: root.sansFamily
            font.pixelSize: 12
            font.weight: Font.Medium
        }

        MouseArea {
            id: textMouse
            anchors.fill: parent
            hoverEnabled: true
            cursorShape: Qt.PointingHandCursor
            onClicked: textAction.activated()
        }
    }

    // ---- Layout ---------------------------------------------------------

    Row {
        anchors.fill: parent
        spacing: 0

        // Icon rail. The active view carries the emission line.
        Rectangle {
            width: root.railWidth
            height: parent.height
            color: root.canvas

            Column {
                anchors.horizontalCenter: parent.horizontalCenter
                anchors.top: parent.top
                anchors.topMargin: 14
                spacing: 6

                // Mark.
                Rectangle {
                    width: 28
                    height: 28
                    radius: 8
                    color: root.raisedHigh
                    border.width: 1
                    border.color: root.line
                    anchors.horizontalCenter: parent.horizontalCenter

                    Text {
                        anchors.centerIn: parent
                        text: "◈"
                        color: root.spectral
                        font.pixelSize: 13
                    }
                }

                Item { width: 1; height: 10 }

                Repeater {
                    model: [
                        { id: "browse", glyph: "▤", hint: "Files" },
                        { id: "tasks", glyph: "⇅", hint: "Transfers" }
                    ]

                    delegate: Item {
                        id: railItem
                        required property var modelData
                        readonly property bool active: root.view === railItem.modelData.id

                        width: root.railWidth
                        height: 38

                        Rectangle {
                            anchors.verticalCenter: parent.verticalCenter
                            anchors.left: parent.left
                            width: 2
                            height: 18
                            radius: 1
                            color: root.spectral
                            visible: railItem.active
                        }

                        Rectangle {
                            anchors.centerIn: parent
                            width: 30
                            height: 30
                            radius: 8
                            color: railItem.active ? root.raisedHigh
                                                   : (railMouse.containsMouse ? root.raised : "transparent")

                            Text {
                                anchors.centerIn: parent
                                text: railItem.modelData.glyph
                                color: railItem.active ? root.ink : root.inkFaint
                                font.pixelSize: 14
                            }
                        }

                        // Live work is the only other place green appears.
                        Rectangle {
                            visible: railItem.modelData.id === "tasks" && root.taskCount > 0
                            anchors.right: parent.right
                            anchors.rightMargin: 9
                            anchors.top: parent.top
                            anchors.topMargin: 5
                            width: 6
                            height: 6
                            radius: 3
                            color: root.spectral
                        }

                        MouseArea {
                            id: railMouse
                            anchors.fill: parent
                            hoverEnabled: true
                            cursorShape: Qt.PointingHandCursor
                            onClicked: root.view = railItem.modelData.id
                        }
                    }
                }
            }
        }

        // Sidebar. Same canvas as the content, separated by one hairline.
        Rectangle {
            width: root.sidebarWidth
            height: parent.height
            color: root.canvas

            Rectangle {
                anchors.left: parent.left
                width: 1
                height: parent.height
                color: root.lineSoft
            }

            Column {
                anchors.fill: parent
                anchors.leftMargin: 12
                anchors.rightMargin: 12
                anchors.topMargin: 14
                spacing: 12

                Item {
                    width: parent.width
                    height: 26

                    Text {
                        anchors.verticalCenter: parent.verticalCenter
                        text: "Thallium Files"
                        color: root.ink
                        font.family: root.sansFamily
                        font.pixelSize: 13
                        font.weight: Font.DemiBold
                    }

                    IconAction {
                        anchors.right: parent.right
                        anchors.verticalCenter: parent.verticalCenter
                        glyph: "↻"
                        hint: "Refresh"
                        onActivated: backendObject.refresh()
                    }
                }

                // Search is inset: darker than its surroundings, because it
                // receives content rather than presenting it.
                Rectangle {
                    width: parent.width
                    height: 32
                    radius: 8
                    color: root.inset
                    border.width: 1
                    border.color: searchInput.activeFocus ? root.lineStrong : root.line

                    Text {
                        anchors.left: parent.left
                        anchors.leftMargin: 10
                        anchors.verticalCenter: parent.verticalCenter
                        text: "⌕"
                        color: root.inkFaint
                        font.pixelSize: 13
                    }

                    TextInput {
                        id: searchInput
                        anchors.fill: parent
                        anchors.leftMargin: 28
                        anchors.rightMargin: 10
                        verticalAlignment: Text.AlignVCenter
                        color: root.ink
                        font.family: root.sansFamily
                        font.pixelSize: 12
                        selectionColor: root.spectralDim
                        selectedTextColor: root.ink
                        clip: true
                        onTextChanged: root.query = text

                        Text {
                            anchors.verticalCenter: parent.verticalCenter
                            text: "Filter this folder"
                            color: root.inkGhost
                            font.family: root.sansFamily
                            font.pixelSize: 12
                            visible: searchInput.text.length === 0
                        }
                    }
                }

                Overline {
                    text: "PLACES"
                    leftPadding: 2
                }

                Column {
                    width: parent.width
                    spacing: 1

                    Repeater {
                        model: root.places

                        delegate: Rectangle {
                            id: place
                            required property var modelData
                            readonly property string target: root.placePath(place.modelData.folder)
                            readonly property bool active: backendObject.currentPath === place.target

                            width: parent.width
                            height: 30
                            radius: 7
                            color: place.active ? root.raisedHigh
                                                : (placeMouse.containsMouse ? root.raised : "transparent")

                            Rectangle {
                                anchors.verticalCenter: parent.verticalCenter
                                anchors.left: parent.left
                                width: 2
                                height: 14
                                radius: 1
                                color: root.spectral
                                visible: place.active
                            }

                            Text {
                                anchors.left: parent.left
                                anchors.leftMargin: 12
                                anchors.verticalCenter: parent.verticalCenter
                                text: place.modelData.folder.length === 0 ? "⌂" : "▸"
                                color: place.active ? root.spectral : root.inkGhost
                                font.pixelSize: 11
                            }

                            Text {
                                anchors.left: parent.left
                                anchors.leftMargin: 30
                                anchors.right: parent.right
                                anchors.rightMargin: 10
                                anchors.verticalCenter: parent.verticalCenter
                                text: place.modelData.label
                                color: place.active ? root.ink : root.inkMuted
                                font.family: root.sansFamily
                                font.pixelSize: 12
                                elide: Text.ElideRight
                            }

                            MouseArea {
                                id: placeMouse
                                anchors.fill: parent
                                hoverEnabled: true
                                cursorShape: Qt.PointingHandCursor
                                onClicked: backendObject.navigate(place.target)
                            }
                        }
                    }
                }
            }

            // Selection summary sits at the foot of the sidebar, where it is
            // out of the way until there is a selection to act on.
            Rectangle {
                anchors.left: parent.left
                anchors.right: parent.right
                anchors.bottom: parent.bottom
                anchors.margins: 12
                height: root.selectedCount > 0 ? 66 : 0
                visible: root.selectedCount > 0
                radius: 10
                color: root.raised
                border.width: 1
                border.color: root.line

                Text {
                    anchors.left: parent.left
                    anchors.leftMargin: 12
                    anchors.top: parent.top
                    anchors.topMargin: 10
                    text: root.selectedCount + (root.selectedCount === 1 ? " item selected" : " items selected")
                    color: root.ink
                    font.family: root.sansFamily
                    font.pixelSize: 12
                    font.weight: Font.Medium
                }

                Row {
                    anchors.left: parent.left
                    anchors.leftMargin: 12
                    anchors.bottom: parent.bottom
                    anchors.bottomMargin: 10
                    spacing: 6

                    TextAction {
                        label: "Trash"
                        onActivated: backendObject.trashSelected()
                    }
                    TextAction {
                        label: "Delete"
                        danger: true
                        onActivated: backendObject.deleteSelected()
                    }
                    TextAction {
                        label: "Clear"
                        onActivated: backendObject.clearSelection()
                    }
                }
            }
        }

        // Content.
        Rectangle {
            width: parent.width - root.railWidth - root.sidebarWidth
            height: parent.height
            color: root.canvas

            Rectangle {
                anchors.left: parent.left
                width: 1
                height: parent.height
                color: root.lineSoft
            }

            Column {
                anchors.fill: parent
                spacing: 0

                // Header: where you are, and how to move.
                Item {
                    width: parent.width
                    height: 52

                    Row {
                        anchors.left: parent.left
                        anchors.leftMargin: 16
                        anchors.verticalCenter: parent.verticalCenter
                        spacing: 2

                        IconAction {
                            glyph: "‹"
                            hint: "Back"
                            onActivated: backendObject.back()
                        }
                        IconAction {
                            glyph: "›"
                            hint: "Forward"
                            onActivated: backendObject.forward()
                        }
                        IconAction {
                            glyph: "↑"
                            hint: "Parent folder"
                            onActivated: backendObject.up()
                        }
                    }

                    // Breadcrumb, in mono: a path is data.
                    Row {
                        anchors.left: parent.left
                        anchors.leftMargin: 16 + 28 * 3 + 12
                        anchors.right: parent.right
                        anchors.rightMargin: 16
                        anchors.verticalCenter: parent.verticalCenter
                        spacing: 0
                        clip: true

                        Repeater {
                            model: root.crumbs

                            delegate: Row {
                                id: crumb
                                required property var modelData
                                required property int index
                                readonly property bool last: crumb.index === root.crumbs.length - 1
                                spacing: 0

                                Text {
                                    anchors.verticalCenter: parent.verticalCenter
                                    text: "/"
                                    color: root.inkGhost
                                    font.family: root.monoFamily
                                    font.pixelSize: 12
                                    visible: crumb.index > 1
                                }

                                Rectangle {
                                    height: 24
                                    width: crumbLabel.implicitWidth + 12
                                    radius: 6
                                    color: crumbMouse.containsMouse && !crumb.last ? root.hover : "transparent"

                                    Text {
                                        id: crumbLabel
                                        anchors.centerIn: parent
                                        text: crumb.modelData.label
                                        color: crumb.last ? root.ink : root.inkFaint
                                        font.family: root.monoFamily
                                        font.pixelSize: 12
                                        font.weight: crumb.last ? Font.DemiBold : Font.Normal
                                    }

                                    MouseArea {
                                        id: crumbMouse
                                        anchors.fill: parent
                                        hoverEnabled: true
                                        cursorShape: crumb.last ? Qt.ArrowCursor : Qt.PointingHandCursor
                                        onClicked: if (!crumb.last) backendObject.navigate(crumb.modelData.path)
                                    }
                                }
                            }
                        }
                    }

                    Rectangle {
                        anchors.bottom: parent.bottom
                        width: parent.width
                        height: 1
                        color: root.lineSoft
                    }
                }

                // Error banner. Failures are told, never swallowed.
                Rectangle {
                    width: parent.width
                    height: backendObject.lastError.length > 0 ? 34 : 0
                    visible: backendObject.lastError.length > 0
                    color: root.raised

                    Text {
                        anchors.left: parent.left
                        anchors.leftMargin: 16
                        anchors.right: parent.right
                        anchors.rightMargin: 16
                        anchors.verticalCenter: parent.verticalCenter
                        text: backendObject.lastError
                        color: root.destructive
                        font.family: root.monoFamily
                        font.pixelSize: 11
                        elide: Text.ElideRight
                    }

                    Rectangle {
                        anchors.bottom: parent.bottom
                        width: parent.width
                        height: 1
                        color: root.lineSoft
                    }
                }

                // ---- Browse view --------------------------------------

                Item {
                    width: parent.width
                    height: parent.height - 52 - (backendObject.lastError.length > 0 ? 34 : 0)
                    visible: root.view === "browse"

                    Column {
                        anchors.fill: parent
                        spacing: 0

                        // Quick access, as dimensional folder cards.
                        Item {
                            width: parent.width
                            height: 172

                            SectionTitle {
                                id: quickTitle
                                anchors.left: parent.left
                                anchors.leftMargin: 16
                                anchors.top: parent.top
                                anchors.topMargin: 18
                                text: "Quick access"
                            }

                            Row {
                                anchors.left: parent.left
                                anchors.leftMargin: 16
                                anchors.top: quickTitle.bottom
                                anchors.topMargin: 14
                                spacing: 10

                                Repeater {
                                    model: root.places

                                    delegate: Rectangle {
                                        id: card
                                        required property var modelData
                                        readonly property string target: root.placePath(card.modelData.folder)
                                        readonly property bool active: backendObject.currentPath === card.target

                                        width: 148
                                        height: 96
                                        radius: 10
                                        color: cardMouse.containsMouse ? root.raisedHigh : root.raised
                                        border.width: 1
                                        border.color: card.active ? root.lineStrong : root.line

                                        // Folder mark: a tab and a face, lit
                                        // from above like brushed metal.
                                        Item {
                                            id: mark
                                            anchors.left: parent.left
                                            anchors.leftMargin: 14
                                            anchors.top: parent.top
                                            anchors.topMargin: 14
                                            width: 34
                                            height: 26

                                            Rectangle {
                                                width: 15
                                                height: 5
                                                radius: 2
                                                color: "#313536"
                                                anchors.top: parent.top
                                                anchors.left: parent.left
                                            }

                                            Rectangle {
                                                anchors.fill: parent
                                                anchors.topMargin: 4
                                                radius: 4
                                                border.width: 1
                                                border.color: root.line
                                                gradient: Gradient {
                                                    GradientStop { position: 0.0; color: "#3A3F40" }
                                                    GradientStop { position: 1.0; color: "#242829" }
                                                }
                                            }
                                        }

                                        Text {
                                            anchors.left: parent.left
                                            anchors.leftMargin: 14
                                            anchors.right: parent.right
                                            anchors.rightMargin: 12
                                            anchors.bottom: cardHint.top
                                            anchors.bottomMargin: 3
                                            text: card.modelData.label
                                            color: root.ink
                                            font.family: root.sansFamily
                                            font.pixelSize: 13
                                            font.weight: Font.Medium
                                            elide: Text.ElideRight
                                        }

                                        Mono {
                                            id: cardHint
                                            anchors.left: parent.left
                                            anchors.leftMargin: 14
                                            anchors.right: parent.right
                                            anchors.rightMargin: 12
                                            anchors.bottom: parent.bottom
                                            anchors.bottomMargin: 12
                                            text: card.modelData.folder.length === 0 ? "~" : "~/" + card.modelData.folder
                                            color: root.inkGhost
                                            font.pixelSize: 11
                                        }

                                        MouseArea {
                                            id: cardMouse
                                            anchors.fill: parent
                                            hoverEnabled: true
                                            cursorShape: Qt.PointingHandCursor
                                            onClicked: backendObject.navigate(card.target)
                                        }
                                    }
                                }
                            }
                        }

                        // The listing.
                        Item {
                            width: parent.width
                            height: parent.height - 172

                            Item {
                                id: listHeader
                                anchors.left: parent.left
                                anchors.right: parent.right
                                anchors.top: parent.top
                                height: 40

                                SectionTitle {
                                    anchors.left: parent.left
                                    anchors.leftMargin: 16
                                    anchors.verticalCenter: parent.verticalCenter
                                    text: root.leafName(backendObject.currentPath)
                                }

                                Mono {
                                    anchors.right: parent.right
                                    anchors.rightMargin: 16
                                    anchors.verticalCenter: parent.verticalCenter
                                    text: root.rows.length + (root.query.length > 0 ? " of " + backendObject.entryCount() : "")
                                          + (root.rows.length === 1 ? " item" : " items")
                                    color: root.inkGhost
                                    font.pixelSize: 11
                                }
                            }

                            // Column rule.
                            Item {
                                id: columns
                                anchors.left: parent.left
                                anchors.right: parent.right
                                anchors.top: listHeader.bottom
                                height: 26

                                Overline {
                                    anchors.left: parent.left
                                    anchors.leftMargin: 16 + 26
                                    anchors.verticalCenter: parent.verticalCenter
                                    text: "NAME"
                                }
                                Overline {
                                    anchors.right: parent.right
                                    anchors.rightMargin: 16 + 150
                                    anchors.verticalCenter: parent.verticalCenter
                                    width: 84
                                    horizontalAlignment: Text.AlignRight
                                    text: "SIZE"
                                }
                                Overline {
                                    anchors.right: parent.right
                                    anchors.rightMargin: 16
                                    width: 132
                                    anchors.verticalCenter: parent.verticalCenter
                                    text: "MODIFIED"
                                }

                                Rectangle {
                                    anchors.bottom: parent.bottom
                                    width: parent.width
                                    height: 1
                                    color: root.lineSoft
                                }
                            }

                            ListView {
                                id: listing
                                anchors.left: parent.left
                                anchors.right: parent.right
                                anchors.top: columns.bottom
                                anchors.bottom: parent.bottom
                                clip: true
                                model: root.rows
                                boundsBehavior: Flickable.StopAtBounds

                                delegate: Item {
                                    id: entry
                                    required property int modelData
                                    readonly property string name: {
                                        void root.revision
                                        return backendObject.entryName(entry.modelData)
                                    }
                                    readonly property bool folder: {
                                        void root.revision
                                        return backendObject.entryIsDirectory(entry.modelData)
                                    }
                                    readonly property bool picked: {
                                        void root.revision
                                        return backendObject.entrySelected(entry.modelData)
                                    }

                                    width: listing.width
                                    height: root.rowHeight

                                    Rectangle {
                                        anchors.fill: parent
                                        anchors.leftMargin: 8
                                        anchors.rightMargin: 8
                                        radius: 6
                                        color: entry.picked ? root.raisedHigh
                                                            : (entryMouse.containsMouse ? root.raised : "transparent")
                                    }

                                    Rectangle {
                                        anchors.verticalCenter: parent.verticalCenter
                                        anchors.left: parent.left
                                        anchors.leftMargin: 8
                                        width: 2
                                        height: 16
                                        radius: 1
                                        color: root.spectral
                                        visible: entry.picked
                                    }

                                    Text {
                                        anchors.left: parent.left
                                        anchors.leftMargin: 18
                                        anchors.verticalCenter: parent.verticalCenter
                                        width: 18
                                        text: entry.folder ? "▸" : "·"
                                        color: entry.folder ? root.inkMuted : root.inkGhost
                                        font.pixelSize: entry.folder ? 11 : 15
                                    }

                                    Text {
                                        anchors.left: parent.left
                                        anchors.leftMargin: 16 + 26
                                        anchors.right: parent.right
                                        anchors.rightMargin: 16 + 150 + 90
                                        anchors.verticalCenter: parent.verticalCenter
                                        text: entry.name
                                        color: entry.picked ? root.ink : (entry.folder ? root.ink : root.inkMuted)
                                        font.family: root.monoFamily
                                        font.pixelSize: 12
                                        elide: Text.ElideMiddle
                                    }

                                    Mono {
                                        anchors.right: parent.right
                                        anchors.rightMargin: 16 + 150
                                        anchors.verticalCenter: parent.verticalCenter
                                        width: 84
                                        horizontalAlignment: Text.AlignRight
                                        text: {
                                            void root.revision
                                            return backendObject.entryDetail(entry.modelData)
                                        }
                                        color: root.inkFaint
                                        font.pixelSize: 11
                                    }

                                    Mono {
                                        anchors.right: parent.right
                                        anchors.rightMargin: 16
                                        anchors.verticalCenter: parent.verticalCenter
                                        width: 132
                                        text: {
                                            void root.revision
                                            return root.modifiedText(backendObject.entryModified(entry.modelData))
                                        }
                                        color: root.inkGhost
                                        font.pixelSize: 11
                                    }

                                    MouseArea {
                                        id: entryMouse
                                        anchors.fill: parent
                                        hoverEnabled: true
                                        cursorShape: Qt.PointingHandCursor
                                        onClicked: backendObject.toggleSelection(entry.modelData)
                                        onDoubleClicked: if (entry.folder) backendObject.navigateRow(entry.modelData)
                                    }
                                }
                            }

                            // Empty states, distinguished: a folder with
                            // nothing in it is not a filter that matched
                            // nothing.
                            Column {
                                anchors.centerIn: parent
                                spacing: 6
                                visible: root.rows.length === 0

                                Text {
                                    anchors.horizontalCenter: parent.horizontalCenter
                                    text: root.query.length > 0 ? "No match" : "Empty folder"
                                    color: root.inkFaint
                                    font.family: root.sansFamily
                                    font.pixelSize: 13
                                }

                                Mono {
                                    anchors.horizontalCenter: parent.horizontalCenter
                                    text: root.query.length > 0 ? "filter: " + root.query : backendObject.currentPath
                                    color: root.inkGhost
                                    font.pixelSize: 11
                                }
                            }
                        }
                    }
                }

                // ---- Transfers view -----------------------------------

                Item {
                    width: parent.width
                    height: parent.height - 52 - (backendObject.lastError.length > 0 ? 34 : 0)
                    visible: root.view === "tasks"

                    SectionTitle {
                        id: tasksTitle
                        anchors.left: parent.left
                        anchors.leftMargin: 16
                        anchors.top: parent.top
                        anchors.topMargin: 18
                        text: "Transfers"
                    }

                    ListView {
                        anchors.left: parent.left
                        anchors.right: parent.right
                        anchors.top: tasksTitle.bottom
                        anchors.topMargin: 14
                        anchors.bottom: parent.bottom
                        anchors.leftMargin: 16
                        anchors.rightMargin: 16
                        clip: true
                        spacing: 8
                        model: root.tasks
                        boundsBehavior: Flickable.StopAtBounds

                        delegate: Rectangle {
                            id: task
                            required property int modelData
                            readonly property string title: {
                                void root.revision
                                return backendObject.taskTitle(task.modelData)
                            }
                            readonly property string status: {
                                void root.revision
                                return backendObject.taskStatus(task.modelData)
                            }
                            readonly property int basisPoints: {
                                void root.revision
                                return backendObject.taskProgress(task.modelData)
                            }
                            readonly property bool live: {
                                void root.revision
                                return backendObject.taskActive(task.modelData)
                            }

                            width: ListView.view.width
                            height: 62
                            radius: 10
                            color: root.raised
                            border.width: 1
                            border.color: root.line

                            Text {
                                anchors.left: parent.left
                                anchors.leftMargin: 14
                                anchors.top: parent.top
                                anchors.topMargin: 12
                                text: task.title
                                color: root.ink
                                font.family: root.sansFamily
                                font.pixelSize: 12
                                font.weight: Font.Medium
                            }

                            Mono {
                                anchors.right: cancel.left
                                anchors.rightMargin: 10
                                anchors.top: parent.top
                                anchors.topMargin: 13
                                text: task.status + "  " + Math.round(task.basisPoints / 100) + "%"
                                color: task.live ? root.spectral : root.inkFaint
                                font.pixelSize: 11
                            }

                            IconAction {
                                id: cancel
                                anchors.right: parent.right
                                anchors.rightMargin: 10
                                anchors.top: parent.top
                                anchors.topMargin: 9
                                glyph: "✕"
                                hint: "Cancel"
                                enabled: task.live
                                onActivated: backendObject.cancelTask(backendObject.taskId(task.modelData))
                            }

                            // Progress reads as the same emission line,
                            // stretched.
                            Rectangle {
                                anchors.left: parent.left
                                anchors.leftMargin: 14
                                anchors.right: parent.right
                                anchors.rightMargin: 14
                                anchors.bottom: parent.bottom
                                anchors.bottomMargin: 14
                                height: 3
                                radius: 2
                                color: root.inset

                                Rectangle {
                                    height: parent.height
                                    radius: 2
                                    width: parent.width * Math.min(1, task.basisPoints / 10000)
                                    color: task.live ? root.spectral : root.inkGhost

                                    Behavior on width {
                                        NumberAnimation { duration: 160; easing.type: Easing.OutCubic }
                                    }
                                }
                            }
                        }
                    }

                    Column {
                        anchors.centerIn: parent
                        spacing: 6
                        visible: root.tasks.length === 0

                        Text {
                            anchors.horizontalCenter: parent.horizontalCenter
                            text: "No transfers"
                            color: root.inkFaint
                            font.family: root.sansFamily
                            font.pixelSize: 13
                        }

                        Mono {
                            anchors.horizontalCenter: parent.horizontalCenter
                            text: "copy, move, trash and delete appear here"
                            color: root.inkGhost
                            font.pixelSize: 11
                        }
                    }
                }
            }
        }
    }
}
