pragma ComponentBehavior: Bound

import QtQuick
import QtQuick.Controls
import QtQuick.Shapes
import QtQuick.Effects
import org.thallium.FileManager 1.0

// Thallium Files.
//
// Monochrome throughout: surfaces step in lightness, state is shown by fill
// and a full outline, and the only ornament is the cross-ended hairline.
ApplicationWindow {
    id: root

    // The palette lives in Theme. These names are what the rest of this
    // file was written against; the design is monochrome, so the old accent
    // and warning roles all resolve to steps of grey.
    readonly property color canvas: Theme.canvas
    readonly property color raised: Theme.surface
    readonly property color raisedHigh: Theme.selected
    readonly property color hover: Theme.surfaceHigh
    readonly property color inset: Theme.inset

    readonly property color line: Theme.line
    readonly property color lineSoft: Theme.line
    readonly property color lineStrong: Theme.outline

    readonly property color ink: Theme.ink
    readonly property color inkMuted: Theme.inkSoft
    readonly property color inkFaint: Theme.inkDim
    readonly property color inkGhost: Theme.inkFaint

    readonly property color spectral: Theme.ink
    readonly property color spectralDim: "#33FFFFFF"

    readonly property color ember: Theme.inkSoft
    readonly property color emberInk: Theme.ink
    readonly property color emberWash: Theme.selected
    readonly property color emberEdge: Theme.outline
    readonly property color destructive: Theme.inkSoft

    readonly property string sansFamily: Theme.family
    readonly property string monoFamily: Theme.family

    readonly property int unit: 4
    readonly property int sidebarWidth: Theme.railWidth + Theme.placesWidth
    // Every control on the header line is this tall, and every band
    // in the folder bar is the next one. Mixing 30 with 34 is what
    // made the top of the window look ragged.
    readonly property int controlHeight: 34
    readonly property int bandHeight: 52
    readonly property int inspectorWidth: 348
    readonly property int rowHeight: 40

    property string view: "list"
    property string query: ""
    property string folderQuery: ""
    property bool showHidden: false
    property string sidebarTab: "places"
    property bool previewOpen: true
    property string sortKey: "name"
    property bool sortAscending: true
    property int focusRow: -1
    property var homeFolders: []
    // Adding a remote. `configTick` is bumped after every call into the
    // configuration conversation, because the answers live in the backend and
    // nothing else would tell the bindings to look again.
    property bool remoteSheetOpen: false
    property int configTick: 0
    property string remoteDraftName: ""
    property string providerQuery: ""

    // Re-read the imperative bridge whenever it stamps a new revision.
    readonly property int revision: root.bridge.modelRevision
    readonly property int selectedCount: {
        void root.revision
        return root.bridge.selectionCount()
    }
    readonly property int taskCount: {
        void root.revision
        return root.bridge.taskCount()
    }

    // Directories first, then the chosen sort key, then the search filter.
    readonly property var rows: {
        void root.revision
        const needle = root.query.toLowerCase()
        const total = root.bridge.entryCount()
        const kept = []
        for (let i = 0; i < total; ++i) {
            const name = root.bridge.entryName(i)
            if (!root.showHidden && root.isHidden(name))
                continue
            if (needle.length === 0 || name.toLowerCase().indexOf(needle) >= 0) {
                kept.push({
                    row: i,
                    name: name,
                    folder: root.bridge.entryIsDirectory(i),
                    modified: root.bridge.entryModified(i)
                })
            }
        }
        const direction = root.sortAscending ? 1 : -1
        const key = root.sortKey
        kept.sort(function (a, b) {
            if (a.folder !== b.folder)
                return a.folder ? -1 : 1
            if (key === "modified")
                return (a.modified - b.modified) * direction
            if (key === "kind") {
                const left = root.kindOf(a.name, a.folder)
                const right = root.kindOf(b.name, b.folder)
                if (left !== right)
                    return left.localeCompare(right) * direction
            }
            return a.name.localeCompare(b.name) * direction
        })
        return kept.map(function (entry) { return entry.row })
    }

    // The first row that is not a directory, so the list can draw a seam
    // between structure and content.
    readonly property int firstFileRow: {
        void root.revision
        for (let i = 0; i < root.rows.length; ++i) {
            if (!root.bridge.entryIsDirectory(root.rows[i]))
                return root.rows[i]
        }
        return -1
    }

    readonly property var tasks: {
        void root.revision
        const out = []
        const total = root.bridge.taskCount()
        for (let i = 0; i < total; ++i)
            out.push(i)
        return out
    }

    readonly property var crumbs: {
        const path = root.bridge.currentPath
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
        const icons = { Desktop: "folder", Documents: "file-text", Downloads: "download",
                        Music: "music", Pictures: "image", Videos: "video" }
        const out = [ { label: "Home", folder: "", icon: "house", section: "Places" } ]
        for (let i = 0; i < known.length; ++i) {
            if (seen.length === 0 || seen.indexOf(known[i]) >= 0)
                out.push({ label: known[i], folder: known[i], icon: icons[known[i]], section: "Places" })
        }
        out.push({ label: "Filesystem", folder: "/", icon: "hard-drive", section: "Places" })
        // Drives that can be mounted, mounted or not.
        void root.revision
        for (let d = 0; d < root.bridge.driveCount(); ++d)
            out.push({ label: root.bridge.driveLabel(d), folder: null, target: root.bridge.driveMount(d), drive: d,
                       icon: root.bridge.driveEjectable(d) ? "usb" : "hard-drive", section: "Drives" })
        // The GTK bookmarks, less any that name a place already listed.
        void root.revision
        const listed = out.map(function (place) { return root.placePath(place.folder) })
        for (let b = 0; b < root.bridge.bookmarkCount(); ++b) {
            const path = root.bridge.bookmarkPath(b)
            if (listed.indexOf(path) < 0)
                out.push({ label: root.bridge.bookmarkLabel(b), folder: null, target: path, bookmark: b,
                           icon: "bookmark", section: "Bookmarks" })
        }
        // Whatever rclone is configured for, listed after the local places
        // because that is the order they are reached in.
        void root.configTick
        const remotes = root.bridge.remoteCount()
        for (let r = 0; r < remotes; ++r) {
            out.push({
                label: root.bridge.remoteName(r),
                folder: null,
                target: root.bridge.remoteLocation(r),
                remote: true,
                icon: "globe",
                section: "Network"
            })
        }
        if (root.bridge.remotesConfigurable())
            out.push({ label: "Add a remote", folder: null, add: true, icon: "plus", section: "Network" })
        return out
    }

    // What this folder is made of, by entry count. Measured, never guessed:
    // the ring only ever describes the directory currently on screen.
    readonly property var composition: {
        void root.revision
        const buckets = { Folders: 0, Code: 0, Documents: 0, Media: 0, Archives: 0, Other: 0 }
        for (let i = 0; i < root.rows.length; ++i) {
            const at = root.rows[i]
            buckets[root.kindOf(root.bridge.entryName(at), root.bridge.entryIsDirectory(at))] += 1
        }
        return [
            { label: "Folders", count: buckets.Folders, shade: root.ink },
            { label: "Code", count: buckets.Code, shade: root.inkMuted },
            { label: "Documents", count: buckets.Documents, shade: root.inkFaint },
            { label: "Media", count: buckets.Media, shade: root.emberInk },
            { label: "Other", count: buckets.Other + buckets.Archives, shade: root.inkGhost }
        ]
    }

    // The tree is the live path: every ancestor of the folder on screen,
    // then the subfolders it actually contains. Nothing is invented, and
    // nothing is read from a directory we have not opened.
    readonly property var tree: {
        void root.revision
        const out = []
        const chain = root.crumbs
        for (let i = 0; i < chain.length; ++i) {
            out.push({
                label: chain[i].label === "/" ? "Filesystem" : chain[i].label,
                path: chain[i].path,
                depth: i,
                current: i === chain.length - 1,
                badge: ""
            })
        }
        const needle = root.folderQuery.toLowerCase()
        const total = root.bridge.entryCount()
        for (let i = 0; i < total; ++i) {
            if (!root.bridge.entryIsDirectory(i))
                continue
            const name = root.bridge.entryName(i)
            if (!root.showHidden && root.isHidden(name))
                continue
            if (needle.length > 0 && name.toLowerCase().indexOf(needle) < 0)
                continue
            out.push({
                label: name,
                path: root.bridge.entryPath(i),
                depth: chain.length,
                current: false,
                badge: ""
            })
        }
        if (out.length > chain.length)
            out[chain.length - 1].badge = String(out.length - chain.length)
        return out
    }

    // Segments with nothing in them are not drawn, and a Row gives no spacing
    // to what it does not lay out, so the bar has to know how many there are.
    readonly property int compositionShown: {
        let shown = 0
        for (let i = 0; i < root.composition.length; ++i)
            if (root.composition[i].count > 0)
                shown += 1
        return shown
    }

    readonly property int compositionTotal: {
        let sum = 0
        for (let i = 0; i < root.composition.length; ++i)
            sum += root.composition[i].count
        return sum
    }

    width: 1480
    height: 920
    minimumWidth: 1080
    minimumHeight: 640
    visible: true
    color: canvas
    title: leafName(root.bridge.currentPath) + " — Thallium Files"

    function leafName(path) {
        if (!path || path === "/")
            return "Filesystem"
        const clean = path.endsWith("/") ? path.slice(0, -1) : path
        const parts = clean.split("/")
        return parts[parts.length - 1] || "/"
    }

    function placePath(folder) {
        // Remotes and the "add" entry carry no folder; they are not paths under
        // home and must not be resolved as if they were.
        if (!folder)
            return folder === "" ? root.bridge.homePath : ""
        if (folder === "/")
            return "/"
        return root.bridge.homePath + "/" + folder
    }

    // Bytes as a disk is spoken about: three significant figures, no more.
    function readableSize(bytes) {
        const units = ["B", "KB", "MB", "GB", "TB", "PB"]
        let value = bytes
        let at = 0
        while (value >= 1024 && at < units.length - 1) {
            value /= 1024
            at += 1
        }
        return (value >= 100 || at === 0 ? Math.round(value) : value.toFixed(1)) + " " + units[at]
    }

    // The desktop's recent files, shown in place of a directory.
    property bool recentOpen: false

    function showRecent() {
        root.endDeepSearch()
        root.trashOpen = false
        root.recentOpen = true
        root.bridge.refreshRecents()
    }

    // The trash, shown in place of a directory. Its selection is its own:
    // trash rows are not entries of any open folder.
    property bool trashOpen: false
    property var trashPicked: []

    function showTrash() {
        root.endDeepSearch()
        root.recentOpen = false
        root.trashOpen = true
        root.trashPicked = []
        root.bridge.refreshTrash()
    }

    function pickTrash(row, extend) {
        const at = root.trashPicked.indexOf(row)
        if (extend)
            root.trashPicked = at >= 0 ? root.trashPicked.filter(function (r) { return r !== row })
                                       : root.trashPicked.concat([row])
        else
            root.trashPicked = at >= 0 && root.trashPicked.length === 1 ? [] : [row]
    }

    function isHidden(name) {
        return name.length > 0 && name.charAt(0) === "."
    }

    function kindOf(name, folder) {
        if (folder)
            return "Folders"
        const dot = name.lastIndexOf(".")
        const extension = dot > 0 ? name.slice(dot + 1).toLowerCase() : ""
        if (["rs", "js", "ts", "jsx", "tsx", "py", "c", "h", "cpp", "hpp", "go", "java", "kt", "rb", "sh", "bash", "fish", "qml", "sql", "json", "toml", "yaml", "yml", "conf", "ini", "lock"].indexOf(extension) >= 0)
            return "Code"
        if (["txt", "md", "pdf", "odt", "doc", "docx", "rtf", "csv", "ods", "xlsx"].indexOf(extension) >= 0)
            return "Documents"
        if (["png", "jpg", "jpeg", "gif", "webp", "svg", "bmp", "mp4", "mkv", "webm", "mov", "mp3", "flac", "wav", "ogg"].indexOf(extension) >= 0)
            return "Media"
        if (["zip", "tar", "gz", "xz", "zst", "bz2", "7z", "rar", "deb", "rpm"].indexOf(extension) >= 0)
            return "Archives"
        return "Other"
    }

    // A mark per kind. The triangle is a claim that something can be played,
    // so only media is allowed to wear it.
    function markFor(kind) {
        if (kind === "Folders")
            return "▣"
        if (kind === "Code")
            return "{ }"
        if (kind === "Documents")
            return "▤"
        if (kind === "Media")
            return "▶"
        if (kind === "Archives")
            return "⧉"
        return "·"
    }

    function markTone(kind) {
        if (kind === "Folders")
            return root.ink
        if (kind === "Other")
            return root.inkGhost
        return root.inkFaint
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
            return value.toLocaleString(Qt.locale(), "dd MMM  HH:mm")
        return value.toLocaleDateString(Qt.locale(), "dd MMM yyyy")
    }

    // One click focuses and selects alone; ctrl or shift extends.
    // A sweep in progress is drawn from QML alone: the entries it covers are
    // marked here and committed to the backend once, on release. Pushing every
    // intermediate set through the command queue meant a clear plus one toggle
    // per covered entry on every mouse move, and a model revision behind each
    // of them, which no amount of delegate thrift could absorb.
    property var sweepMark: ({})
    property string sweepKey: ""

    // One sweep serves both views and every surface a drag can start on, so a
    // band behaves the same whether it began on empty ground or on an entry.
    property bool sweeping: false
    property bool sweptRecently: false
    property real sweepOriginX: 0
    property real sweepOriginY: 0
    property rect band: Qt.rect(0, 0, 0, 0)

    function sweepBegin(x, y) {
        root.sweepOriginX = x
        root.sweepOriginY = y
        root.sweeping = false
        root.sweptRecently = false
        root.sweepKey = ""
        root.sweepMark = ({})
    }

    function sweepMove(x, y) {
        if (!root.sweeping
            && Math.abs(x - root.sweepOriginX) < 5
            && Math.abs(y - root.sweepOriginY) < 5)
            return
        root.sweeping = true
        root.band = Qt.rect(Math.min(root.sweepOriginX, x),
                            Math.min(root.sweepOriginY, y),
                            Math.abs(x - root.sweepOriginX),
                            Math.abs(y - root.sweepOriginY))
        const hits = root.view === "grid"
                     ? root.bandGrid(root.band.x, root.band.y,
                                     root.band.x + root.band.width,
                                     root.band.y + root.band.height)
                     : root.bandList(root.band.y, root.band.y + root.band.height)
        const key = hits.join(",")
        if (key === root.sweepKey)
            return
        root.sweepKey = key
        let mark = {}
        for (let at = 0; at < hits.length; at++)
            mark[hits[at]] = true
        root.sweepMark = mark
    }

    // The one and only push of the swept selection. This has to run on the
    // release rather than the click: a drag almost always ends outside the item
    // it began on, and a MouseArea emits no click in that case, which would
    // leave the band drawn and the selection uncommitted.
    function sweepRelease() {
        const swept = root.sweeping
        root.sweeping = false
        if (swept) {
            const marked = Object.keys(root.sweepMark)
            let rows = []
            for (let at = 0; at < marked.length; at++)
                rows.push(Number(marked[at]))
            root.bridge.selectRows(rows)
        }
        root.sweepMark = ({})
        root.sweepKey = ""
        root.sweptRecently = swept
    }

    // True when the click now arriving is the tail of a sweep and must not be
    // read as a click on whatever sits under the pointer.
    function sweepConsumed() {
        const swept = root.sweptRecently
        root.sweptRecently = false
        return swept
    }

    // Both views lay entries out on a fixed pitch, so the band's bounds give
    // the covered indices by arithmetic; no per-item hit test is needed.
    function bandGrid(left, top, right, bottom) {
        const columns = Math.max(1, Math.floor(gridBrowser.width / gridBrowser.cellWidth))
        const firstRow = Math.max(0, Math.floor(top / gridBrowser.cellHeight))
        const lastRow = Math.floor(bottom / gridBrowser.cellHeight)
        const firstColumn = Math.max(0, Math.floor(left / gridBrowser.cellWidth))
        const lastColumn = Math.min(columns - 1, Math.floor(right / gridBrowser.cellWidth))
        let hits = []
        for (let r = firstRow; r <= lastRow; r++)
            for (let c = firstColumn; c <= lastColumn; c++) {
                const at = r * columns + c
                if (at >= 0 && at < root.rows.length)
                    hits.push(root.rows[at])
            }
        return hits
    }

    function bandList(top, bottom) {
        const first = Math.max(0, Math.floor(top / root.rowHeight))
        const last = Math.min(root.rows.length - 1, Math.floor(bottom / root.rowHeight))
        let hits = []
        for (let at = first; at <= last; at++)
            hits.push(root.rows[at])
        return hits
    }

    // Clicking an entry that already stands alone in the selection lets go of
    // it, so a pick can always be taken back the way it was made. Clicking a
    // different entry replaces the selection, as it always did.
    function pick(row, extend) {
        root.focusRow = row
        if (extend) {
            root.bridge.toggleSelection(row)
            return
        }
        const held = root.bridge.entrySelected(row) && root.selectedCount === 1
        root.bridge.selectRows(held ? [] : [row])
    }

    function applySort(key) {
        if (root.sortKey === key)
            root.sortAscending = !root.sortAscending
        else {
            root.sortKey = key
            root.sortAscending = true
        }
    }

    Shortcut {
        sequence: "Ctrl+H"
        onActivated: root.showHidden = !root.showHidden
    }

    // A FileManager1 properties request: show the item, then describe it once
    // its folder has listed it and selected it.
    Timer {
        id: inspectRevealed
        interval: 700
        onTriggered: root.showProperties(root.firstSelectedRow())
    }

    // ---- Sections ----
    // What the rail shows as current, derived from what is on screen.
    readonly property string section: root.trashOpen ? "trash"
        : root.recentOpen ? "recent"
        : root.bridge.currentPath === root.bridge.homePath ? "home"
        : root.bridge.currentPath.startsWith("/") ? "files"
        : "network"

    function chooseSection(key) {
        if (key === "home") {
            root.recentOpen = false
            root.trashOpen = false
            root.bridge.navigate(root.bridge.homePath)
        } else if (key === "files") {
            root.recentOpen = false
            root.trashOpen = false
        } else if (key === "recent") {
            root.showRecent()
        } else if (key === "trash") {
            root.showTrash()
        } else if (key === "network") {
            if (root.bridge.remoteCount() > 0) {
                root.recentOpen = false
                root.trashOpen = false
                root.bridge.navigate(root.bridge.remoteLocation(0))
            } else if (root.bridge.remotesConfigurable()) {
                root.remoteDraftName = ""
                root.providerQuery = ""
                root.remoteSheetOpen = true
            }
        } else if (key === "settings") {
            settingsMenu.popup(rail, rail.width + 6, rail.height / 2)
        }
    }

    MenuSurface {
        id: moreMenu
        MenuLine { text: "New tab"; onTriggered: root.newTab(root.bridge.currentPath) }
        MenuLine { text: "Open terminal here"; onTriggered: root.bridge.openTerminalHere() }
        MenuLine { text: "Go to location…"; onTriggered: root.editPath() }
    }

    MenuSurface {
        id: settingsMenu
        property bool isDefault: true
        onAboutToShow: isDefault = root.bridge.isDefaultManager()
        MenuLine {
            text: root.showHidden ? "Hide hidden files" : "Show hidden files"
            onTriggered: root.showHidden = !root.showHidden
        }
        MenuLine {
            text: root.previewOpen ? "Hide preview panel" : "Show preview panel"
            onTriggered: root.previewOpen = !root.previewOpen
        }
        MenuLine { text: "Edit custom actions…"; onTriggered: root.bridge.editActions() }
        MenuLine {
            text: "Make Thallium Files the default"
            enabled: !settingsMenu.isDefault
            onTriggered: root.bridge.makeDefaultManager()
        }
    }

    // ---- Tabs ----
    // Each tab is a bridge of its own: its own folder, history, selection and
    // watcher. Everything on screen talks to the current one through
    // `root.bridge`, so switching tabs is swapping that one property.
    property BackendBridge bridge: firstBridge
    property var tabs: [firstBridge]
    property int currentTab: 0

    Component {
        id: bridgeFactory
        BackendBridge {}
    }

    function newTab(path) {
        const tab = bridgeFactory.createObject(root)
        if (path.length > 0)
            tab.navigate(path)
        root.tabs = root.tabs.concat([tab])
        root.switchTab(root.tabs.length - 1)
    }

    function closeTab(index) {
        if (root.tabs.length <= 1)
            return
        const closing = root.tabs[index]
        const kept = root.tabs.filter(function (tab, at) { return at !== index })
        const next = Math.min(index < root.currentTab ? root.currentTab - 1 : root.currentTab, kept.length - 1)
        root.tabs = kept
        root.switchTab(next)
        if (closing !== firstBridge)
            closing.destroy()
    }

    function switchTab(index) {
        if (index < 0 || index >= root.tabs.length)
            return
        root.endDeepSearch()
        root.query = ""
        root.focusRow = -1
        root.anchorRow = -1
        root.recentOpen = false
        root.trashOpen = false
        root.currentTab = index
        root.bridge = root.tabs[index]
    }

    Shortcut { sequences: ["Ctrl+T"]; enabled: !root.typing; onActivated: root.newTab(root.bridge.currentPath) }
    Shortcut { sequences: ["Ctrl+W"]; enabled: !root.typing; onActivated: root.closeTab(root.currentTab) }
    Shortcut {
        sequences: ["Ctrl+Tab", "Ctrl+PgDown"]
        enabled: root.tabs.length > 1
        onActivated: root.switchTab((root.currentTab + 1) % root.tabs.length)
    }
    Shortcut {
        sequences: ["Ctrl+Shift+Tab", "Ctrl+PgUp"]
        enabled: root.tabs.length > 1
        onActivated: root.switchTab((root.currentTab + root.tabs.length - 1) % root.tabs.length)
    }

    BackendBridge {
        id: firstBridge
        // Other applications' "Show in folder" lands here, in a tab of its own.
        onExternalRequest: function (target, mode) {
            root.newTab("")
            root.openTarget(target)
            if (mode === "properties")
                inspectRevealed.restart()
            root.raise()
            root.requestActivate()
        }
        // `thallium-file-manager PATH` opens PATH, or the folder holding it.
        Component.onCompleted: {
            firstBridge.serveDesktop()
            const args = Qt.application.arguments
            if (args.length > 1)
                root.openTarget(args[args.length - 1])
        }
    }

    // Filtering the open folder is instant and happens as you type. Walking the
    // tree beneath it is not, so it waits for a pause and is abandoned the
    // moment the query moves on.
    property bool deepSearch: false
    readonly property int searchHits: {
        void root.revision
        return root.bridge.searchCount()
    }

    Timer {
        id: searchDebounce
        interval: 180
        onTriggered: root.bridge.startSearch(root.query, root.showHidden)
    }

    function beginDeepSearch() {
        if (root.query.length === 0)
            return
        root.deepSearch = true
        searchDebounce.restart()
    }

    function endDeepSearch() {
        root.deepSearch = false
        searchDebounce.stop()
        root.bridge.cancelSearch()
    }

    onQueryChanged: {
        if (root.query.length === 0)
            root.endDeepSearch()
        else if (root.deepSearch)
            searchDebounce.restart()
    }

    // Qt has no clipboard API for QML, so the copy goes through a text item
    // that is never shown.
    TextEdit {
        id: clipboardBuffer
        visible: false
    }

    function copyToClipboard(text) {
        clipboardBuffer.text = text
        clipboardBuffer.selectAll()
        clipboardBuffer.copy()
        clipboardBuffer.deselect()
    }

    function selectAll() {
        root.bridge.selectRows(root.rows)
    }

    // A menu item drawn like everything else here; the platform style is light
    // and would arrive as a white rectangle over a graphite window.
    component MenuLine: MenuItem {
        id: menuLine
        implicitHeight: 30
        contentItem: Text {
            leftPadding: 10
            rightPadding: 18
            text: menuLine.text
            color: menuLine.enabled ? root.ink : root.inkGhost
            font.family: root.sansFamily
            font.pixelSize: 13
            verticalAlignment: Text.AlignVCenter
        }
        background: Rectangle {
            color: menuLine.highlighted ? root.hover : "transparent"
            radius: 5
        }
    }

    component MenuSurface: Menu {
        id: menuSurface
        implicitWidth: 232
        padding: 5
        // Submenu titles are drawn by this delegate, not the platform style.
        delegate: MenuLine {}
        background: Rectangle {
            color: root.raisedHigh
            radius: 9
            border.width: 1
            border.color: root.line
        }
    }

    // Right-click on an entry. One instance serves both views: the row it acts
    // on is set just before it opens.
    MenuSurface {
        id: entryMenu
        property int row: -1
        onAboutToShow: {
            root.bridge.offerApps(entryMenu.row)
            root.bridge.offerActions()
            root.offerTick++
        }
        readonly property bool folder: {
            void root.revision
            return entryMenu.row >= 0 && root.bridge.entryIsDirectory(entryMenu.row)
        }
        readonly property int held: root.selectedCount
        readonly property bool archive: {
            void root.revision
            return entryMenu.row >= 0 && root.bridge.isArchive(entryMenu.row)
        }

        MenuLine {
            text: "Open in new tab"
            visible: entryMenu.folder
            height: visible ? implicitHeight : 0
            onTriggered: root.newTab(root.bridge.entryPath(entryMenu.row))
        }
        MenuLine {
            text: entryMenu.folder ? "Open" : "Open with default application"
            onTriggered: {
                if (entryMenu.folder)
                    root.bridge.navigateRow(entryMenu.row)
                else
                    root.bridge.openEntry(entryMenu.row)
            }
        }
        MenuSurface {
            id: openWithMenu
            title: "Open with"
            Repeater {
                model: {
                    void root.offerTick
                    return root.bridge.offeredCount()
                }
                delegate: MenuLine {
                    required property int index
                    text: {
                        void root.offerTick
                        return root.bridge.offeredName(index)
                               + (root.bridge.offeredIsDefault(index) ? "  (default)" : "")
                    }
                    onTriggered: root.bridge.launchOffered(index, false)
                }
            }
            MenuSeparator {
                contentItem: Rectangle { implicitHeight: 1; color: root.line }
            }
            MenuLine { text: "Other application…"; onTriggered: root.pickApp() }
        }
        MenuLine {
            text: "Copy path"
            onTriggered: root.copyToClipboard(root.bridge.entryPath(entryMenu.row))
        }
        MenuSeparator {
            contentItem: Rectangle { implicitHeight: 1; color: root.line }
        }
        MenuLine { text: "Cut"; onTriggered: root.bridge.offerSelected(true) }
        MenuLine { text: "Copy"; onTriggered: root.bridge.offerSelected(false) }
        MenuSeparator {
            contentItem: Rectangle { implicitHeight: 1; color: root.line }
        }
        MenuLine {
            text: entryMenu.held > 1 ? "Rename " + entryMenu.held + " items…" : "Rename…"
            onTriggered: entryMenu.held > 1 ? root.beginBulkRename() : root.beginRename(entryMenu.row)
        }
        MenuLine {
            text: entryMenu.held > 1 ? "Make " + entryMenu.held + " links" : "Make link"
            onTriggered: root.bridge.linkSelected(root.bridge.currentPath)
        }
        MenuSeparator {
            contentItem: Rectangle { implicitHeight: 1; color: root.line }
        }
        // Custom actions that apply to everything held.
        Repeater {
            model: {
                void root.offerTick
                return root.bridge.actionCount()
            }
            delegate: MenuLine {
                required property int index
                text: {
                    void root.offerTick
                    return root.bridge.actionName(index)
                }
                onTriggered: root.bridge.runAction(index)
            }
        }
        MenuLine {
            text: "Extract here"
            visible: entryMenu.archive
            height: visible ? implicitHeight : 0
            onTriggered: root.bridge.extractSelected()
        }
        MenuLine {
            text: "Compress…"
            onTriggered: root.beginCompress()
        }
        MenuLine {
            text: "Add bookmark"
            visible: entryMenu.folder && entryMenu.held <= 1
            height: visible ? implicitHeight : 0
            onTriggered: root.bridge.addBookmark(root.bridge.entryPath(entryMenu.row))
        }
        MenuLine {
            text: "Properties…"
            enabled: entryMenu.held <= 1
            onTriggered: root.showProperties(entryMenu.row)
        }
        MenuSeparator {
            contentItem: Rectangle { implicitHeight: 1; color: root.line }
        }
        MenuLine {
            text: entryMenu.held > 1 ? "Move " + entryMenu.held + " to Trash" : "Move to Trash"
            onTriggered: root.bridge.trashSelected()
        }
        MenuLine {
            text: entryMenu.held > 1 ? "Delete " + entryMenu.held + " permanently" : "Delete permanently"
            onTriggered: deleteConfirm.open()
        }
    }

    // Right-click on empty ground: about the folder, not about an entry.
    MenuSurface {
        id: groundMenu
        property bool pasteable: false
        property bool undoable: false
        property bool isDefault: true
        onAboutToShow: {
            pasteable = root.bridge.canPaste()
            undoable = root.bridge.canUndo()
            isDefault = root.bridge.isDefaultManager()
        }
        MenuLine {
            text: "Paste"
            enabled: groundMenu.pasteable
            onTriggered: root.bridge.pasteFiles()
        }
        MenuLine {
            text: "Undo"
            enabled: groundMenu.undoable
            onTriggered: root.bridge.undo()
        }
        MenuSeparator {
            contentItem: Rectangle { implicitHeight: 1; color: root.line }
        }
        MenuLine { text: "New folder…"; onTriggered: root.beginCreate("folder", "") }
        MenuLine { text: "New empty file…"; onTriggered: root.beginCreate("file", "") }
        MenuSurface {
            id: templateMenu
            title: "New from template"
            enabled: templateList.count > 0
            Repeater {
                id: templateList
                model: {
                    void root.revision
                    return root.bridge.templateCount()
                }
                delegate: MenuLine {
                    required property int index
                    text: root.bridge.templateName(index)
                    onTriggered: root.beginCreate("file", root.bridge.templatePath(index))
                }
            }
        }
        MenuSeparator {
            contentItem: Rectangle { implicitHeight: 1; color: root.line }
        }
        MenuLine { text: "Open terminal here"; onTriggered: root.bridge.openTerminalHere() }
        MenuLine { text: "Edit custom actions…"; onTriggered: root.bridge.editActions() }
        MenuLine {
            text: "Make Thallium Files the default file manager"
            visible: !groundMenu.isDefault
            height: visible ? implicitHeight : 0
            onTriggered: root.bridge.makeDefaultManager()
        }
        MenuLine { text: "Bookmark this folder"; onTriggered: root.bridge.addBookmark(root.bridge.currentPath) }
        MenuSeparator {
            contentItem: Rectangle { implicitHeight: 1; color: root.line }
        }
        MenuLine { text: "Select all"; onTriggered: root.selectAll() }
        MenuLine {
            text: root.showHidden ? "Hide hidden files" : "Show hidden files"
            onTriggered: root.showHidden = !root.showHidden
        }
        MenuSeparator {
            contentItem: Rectangle { implicitHeight: 1; color: root.line }
        }
        MenuLine { text: "Refresh"; onTriggered: root.bridge.refresh() }
    }

    MenuSurface {
        id: trashMenu
        MenuLine {
            text: "Restore"
            onTriggered: {
                root.bridge.restoreTrash(root.trashPicked)
                root.trashPicked = []
            }
        }
        MenuLine {
            text: "Delete permanently"
            enabled: !root.bridge.trashBusy()
            onTriggered: {
                root.bridge.purgeTrash(root.trashPicked)
                root.trashPicked = []
            }
        }
    }

    Popup {
        id: emptyConfirm
        anchors.centerIn: Overlay.overlay
        width: 380
        modal: true
        focus: true
        padding: 16
        onClosed: root.release()
        background: Rectangle {
            color: root.raisedHigh
            radius: 9
            border.width: 1
            border.color: root.line
        }
        Column {
            width: parent.width
            spacing: 12
            Text {
                text: "Empty the trash?"
                color: root.ink
                font.family: root.sansFamily
                font.pixelSize: 14
            }
            Text {
                text: "Everything in it is deleted permanently."
                color: root.inkMuted
                font.family: root.sansFamily
                font.pixelSize: 12
            }
            Row {
                anchors.right: parent.right
                spacing: 8
                Button { text: "Cancel"; onClicked: emptyConfirm.close() }
                Button {
                    text: "Empty Trash"
                    onClicked: {
                        root.bridge.emptyTrash()
                        root.trashPicked = []
                        emptyConfirm.close()
                    }
                }
            }
        }
    }

    // ---- Properties ----
    property int propTick: 0

    function showProperties(row) {
        if (row < 0)
            return
        root.bridge.inspect(row)
        root.propTick++
        propertiesSheet.open()
    }

    Shortcut {
        sequences: ["Alt+Return", "Alt+Enter"]
        enabled: !root.typing
        onActivated: root.showProperties(root.firstSelectedRow())
    }

    Popup {
        id: propertiesSheet
        anchors.centerIn: Overlay.overlay
        width: 440
        modal: true
        focus: true
        padding: 18
        onClosed: root.release()
        background: Rectangle {
            color: root.raisedHigh
            radius: 9
            border.width: 1
            border.color: root.line
        }
        function text(field) {
            void root.propTick
            void root.revision
            return root.bridge.propText(field)
        }
        function number(field) {
            void root.propTick
            void root.revision
            return root.bridge.propNumber(field)
        }
        readonly property int mode: number("mode")
        readonly property bool owned: number("owned") === 1

        Column {
            width: parent.width
            spacing: 12
            Text {
                width: parent.width
                text: propertiesSheet.text("name")
                elide: Text.ElideMiddle
                color: root.ink
                font.family: root.sansFamily
                font.pixelSize: 16
            }
            Grid {
                columns: 2
                columnSpacing: 14
                rowSpacing: 6
                Repeater {
                    model: [
                        ["Type", propertiesSheet.text("type")],
                        ["Size", propertiesSheet.number("directory") === 1
                            ? root.readableSize(propertiesSheet.number("size")) + ", "
                              + propertiesSheet.number("files")
                              + (propertiesSheet.number("files") === 1 ? " file" : " files")
                              + (propertiesSheet.number("measuring") === 1 ? " (counting…)" : "")
                            : root.readableSize(propertiesSheet.number("size"))],
                        ["Location", propertiesSheet.text("folder")],
                        ["Link to", propertiesSheet.text("link")],
                        ["Modified", root.modifiedText(propertiesSheet.number("modified"))],
                        ["Accessed", root.modifiedText(propertiesSheet.number("accessed"))],
                        ["Created", root.modifiedText(propertiesSheet.number("created"))],
                        ["Owner", propertiesSheet.text("owner") + " : " + propertiesSheet.text("group")],
                        ["Permissions", propertiesSheet.text("mode")]
                    ].filter(function (pair) { return pair[1].length > 0 })
                    delegate: Repeater {
                        required property var modelData
                        model: [modelData[0], modelData[1]]
                        delegate: Text {
                            required property var modelData
                            required property int index
                            width: index === 0 ? 90 : 300
                            text: modelData
                            wrapMode: Text.WrapAnywhere
                            color: index === 0 ? root.inkMuted : root.ink
                            font.family: index === 0 ? root.sansFamily : root.monoFamily
                            font.pixelSize: 12
                        }
                    }
                }
            }
            // Read, write and run for the owner, the group and everyone else,
            // applied as they are ticked.
            Grid {
                columns: 4
                columnSpacing: 12
                rowSpacing: 2
                enabled: propertiesSheet.owned
                Repeater {
                    model: ["", "Read", "Write", propertiesSheet.number("directory") === 1 ? "Enter" : "Run"]
                    delegate: Text {
                        required property var modelData
                        width: modelData.length === 0 ? 70 : 60
                        text: modelData
                        color: root.inkMuted
                        font.family: root.sansFamily
                        font.pixelSize: 11
                    }
                }
                Repeater {
                    model: [["Owner", 6], ["Group", 3], ["Others", 0]]
                    delegate: Repeater {
                        id: permRow
                        required property var modelData
                        readonly property string who: modelData[0]
                        readonly property int shift: modelData[1]
                        model: 4
                        delegate: Item {
                            id: permCell
                            required property int index
                            width: index === 0 ? 70 : 60
                            height: 26
                            readonly property int bit: index === 0 ? 0 : (4 >> (index - 1)) << permRow.shift
                            Text {
                                visible: permCell.index === 0
                                anchors.verticalCenter: parent.verticalCenter
                                text: permRow.who
                                color: root.ink
                                font.family: root.sansFamily
                                font.pixelSize: 12
                            }
                            CheckBox {
                                visible: permCell.index > 0
                                anchors.verticalCenter: parent.verticalCenter
                                checked: (propertiesSheet.mode & permCell.bit) !== 0
                                onToggled: root.bridge.setMode(checked ? propertiesSheet.mode | permCell.bit
                                                                         : propertiesSheet.mode & ~permCell.bit)
                            }
                        }
                    }
                }
            }
            Text {
                visible: !propertiesSheet.owned
                text: "Only the owner can change permissions."
                color: root.inkMuted
                font.family: root.sansFamily
                font.pixelSize: 11
            }
            Button {
                anchors.right: parent.right
                text: "Close"
                onClicked: propertiesSheet.close()
            }
        }
    }

    // ---- Drives ----
    MenuSurface {
        id: driveMenu
        property int index: -1
        readonly property bool mounted: {
            void root.revision
            return root.bridge.driveMount(driveMenu.index).length > 0
        }
        MenuLine { text: "Open"; onTriggered: root.bridge.openDrive(driveMenu.index) }
        MenuLine {
            text: "Unmount"
            enabled: driveMenu.mounted && !root.bridge.driveBusy()
            onTriggered: root.bridge.unmountDrive(driveMenu.index)
        }
        MenuLine {
            text: "Eject"
            enabled: root.bridge.driveEjectable(driveMenu.index) && !root.bridge.driveBusy()
            onTriggered: root.bridge.ejectDrive(driveMenu.index)
        }
    }

    // ---- Bookmarks ----
    MenuSurface {
        id: bookmarkMenu
        property int index: -1
        MenuLine {
            text: "Rename bookmark…"
            onTriggered: {
                namePrompt.mode = "bookmark"
                namePrompt.row = bookmarkMenu.index
                namePrompt.template = ""
                namePrompt.open()
                promptField.text = root.bridge.bookmarkLabel(bookmarkMenu.index)
                promptField.selectAll()
            }
        }
        MenuLine {
            text: "Remove bookmark"
            onTriggered: root.bridge.removeBookmark(bookmarkMenu.index)
        }
    }

    Shortcut {
        sequences: ["Ctrl+D"]
        enabled: !root.typing && !root.trashOpen && !root.recentOpen
        onActivated: root.bridge.addBookmark(root.bridge.currentPath)
    }

    // ---- Bulk rename ----
    property int bulkTick: 0

    function beginBulkRename() {
        bulkMode.currentIndex = 0
        bulkFirst.text = ""
        bulkSecond.text = ""
        bulkSheet.update()
        bulkSheet.open()
    }

    Popup {
        id: bulkSheet
        anchors.centerIn: Overlay.overlay
        width: 560
        height: 520
        modal: true
        focus: true
        padding: 16
        onOpened: bulkFirst.forceActiveFocus()
        onClosed: root.release()
        background: Rectangle {
            color: root.raisedHigh
            radius: 9
            border.width: 1
            border.color: root.line
        }
        readonly property var modes: [
            { key: "replace", label: "Replace text", first: "Find", second: "Replace with" },
            { key: "number", label: "Number", first: "Name pattern (### is the number)", second: "" },
            { key: "affix", label: "Add text", first: "Before the name", second: "After the name" },
            { key: "lower", label: "lowercase", first: "", second: "" },
            { key: "upper", label: "UPPERCASE", first: "", second: "" },
            { key: "title", label: "Title Case", first: "", second: "" }
        ]
        readonly property var mode: modes[Math.max(0, bulkMode.currentIndex)]
        readonly property string error: {
            void root.bulkTick
            return root.bridge.bulkError()
        }
        function update() {
            root.bridge.bulkRule(mode.key, bulkFirst.text, bulkSecond.text, bulkStart.value, bulkCase.checked)
            root.bulkTick++
        }

        Column {
            anchors.fill: parent
            spacing: 10
            Text {
                text: "Rename " + root.selectedCount + " items"
                color: root.ink
                font.family: root.sansFamily
                font.pixelSize: 14
            }
            ComboBox {
                id: bulkMode
                width: 220
                model: bulkSheet.modes.map(function (mode) { return mode.label })
                onCurrentIndexChanged: bulkSheet.update()
            }
            Row {
                spacing: 8
                visible: bulkSheet.mode.first.length > 0
                Rectangle {
                    width: bulkSheet.mode.second.length > 0 ? 256 : 380
                    height: 32
                    radius: 6
                    color: root.raised
                    border.width: 1
                    border.color: bulkFirst.activeFocus ? root.lineStrong : root.line
                    TextInput {
                        id: bulkFirst
                        anchors.fill: parent
                        anchors.leftMargin: 8
                        anchors.rightMargin: 8
                        verticalAlignment: TextInput.AlignVCenter
                        clip: true
                        color: root.ink
                        font.family: root.sansFamily
                        font.pixelSize: 13
                        onTextChanged: bulkSheet.update()
                        Keys.onEscapePressed: bulkSheet.close()
                        Text {
                            anchors.verticalCenter: parent.verticalCenter
                            visible: parent.text.length === 0
                            text: bulkSheet.mode.first
                            color: root.inkGhost
                            font: parent.font
                        }
                    }
                }
                Rectangle {
                    visible: bulkSheet.mode.second.length > 0
                    width: 256
                    height: 32
                    radius: 6
                    color: root.raised
                    border.width: 1
                    border.color: bulkSecond.activeFocus ? root.lineStrong : root.line
                    TextInput {
                        id: bulkSecond
                        anchors.fill: parent
                        anchors.leftMargin: 8
                        anchors.rightMargin: 8
                        verticalAlignment: TextInput.AlignVCenter
                        clip: true
                        color: root.ink
                        font.family: root.sansFamily
                        font.pixelSize: 13
                        onTextChanged: bulkSheet.update()
                        Keys.onEscapePressed: bulkSheet.close()
                        Text {
                            anchors.verticalCenter: parent.verticalCenter
                            visible: parent.text.length === 0
                            text: bulkSheet.mode.second
                            color: root.inkGhost
                            font: parent.font
                        }
                    }
                }
                SpinBox {
                    id: bulkStart
                    visible: bulkSheet.mode.key === "number"
                    from: 0
                    to: 999999
                    value: 1
                    onValueChanged: bulkSheet.update()
                }
            }
            CheckBox {
                id: bulkCase
                visible: bulkSheet.mode.key === "replace"
                text: "Ignore case"
                onCheckedChanged: bulkSheet.update()
            }
            ListView {
                width: parent.width
                height: parent.height - 220
                clip: true
                model: {
                    void root.bulkTick
                    return root.bridge.bulkCount()
                }
                delegate: Row {
                    required property int index
                    spacing: 10
                    Text {
                        width: 250
                        elide: Text.ElideMiddle
                        text: {
                            void root.bulkTick
                            return root.bridge.bulkOld(index)
                        }
                        color: root.inkMuted
                        font.family: root.monoFamily
                        font.pixelSize: 12
                    }
                    Text { text: "→"; color: root.inkGhost; font.pixelSize: 12 }
                    Text {
                        width: 250
                        elide: Text.ElideMiddle
                        text: {
                            void root.bulkTick
                            return root.bridge.bulkNew(index)
                        }
                        color: root.ink
                        font.family: root.monoFamily
                        font.pixelSize: 12
                    }
                }
            }
            Text {
                width: parent.width
                visible: bulkSheet.error.length > 0
                text: bulkSheet.error
                wrapMode: Text.Wrap
                color: root.destructive
                font.family: root.sansFamily
                font.pixelSize: 12
            }
            Row {
                anchors.right: parent.right
                spacing: 8
                Button { text: "Cancel"; onClicked: bulkSheet.close() }
                Button {
                    text: "Rename"
                    enabled: bulkSheet.error.length === 0
                    onClicked: {
                        root.bridge.bulkApply()
                        bulkSheet.close()
                    }
                }
            }
        }
    }

    // Bumped whenever the offered application list is re-read.
    property int offerTick: 0

    function pickApp() {
        root.bridge.offerAllApps()
        root.offerTick++
        appFilter.text = ""
        appDefault.checked = false
        appPicker.open()
    }

    // "Other application…": every installed application, filtered as you
    // type, with the choice of making it the default for the type.
    Popup {
        id: appPicker
        anchors.centerIn: Overlay.overlay
        width: 420
        height: 480
        modal: true
        focus: true
        padding: 16
        onOpened: appFilter.forceActiveFocus()
        onClosed: root.release()
        background: Rectangle {
            color: root.raisedHigh
            radius: 9
            border.width: 1
            border.color: root.line
        }
        readonly property var matches: {
            void root.offerTick
            const needle = appFilter.text.toLowerCase()
            const out = []
            for (let i = 0; i < root.bridge.offeredCount(); i++)
                if (root.bridge.offeredName(i).toLowerCase().indexOf(needle) >= 0)
                    out.push(i)
            return out
        }
        function choose(index) {
            root.bridge.launchOffered(index, appDefault.checked)
            close()
        }

        Column {
            anchors.fill: parent
            spacing: 10
            Text {
                text: "Open with"
                color: root.ink
                font.family: root.sansFamily
                font.pixelSize: 14
            }
            Rectangle {
                width: parent.width
                height: 32
                radius: 6
                color: root.raised
                border.width: 1
                border.color: appFilter.activeFocus ? root.lineStrong : root.line
                TextInput {
                    id: appFilter
                    anchors.fill: parent
                    anchors.leftMargin: 8
                    anchors.rightMargin: 8
                    verticalAlignment: TextInput.AlignVCenter
                    clip: true
                    color: root.ink
                    font.family: root.sansFamily
                    font.pixelSize: 13
                    Keys.onReturnPressed: if (appPicker.matches.length > 0) appPicker.choose(appPicker.matches[appList.currentIndex >= 0 ? appList.currentIndex : 0])
                    Keys.onEscapePressed: appPicker.close()
                    Keys.onDownPressed: appList.incrementCurrentIndex()
                    Keys.onUpPressed: appList.decrementCurrentIndex()
                }
            }
            ListView {
                id: appList
                width: parent.width
                height: parent.height - 32 - 24 - 40 - 30
                clip: true
                model: appPicker.matches
                currentIndex: 0
                delegate: Rectangle {
                    required property int index
                    required property var modelData
                    width: ListView.view.width
                    height: 30
                    radius: 5
                    color: ListView.isCurrentItem ? root.hover : "transparent"
                    Text {
                        anchors.left: parent.left
                        anchors.leftMargin: 8
                        anchors.verticalCenter: parent.verticalCenter
                        text: root.bridge.offeredName(modelData)
                        color: root.ink
                        font.family: root.sansFamily
                        font.pixelSize: 13
                    }
                    MouseArea {
                        anchors.fill: parent
                        onClicked: appList.currentIndex = index
                        onDoubleClicked: appPicker.choose(modelData)
                    }
                }
            }
            CheckBox {
                id: appDefault
                text: "Always use for " + root.bridge.offeredType()
            }
        }
    }

    // One prompt names things: a rename, a new folder, a new file. `mode` says
    // which; `row` is the entry being renamed and `template` the file a new
    // one copies, empty for an empty file.
    function beginRename(row) {
        if (row < 0)
            return
        namePrompt.mode = "rename"
        namePrompt.row = row
        namePrompt.template = ""
        namePrompt.open()
        promptField.text = root.bridge.entryName(row)
        const dot = promptField.text.lastIndexOf(".")
        promptField.select(0, root.bridge.entryIsDirectory(row) || dot <= 0 ? promptField.text.length : dot)
    }

    // Compress names the archive after the one entry, or after the folder
    // when several are held; the suffix picks the format.
    function beginCompress() {
        namePrompt.mode = "compress"
        namePrompt.row = -1
        namePrompt.template = ""
        namePrompt.open()
        const base = root.selectedCount === 1
            ? root.bridge.entryName(root.firstSelectedRow())
            : root.leafName(root.bridge.currentPath)
        promptField.text = root.bridge.freeName(base + ".zip")
        promptField.select(0, promptField.text.length - 4)
    }

    function beginCreate(kind, template) {
        namePrompt.mode = kind
        namePrompt.row = -1
        namePrompt.template = template
        namePrompt.open()
        const base = kind === "folder" ? "New Folder"
            : template.length > 0 ? template.substring(template.lastIndexOf("/") + 1) : "Untitled"
        promptField.text = root.bridge.freeName(base)
        const dot = promptField.text.lastIndexOf(".")
        promptField.select(0, kind === "folder" || dot <= 0 ? promptField.text.length : dot)
    }

    // A typed or handed-over location: `trash:` opens the trash, anything
    // else is a path or file URI.
    function openTarget(target) {
        if (target.startsWith("trash:")) {
            root.showTrash()
            return
        }
        root.trashOpen = false
        root.recentOpen = false
        root.bridge.reveal(target)
    }

    function editPath() {
        pathEditor.text = root.bridge.currentPath
        pathEditor.visible = true
        pathEditor.forceActiveFocus()
        pathEditor.selectAll()
    }

    function firstSelectedRow() {
        for (let row = 0; row < root.bridge.entryCount(); row++)
            if (root.bridge.entrySelected(row))
                return row
        return -1
    }

    Popup {
        id: namePrompt
        property string mode: "rename"
        property int row: -1
        property string template: ""
        anchors.centerIn: Overlay.overlay
        width: 380
        modal: true
        focus: true
        padding: 16
        onOpened: promptField.forceActiveFocus()
        onClosed: root.release()
        background: Rectangle {
            color: root.raisedHigh
            radius: 9
            border.width: 1
            border.color: root.line
        }

        function accept() {
            const name = promptField.text.trim()
            if (name.length === 0)
                return
            if (mode === "rename")
                root.bridge.renameRow(row, name)
            else if (mode === "bookmark")
                root.bridge.renameBookmark(row, name)
            else if (mode === "compress")
                root.bridge.compressSelected(name)
            else if (mode === "folder")
                root.bridge.createFolder(name)
            else
                root.bridge.createFile(name, template)
            close()
        }

        Column {
            width: parent.width
            spacing: 10
            Text {
                text: namePrompt.mode === "rename" ? "Rename"
                    : namePrompt.mode === "bookmark" ? "Bookmark name"
                    : namePrompt.mode === "compress" ? "Compress to (.zip, .tar.gz, .tar.xz, .tar.zst)"
                    : namePrompt.mode === "folder" ? "New folder" : "New file"
                color: root.ink
                font.family: root.sansFamily
                font.pixelSize: 14
            }
            Rectangle {
                width: parent.width
                height: 32
                radius: 6
                color: root.raised
                border.width: 1
                border.color: promptField.activeFocus ? root.lineStrong : root.line
                TextInput {
                    id: promptField
                    anchors.fill: parent
                    anchors.leftMargin: 8
                    anchors.rightMargin: 8
                    verticalAlignment: TextInput.AlignVCenter
                    clip: true
                    color: root.ink
                    selectionColor: root.spectralDim
                    selectedTextColor: root.ink
                    font.family: root.sansFamily
                    font.pixelSize: 13
                    Keys.onReturnPressed: namePrompt.accept()
                    Keys.onEnterPressed: namePrompt.accept()
                    Keys.onEscapePressed: namePrompt.close()
                }
            }
            Text {
                visible: promptField.text.indexOf("/") >= 0
                text: "A name cannot contain “/”."
                color: root.inkMuted
                font.family: root.sansFamily
                font.pixelSize: 11
            }
        }
    }

    Shortcut {
        sequence: "F2"
        enabled: !namePrompt.opened
        onActivated: root.selectedCount > 1 ? root.beginBulkRename() : root.beginRename(root.firstSelectedRow())
    }

    // ---- Drag and drop ----
    // Pressing an entry that is already held and moving drags the held files,
    // as a system drag, so the same gesture works within the window, between
    // windows, and into other applications. Pressing anything else sweeps.
    property bool dragArmed: false
    property real dragOriginX: 0
    property real dragOriginY: 0

    function armDrag(row) {
        root.dragArmed = root.bridge.entrySelected(row)
        root.dragOriginX = -1
        return root.dragArmed
    }

    function followDrag(x, y) {
        if (root.dragOriginX < 0) {
            root.dragOriginX = x
            root.dragOriginY = y
            return
        }
        if (Math.abs(x - root.dragOriginX) < 8 && Math.abs(y - root.dragOriginY) < 8)
            return
        root.dragArmed = false
        const uris = root.bridge.selectedUris()
        if (uris.length === 0)
            return
        fileDrag.uris = uris.split("\n").join("\r\n") + "\r\n"
        fileDrag.Drag.active = true
    }

    Item {
        id: fileDrag
        property string uris: ""
        Drag.dragType: Drag.Automatic
        Drag.supportedActions: Qt.CopyAction | Qt.MoveAction | Qt.LinkAction
        Drag.proposedAction: Qt.MoveAction
        Drag.mimeData: ({ "text/uri-list": fileDrag.uris })
        Drag.onDragFinished: fileDrag.Drag.active = false
    }

    // Ctrl copies, Ctrl+Shift links, and a plain drop moves within one
    // filesystem and copies across them. The source is always told it was a
    // copy, so another application never deletes what was moved here.
    function dropInto(drop, path) {
        if (!drop.hasUrls || path.length === 0)
            return
        const uris = []
        for (let i = 0; i < drop.urls.length; i++)
            uris.push(String(drop.urls[i]))
        const action = drop.proposedAction === Qt.CopyAction ? 1
                     : drop.proposedAction === Qt.LinkAction ? 4 : 2
        root.bridge.dropUris(uris.join("\n"), path, action)
        drop.accept(Qt.CopyAction)
    }

    // Double-click or Enter on one entry: folders open here, files open in
    // their default application.
    function activateRow(row) {
        if (root.bridge.entryIsDirectory(row))
            root.bridge.navigateRow(row)
        else
            root.bridge.openEntry(row)
    }

    // ---- Keyboard ----
    // Shortcuts fire unless a text field claims the key first, so arrows and
    // Backspace keep editing inside the search box.
    property int anchorRow: -1

    function displayIndex(row) { return root.rows.indexOf(row) }

    function columns() {
        return root.view === "grid"
            ? Math.max(1, Math.floor(gridBrowser.width / gridBrowser.cellWidth)) : 1
    }

    // Move the focus `delta` places in display order. With `extend` the
    // selection runs from the anchor to the new focus, as Shift does.
    function moveFocus(delta, extend) {
        const count = root.rows.length
        if (count === 0)
            return
        const at = root.displayIndex(root.focusRow)
        const next = at < 0 ? (delta > 0 ? 0 : count - 1) : Math.max(0, Math.min(count - 1, at + delta))
        root.focusOn(next, extend)
    }

    function focusOn(index, extend) {
        const row = root.rows[index]
        if (!extend || root.anchorRow < 0)
            root.anchorRow = row
        root.focusRow = row
        if (extend) {
            const from = root.displayIndex(root.anchorRow)
            const lo = Math.min(from, index), hi = Math.max(from, index)
            root.bridge.selectRows(root.rows.slice(lo, hi + 1))
        } else {
            root.bridge.selectRows([row])
        }
        if (root.view === "grid")
            gridBrowser.positionViewAtIndex(index, GridView.Contain)
        else
            listBrowser.positionViewAtIndex(index, ListView.Contain)
    }

    function pageSize() {
        const view = root.view === "grid" ? gridBrowser : listBrowser
        const pitch = root.view === "grid" ? gridBrowser.cellHeight : root.rowHeight
        return Math.max(1, Math.floor(view.height / pitch)) * root.columns()
    }

    // Enter opens what is selected: a single folder is entered, files open
    // in their default applications.
    function activateSelection() {
        const row = root.selectedCount === 1 ? root.firstSelectedRow() : root.focusRow
        if (root.selectedCount <= 1) {
            if (row < 0)
                return
            if (root.bridge.entryIsDirectory(row))
                root.bridge.navigateRow(row)
            else
                root.bridge.openEntry(row)
            return
        }
        for (let i = 0; i < root.rows.length; i++) {
            const each = root.rows[i]
            if (root.bridge.entrySelected(each) && !root.bridge.entryIsDirectory(each))
                root.bridge.openEntry(each)
        }
    }

    // Typing a name jumps to the first entry that starts with it; a pause
    // starts the next search afresh.
    property string typed: ""
    Timer {
        id: typeAhead
        interval: 900
        onTriggered: root.typed = ""
    }

    function findTyped(text) {
        root.typed += text.toLowerCase()
        typeAhead.restart()
        for (let i = 0; i < root.rows.length; i++)
            if (root.bridge.entryName(root.rows[i]).toLowerCase().startsWith(root.typed)) {
                root.focusOn(i, false)
                return
            }
    }

    // Holds the keyboard whenever no text field does, so typing reaches
    // the type-ahead.
    Item {
        id: keyCatcher
        focus: true
        Keys.onPressed: function (event) {
            const plain = (event.modifiers & ~Qt.ShiftModifier) === 0
            if (plain && event.text.length === 1 && event.text >= " " && event.text !== "\u007f") {
                root.findTyped(event.text)
                event.accepted = true
            }
        }
    }

    function release() { keyCatcher.forceActiveFocus() }

    readonly property bool typing: namePrompt.opened || pathEditor.visible || deleteConfirm.opened
                                   || emptyConfirm.opened || appPicker.opened || propertiesSheet.opened
                                   || bulkSheet.opened

    Shortcut { sequences: ["Down"]; enabled: !root.typing; onActivated: root.moveFocus(root.columns(), false) }
    Shortcut { sequences: ["Up"]; enabled: !root.typing; onActivated: root.moveFocus(-root.columns(), false) }
    Shortcut { sequences: ["Shift+Down"]; enabled: !root.typing; onActivated: root.moveFocus(root.columns(), true) }
    Shortcut { sequences: ["Shift+Up"]; enabled: !root.typing; onActivated: root.moveFocus(-root.columns(), true) }
    Shortcut { sequences: ["Right"]; enabled: !root.typing && root.view === "grid"; onActivated: root.moveFocus(1, false) }
    Shortcut { sequences: ["Left"]; enabled: !root.typing && root.view === "grid"; onActivated: root.moveFocus(-1, false) }
    Shortcut { sequences: ["Shift+Right"]; enabled: !root.typing && root.view === "grid"; onActivated: root.moveFocus(1, true) }
    Shortcut { sequences: ["Shift+Left"]; enabled: !root.typing && root.view === "grid"; onActivated: root.moveFocus(-1, true) }
    Shortcut { sequences: ["Home"]; enabled: !root.typing; onActivated: if (root.rows.length > 0) root.focusOn(0, false) }
    Shortcut { sequences: ["End"]; enabled: !root.typing; onActivated: if (root.rows.length > 0) root.focusOn(root.rows.length - 1, false) }
    Shortcut { sequences: ["Shift+Home"]; enabled: !root.typing; onActivated: if (root.rows.length > 0) root.focusOn(0, true) }
    Shortcut { sequences: ["Shift+End"]; enabled: !root.typing; onActivated: if (root.rows.length > 0) root.focusOn(root.rows.length - 1, true) }
    Shortcut { sequences: ["PgDown"]; enabled: !root.typing; onActivated: root.moveFocus(root.pageSize(), false) }
    Shortcut { sequences: ["PgUp"]; enabled: !root.typing; onActivated: root.moveFocus(-root.pageSize(), false) }
    Shortcut { sequences: ["Return", "Enter"]; enabled: !root.typing && root.query.length === 0; onActivated: root.activateSelection() }
    Shortcut { sequences: ["Backspace", "Alt+Up"]; enabled: !root.typing; onActivated: root.bridge.up() }
    Shortcut { sequences: ["Alt+Left", "Back"]; enabled: !root.typing; onActivated: root.bridge.back() }
    Shortcut { sequences: ["Alt+Right", "Forward"]; enabled: !root.typing; onActivated: root.bridge.forward() }
    Shortcut { sequences: ["F5", "Ctrl+R"]; enabled: !root.typing; onActivated: root.bridge.refresh() }
    Shortcut { sequences: [StandardKey.SelectAll]; enabled: !root.typing; onActivated: root.selectAll() }
    Shortcut { sequences: ["Ctrl+Space"]; enabled: !root.typing && root.focusRow >= 0; onActivated: root.bridge.toggleSelection(root.focusRow) }
    Shortcut {
        sequences: ["Escape"]
        enabled: !root.typing
        onActivated: {
            root.bridge.clearSelection()
            root.typed = ""
            root.release()
        }
    }
    Shortcut { sequences: ["Delete"]; enabled: !root.typing && root.selectedCount > 0; onActivated: root.bridge.trashSelected() }
    Shortcut { sequences: ["Shift+Delete"]; enabled: !root.typing && root.selectedCount > 0; onActivated: deleteConfirm.open() }
    Shortcut { sequences: ["Ctrl+L", "F6"]; enabled: !root.typing; onActivated: root.editPath() }
    Shortcut { sequences: [StandardKey.Find]; enabled: !root.typing; onActivated: searchInput.forceActiveFocus() }
    // The context menu from the keyboard: for the held entry, or for the
    // folder when nothing is held.
    Shortcut {
        sequences: ["Menu", "Shift+F10"]
        enabled: !root.typing
        onActivated: {
            const row = root.focusRow >= 0 && root.bridge.entrySelected(root.focusRow)
                ? root.focusRow : root.firstSelectedRow()
            if (row >= 0) {
                entryMenu.row = row
                entryMenu.popup(root.contentItem, root.width / 3, root.height / 3)
            } else {
                groundMenu.popup(root.contentItem, root.width / 3, root.height / 3)
            }
        }
    }
    Shortcut { sequences: [StandardKey.Undo]; enabled: !root.typing; onActivated: root.bridge.undo() }
    Shortcut { sequences: ["Ctrl+1"]; onActivated: root.view = "list" }
    Shortcut { sequences: ["Ctrl+2"]; onActivated: root.view = "grid" }

    // Permanent deletion cannot be undone, so it always asks.
    Popup {
        id: deleteConfirm
        anchors.centerIn: Overlay.overlay
        width: 380
        modal: true
        focus: true
        padding: 16
        onOpened: deleteYes.forceActiveFocus()
        onClosed: root.release()
        background: Rectangle {
            color: root.raisedHigh
            radius: 9
            border.width: 1
            border.color: root.line
        }
        Column {
            width: parent.width
            spacing: 12
            Text {
                width: parent.width
                wrapMode: Text.Wrap
                text: root.selectedCount === 1
                    ? "Permanently delete “" + root.bridge.entryName(root.firstSelectedRow()) + "”?"
                    : "Permanently delete " + root.selectedCount + " items?"
                color: root.ink
                font.family: root.sansFamily
                font.pixelSize: 14
            }
            Text {
                text: "This cannot be undone."
                color: root.inkMuted
                font.family: root.sansFamily
                font.pixelSize: 12
            }
            Row {
                anchors.right: parent.right
                spacing: 8
                Button {
                    text: "Cancel"
                    onClicked: deleteConfirm.close()
                    Keys.onEscapePressed: deleteConfirm.close()
                }
                Button {
                    id: deleteYes
                    text: "Delete"
                    onClicked: {
                        root.bridge.deleteSelected()
                        deleteConfirm.close()
                    }
                    Keys.onReturnPressed: clicked()
                    Keys.onEnterPressed: clicked()
                    Keys.onEscapePressed: deleteConfirm.close()
                }
            }
        }
    }

    // Text fields claim these keys first, so copying from the search box
    // still copies text. The keys are spelled out because StandardKey.Cut
    // also holds Shift+Delete, which here deletes.
    Shortcut {
        sequences: ["Ctrl+X"]
        enabled: !root.typing
        onActivated: root.bridge.offerSelected(true)
    }

    Shortcut {
        sequences: ["Ctrl+C", "Ctrl+Insert"]
        enabled: !root.typing
        onActivated: root.bridge.offerSelected(false)
    }

    Shortcut {
        sequences: ["Ctrl+V", "Shift+Insert"]
        enabled: !root.typing
        onActivated: root.bridge.pasteFiles()
    }

    Shortcut {
        sequence: "Ctrl+Shift+N"
        enabled: !namePrompt.opened
        onActivated: root.beginCreate("folder", "")
    }

    // Opening the menu for a row that is not part of the selection makes that
    // row the selection first, so the actions always name what is on screen.
    function openEntryMenu(row) {
        if (!root.bridge.entrySelected(row))
            root.pick(row, false)
        entryMenu.row = row
        entryMenu.popup()
    }


    Timer {
        interval: 100
        repeat: true
        running: true
        // Background tabs are polled too, so their queues keep draining.
        onTriggered: {
            for (let i = 0; i < root.tabs.length; i++)
                root.tabs[i].poll()
        }
    }

    // Remember what home holds, so the places list stays truthful.
    Connections {
        target: root.bridge
        function onCurrentPathChanged() {
            root.focusRow = -1
            root.trashOpen = false
            if (root.bridge.currentPath !== root.bridge.homePath)
                return
            const found = []
            const total = root.bridge.entryCount()
            for (let i = 0; i < total; ++i) {
                if (root.bridge.entryIsDirectory(i))
                    found.push(root.bridge.entryName(i))
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

    // A directory entry's face. The icon comes from the desktop's icon theme,
    // so a Rust file looks like a Rust file here and in every other
    // application; once a thumbnail has been generated it takes the icon's
    // place. Symbolic icons ship as black glyphs and are recoloured to the
    // palette; a full-colour icon is left as its designer drew it. If no
    // installed theme answers for the type at all, the typographic mark that
    // preceded icons is still there underneath.
    component EntryIcon: Item {
        id: entryIcon

        property int entry: -1
        property string kind: "Other"
        property color tint: root.markTone(entryIcon.kind)
        property int glyphSize: 16
        property int markSize: 12
        property bool allowThumbnail: true

        readonly property string iconSource: {
            void root.revision
            return entryIcon.entry >= 0 ? root.bridge.entryIcon(entryIcon.entry) : ""
        }
        readonly property string thumbnailSource: {
            void root.revision
            return entryIcon.allowThumbnail && entryIcon.entry >= 0
                   ? root.bridge.entryThumbnail(entryIcon.entry)
                   : ""
        }
        readonly property bool symbolic: entryIcon.iconSource.indexOf("symbolic") >= 0
        readonly property bool thumbnailShown: thumbnail.status === Image.Ready
        readonly property bool iconShown: !entryIcon.thumbnailShown && icon.status === Image.Ready

        Image {
            id: thumbnail
            anchors.fill: parent
            visible: entryIcon.thumbnailShown
            source: entryIcon.thumbnailSource
            fillMode: Image.PreserveAspectCrop
            sourceSize.width: Math.max(2, Math.round(entryIcon.width))
            sourceSize.height: Math.max(2, Math.round(entryIcon.height))
            asynchronous: true
            smooth: true
            mipmap: true
        }

        Image {
            id: icon
            anchors.centerIn: parent
            width: entryIcon.glyphSize
            height: entryIcon.glyphSize
            // Kept off-screen when it needs tinting: the effect below draws it.
            visible: entryIcon.iconShown && !entryIcon.symbolic
            source: entryIcon.iconSource
            sourceSize.width: entryIcon.glyphSize * 2
            sourceSize.height: entryIcon.glyphSize * 2
            asynchronous: true
            smooth: true
        }

        MultiEffect {
            anchors.fill: icon
            source: icon
            visible: entryIcon.iconShown && entryIcon.symbolic
            brightness: 1.0
            colorization: 1.0
            colorizationColor: entryIcon.tint
        }

        Text {
            anchors.centerIn: parent
            visible: !entryIcon.thumbnailShown && !entryIcon.iconShown
            text: root.markFor(entryIcon.kind)
            color: entryIcon.tint
            font.family: entryIcon.kind === "Code" ? root.monoFamily : root.sansFamily
            font.pixelSize: entryIcon.markSize
        }
    }

    // A bordered single-line input, used by the remote sheet.
    component Field: Rectangle {
        id: field
        property alias text: fieldInput.text
        property string placeholder: ""
        property bool masked: false
        signal accepted

        implicitHeight: 32
        radius: 7
        color: root.inset
        border.width: 1
        border.color: fieldInput.activeFocus ? root.spectral : root.line

        TextInput {
            id: fieldInput
            anchors.left: parent.left
            anchors.right: parent.right
            anchors.leftMargin: 10
            anchors.rightMargin: 10
            anchors.verticalCenter: parent.verticalCenter
            clip: true
            color: root.ink
            selectionColor: root.emberWash
            selectedTextColor: root.ink
            font.family: field.masked ? root.monoFamily : root.sansFamily
            font.pixelSize: 13
            echoMode: field.masked ? TextInput.Password : TextInput.Normal
            onAccepted: field.accepted()

            Text {
                anchors.verticalCenter: parent.verticalCenter
                visible: fieldInput.text.length === 0
                text: field.placeholder
                color: root.inkGhost
                font.family: root.sansFamily
                font.pixelSize: 13
            }
        }
    }

    // A text button for the remote sheet.
    component SheetButton: Rectangle {
        id: sheetButton
        property string label: ""
        property bool primary: false
        property bool enabled: true
        signal activated

        implicitWidth: buttonLabel.implicitWidth + 26
        implicitHeight: 30
        radius: 7
        color: !sheetButton.enabled
               ? "transparent"
               : (sheetButton.primary
                  ? (buttonMouse.containsMouse ? root.hover : root.raisedHigh)
                  : (buttonMouse.containsMouse ? root.hover : "transparent"))
        border.width: 1
        border.color: !sheetButton.enabled
                      ? root.lineSoft
                      : (sheetButton.primary ? root.spectral : root.line)

        Text {
            id: buttonLabel
            anchors.centerIn: parent
            text: sheetButton.label
            color: !sheetButton.enabled
                   ? root.inkGhost
                   : (sheetButton.primary ? root.spectral : root.inkMuted)
            font.family: root.sansFamily
            font.pixelSize: 13
        }

        MouseArea {
            id: buttonMouse
            anchors.fill: parent
            hoverEnabled: true
            cursorShape: sheetButton.enabled ? Qt.PointingHandCursor : Qt.ArrowCursor
            onClicked: if (sheetButton.enabled) sheetButton.activated()
        }
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
        font.pixelSize: 11
        font.weight: Font.DemiBold
        font.letterSpacing: 0.9
    }

    component Mono: Text {
        color: root.inkFaint
        font.family: root.monoFamily
        font.pixelSize: 13
        elide: Text.ElideRight
        verticalAlignment: Text.AlignVCenter
    }

    component IconAction: Rectangle {
        id: iconAction
        property string glyph: ""
        property string hint: ""
        property bool enabled: true
        property bool active: false
        signal activated

        implicitWidth: 30
        implicitHeight: 30
        radius: 7
        color: iconAction.active
               ? root.raisedHigh
               : (mouse.containsMouse && iconAction.enabled ? root.hover : "transparent")
        border.width: iconAction.active ? 1 : 0
        border.color: root.lineStrong

        Text {
            anchors.centerIn: parent
            text: iconAction.glyph
            color: !iconAction.enabled
                   ? root.inkGhost
                   : (iconAction.active
                      ? root.ink
                      : (mouse.containsMouse ? root.ink : root.inkMuted))
            font.family: root.sansFamily
            font.pixelSize: 15
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

    component Tick: Rectangle {
        id: tick
        property bool checked: false
        signal toggled

        implicitWidth: 15
        implicitHeight: 15
        radius: 4
        color: tick.checked ? root.ember : "transparent"
        border.width: 1
        border.color: tick.checked
                      ? root.ember
                      : (tickMouse.containsMouse ? root.lineStrong : root.line)

        Text {
            anchors.centerIn: parent
            text: "✓"
            visible: tick.checked
            color: root.inset
            font.family: root.sansFamily
            font.pixelSize: 10
            font.weight: Font.Bold
        }

        MouseArea {
            id: tickMouse
            anchors.fill: parent
            hoverEnabled: true
            cursorShape: Qt.PointingHandCursor
            onClicked: tick.toggled()
        }
    }

    // A dotted rule. Nothing OS draws its structure in dots rather than
    // strokes, and a dotted line reads as a seam instead of a border.
    component DashRule: Row {
        property int dotSize: 1
        property color tone: root.lineStrong
        spacing: 3
        clip: true

        Repeater {
            model: Math.max(0, Math.floor(parent.width / 4))
            delegate: Rectangle {
                width: 1
                height: 1
                radius: 0
                color: root.lineStrong
            }
        }
    }

    // One half of the sidebar's segmented control.
    component Segment: Rectangle {
        id: segment
        property string label: ""
        property string key: ""

        readonly property bool current: root.sidebarTab === segment.key
        implicitHeight: 26
        radius: 6
        color: segment.current ? root.raisedHigh : "transparent"
        border.width: segment.current ? 1 : 0
        border.color: root.line

        Text {
            anchors.centerIn: parent
            text: segment.label
            color: segment.current ? root.ink : root.inkFaint
            font.family: root.sansFamily
            font.pixelSize: 12
            font.weight: segment.current ? Font.DemiBold : Font.Normal
        }

        MouseArea {
            anchors.fill: parent
            cursorShape: Qt.PointingHandCursor
            onClicked: root.sidebarTab = segment.key
        }
    }

    // A row of the folder tree: indent guides, folder mark, name, count.
    component TreeRow: Rectangle {
        id: treeRow
        property string label: ""
        property string badge: ""
        property int depth: 0
        property bool current: false
        signal activated

        readonly property int step: 14

        width: parent ? parent.width : 0
        implicitHeight: 30
        radius: 7
        color: treeRow.current
               ? root.raisedHigh
               : (treeMouse.containsMouse ? root.raised : "transparent")
        border.width: treeRow.current ? 1 : 0
        border.color: root.line

        // The guides that tie a child back to its parent.
        Repeater {
            model: treeRow.depth

            delegate: Item {
                required property int index
                readonly property bool elbow: index === treeRow.depth - 1
                anchors.top: parent.top
                anchors.bottom: parent.bottom
                x: 12 + index * treeRow.step
                width: treeRow.step

                Rectangle {
                    anchors.top: parent.top
                    height: parent.elbow ? parent.height / 2 : parent.height
                    width: 1
                    color: root.line
                }

                Rectangle {
                    visible: parent.elbow
                    anchors.verticalCenter: parent.verticalCenter
                    width: 7
                    height: 1
                    color: root.line
                }
            }
        }

        Text {
            id: treeGlyph
            x: 12 + treeRow.depth * treeRow.step + (treeRow.depth > 0 ? 9 : 0)
            anchors.verticalCenter: parent.verticalCenter
            width: 15
            text: treeRow.current ? "▼" : "▶"
            horizontalAlignment: Text.AlignHCenter
            color: treeRow.current ? root.spectral : root.inkGhost
            font.family: root.sansFamily
            font.pixelSize: 9
        }

        Text {
            anchors.left: treeGlyph.right
            anchors.leftMargin: 7
            anchors.right: treeBadge.left
            anchors.rightMargin: 8
            anchors.verticalCenter: parent.verticalCenter
            text: treeRow.label
            elide: Text.ElideMiddle
            color: treeRow.current ? root.ink : root.inkMuted
            font.family: root.sansFamily
            font.pixelSize: 14
        }

        Rectangle {
            id: treeBadge
            anchors.right: parent.right
            anchors.rightMargin: 8
            anchors.verticalCenter: parent.verticalCenter
            visible: treeRow.badge.length > 0
            width: badgeText.implicitWidth + 12
            height: 18
            radius: 5
            color: root.raisedHigh

            Text {
                id: badgeText
                anchors.centerIn: parent
                text: treeRow.badge
                color: root.inkFaint
                font.family: root.monoFamily
                font.pixelSize: 10
            }
        }

        MouseArea {
            id: treeMouse
            anchors.fill: parent
            hoverEnabled: true
            cursorShape: Qt.PointingHandCursor
            onClicked: treeRow.activated()
        }
    }

    // A labelled action in the preview panel's foot.
    component PanelAction: Item {
        id: panelAction
        property string glyph: ""
        property string label: ""
        property bool danger: false
        signal activated

        implicitHeight: 42

        Column {
            anchors.centerIn: parent
            spacing: 4

            Text {
                anchors.horizontalCenter: parent.horizontalCenter
                text: panelAction.glyph
                color: panelAction.danger
                       ? root.destructive
                       : (panelMouse.containsMouse ? root.ink : root.inkMuted)
                font.family: root.sansFamily
                font.pixelSize: 14
            }

            Text {
                anchors.horizontalCenter: parent.horizontalCenter
                text: panelAction.label
                color: panelAction.danger
                       ? root.destructive
                       : (panelMouse.containsMouse ? root.inkMuted : root.inkFaint)
                font.family: root.sansFamily
                font.pixelSize: 10
            }
        }

        MouseArea {
            id: panelMouse
            anchors.fill: parent
            hoverEnabled: true
            cursorShape: Qt.PointingHandCursor
            onClicked: panelAction.activated()
        }
    }


    // A sidebar destination: glyph, label, trailing count.
    component NavItem: Rectangle {
        id: navItem
        property string glyph: ""
        property string icon: ""
        property string label: ""
        property string trailing: ""
        property bool active: false
        signal activated
        signal menuRequested
        // A folder this item names, so files can be dropped on it.
        property string dropTarget: ""

        width: parent ? parent.width : 0
        implicitHeight: 34
        radius: Theme.radiusSmall
        color: navItem.active
               ? Theme.selected
               : (navMouse.containsMouse ? Theme.surfaceHigh : "transparent")
        border.width: navItem.active ? 1 : 0
        border.color: Theme.outline

        Item {
            id: navGlyph
            anchors.left: parent.left
            anchors.leftMargin: 10
            anchors.verticalCenter: parent.verticalCenter
            width: 16
            height: 16
            Icon {
                visible: navItem.icon.length > 0
                anchors.centerIn: parent
                name: navItem.icon.length > 0 ? navItem.icon : "folder"
                size: 16
                shade: navItem.active ? 1 : 0.55
            }
            Glyph {
                visible: navItem.icon.length === 0
                anchors.centerIn: parent
                text: navItem.glyph
                font.pixelSize: 13
                color: navItem.active ? root.ink : root.inkFaint
            }
        }

        Text {
            anchors.left: navGlyph.right
            anchors.leftMargin: 9
            anchors.right: navCount.left
            anchors.rightMargin: 8
            anchors.verticalCenter: parent.verticalCenter
            text: navItem.label
            elide: Text.ElideRight
            color: navItem.active ? Theme.ink : Theme.inkSoft
            font.family: Theme.family
            font.pixelSize: Theme.text
        }

        Mono {
            id: navCount
            anchors.right: parent.right
            anchors.rightMargin: 10
            anchors.verticalCenter: parent.verticalCenter
            text: navItem.trailing
            font.pixelSize: 10
            color: root.inkGhost
        }

        MouseArea {
            id: navMouse
            anchors.fill: parent
            hoverEnabled: true
            cursorShape: Qt.PointingHandCursor
            acceptedButtons: Qt.LeftButton | Qt.RightButton
            onClicked: function (mouse) {
                if (mouse.button === Qt.RightButton)
                    navItem.menuRequested()
                else
                    navItem.activated()
            }
        }

        DropArea {
            anchors.fill: parent
            enabled: navItem.dropTarget.length > 0
            onDropped: function (drop) { root.dropInto(drop, navItem.dropTarget) }
            Rectangle {
                anchors.fill: parent
                visible: parent.containsDrag
                color: "transparent"
                radius: 7
                border.width: 1
                border.color: root.spectral
            }
        }
    }

    // A clickable column head that carries the sort direction.
    component SortHead: Item {
        id: sortHead
        property string label: ""
        property string key: ""
        property bool numeric: false

        implicitHeight: 16

        Row {
            anchors.left: sortHead.numeric ? undefined : parent.left
            anchors.right: sortHead.numeric ? parent.right : undefined
            anchors.verticalCenter: parent.verticalCenter
            spacing: 4

            Overline {
                text: sortHead.label.toUpperCase()
                color: root.sortKey === sortHead.key ? root.inkMuted : root.inkGhost
                anchors.verticalCenter: parent.verticalCenter
            }

            Text {
                visible: root.sortKey === sortHead.key
                text: root.sortAscending ? "▲" : "▼"
                color: root.spectral
                font.pixelSize: 6
                anchors.verticalCenter: parent.verticalCenter
            }
        }

        MouseArea {
            anchors.fill: parent
            cursorShape: Qt.PointingHandCursor
            onClicked: root.applySort(sortHead.key)
        }
    }

    // ---- Layout ---------------------------------------------------------

    Row {
        anchors.fill: parent
        spacing: 0

        // ============================ SIDEBAR ============================
        Rectangle {
            id: sidebar
            width: root.sidebarWidth
            height: parent.height
            color: root.canvas

            Rail {
                id: rail
                height: parent.height
                current: root.section
                onChosen: function (key) { root.chooseSection(key) }
                onMoreRequested: function (anchor) { moreMenu.popup(anchor, anchor.width + 6, 0) }
            }

            // Places, bookmarks, drives and remotes, beside the rail.
            Item {
                id: placesColumn
                anchors.left: rail.right
                anchors.right: parent.right
                height: parent.height

                Rectangle {
                    anchors.right: parent.right
                    width: 1
                    height: parent.height
                    color: root.line
                }

                // ---- Header: where you are, and the panel switch ----
                Item {
                    id: sidebarHead
                    anchors.left: parent.left
                    anchors.right: parent.right
                    anchors.top: parent.top
                    anchors.leftMargin: 14
                    anchors.rightMargin: 10
                    anchors.topMargin: 14
                    height: 30

                    Text {
                        anchors.left: parent.left
                        anchors.right: headActions.left
                        anchors.rightMargin: 8
                        anchors.verticalCenter: parent.verticalCenter
                        text: "Places"
                        elide: Text.ElideMiddle
                        color: root.ink
                        font.family: root.sansFamily
                        font.pixelSize: Theme.title
                        font.weight: Font.Medium
                    }

                    Row {
                        id: headActions
                        anchors.right: parent.right
                        anchors.verticalCenter: parent.verticalCenter
                        spacing: 2

                        IconAction {
                            glyph: "⟳"
                            hint: "Refresh"
                            onActivated: root.bridge.refresh()
                        }

                        IconAction {
                            glyph: root.previewOpen ? "◨" : "◧"
                            hint: root.previewOpen ? "Hide preview" : "Show preview"
                            onActivated: root.previewOpen = !root.previewOpen
                        }
                    }
                }

                // ---- Filter the tree, not the file list ----
                Rectangle {
                    id: folderSearch
                    anchors.left: parent.left
                    anchors.right: parent.right
                    anchors.top: sidebarHead.bottom
                    anchors.leftMargin: 12
                    anchors.rightMargin: 12
                    anchors.topMargin: 12
                    height: 30
                    radius: 7
                    color: root.raised
                    border.width: 1
                    border.color: folderSearchInput.activeFocus ? root.lineStrong : root.line

                    Text {
                        id: folderSearchGlyph
                        anchors.left: parent.left
                        anchors.leftMargin: 10
                        anchors.verticalCenter: parent.verticalCenter
                        text: "⌕"
                        color: root.inkFaint
                        font.family: root.sansFamily
                        font.pixelSize: 14
                    }

                    TextInput {
                        id: folderSearchInput
                        anchors.left: folderSearchGlyph.right
                        anchors.leftMargin: 8
                        anchors.right: parent.right
                        anchors.rightMargin: 10
                        anchors.verticalCenter: parent.verticalCenter
                        clip: true
                        color: root.ink
                        selectionColor: root.emberWash
                        selectedTextColor: root.ink
                        font.family: root.sansFamily
                        font.pixelSize: 13
                        text: root.folderQuery
                        onTextChanged: root.folderQuery = text
                        Keys.onEscapePressed: root.folderQuery = ""

                        Text {
                            anchors.verticalCenter: parent.verticalCenter
                            visible: folderSearchInput.text.length === 0
                            text: "Search…"
                            color: root.inkGhost
                            font.family: root.sansFamily
                            font.pixelSize: 12
                        }
                    }
                }

                // ---- Folders / Places ----
                Rectangle {
                    id: sidebarTabs
                    anchors.left: parent.left
                    anchors.right: parent.right
                    anchors.top: folderSearch.bottom
                    anchors.leftMargin: 12
                    anchors.rightMargin: 12
                    anchors.topMargin: 12
                    height: 32
                    radius: 8
                    color: root.inset
                    border.width: 1
                    border.color: root.lineSoft

                    Row {
                        anchors.fill: parent
                        anchors.margins: 3
                        spacing: 3

                        Segment {
                            width: (parent.width - 3) / 2
                            height: parent.height
                            label: "Places"
                            key: "places"
                        }

                        Segment {
                            width: (parent.width - 3) / 2
                            height: parent.height
                            label: "Tree"
                            key: "folders"
                        }
                    }
                }

                // ---- The tree itself ----
                ListView {
                    id: treeView
                    anchors.left: parent.left
                    anchors.right: parent.right
                    anchors.top: sidebarTabs.bottom
                    anchors.bottom: ringPanel.top
                    anchors.leftMargin: 8
                    anchors.rightMargin: 8
                    anchors.topMargin: 10
                    anchors.bottomMargin: 10
                    clip: true
                    spacing: 1
                    visible: root.sidebarTab === "folders"
                    model: root.tree
                    boundsBehavior: Flickable.StopAtBounds

                    delegate: TreeRow {
                        id: treeEntry
                        required property var modelData
                        width: treeView.width
                        label: treeEntry.modelData.label
                        badge: treeEntry.modelData.badge
                        depth: treeEntry.modelData.depth
                        current: treeEntry.modelData.current
                        onActivated: root.bridge.navigate(treeEntry.modelData.path)
                    }
                }

                ListView {
                    id: placesView
                    anchors.fill: treeView
                    clip: true
                    spacing: 2
                    visible: root.sidebarTab === "places"
                    model: root.places
                    boundsBehavior: Flickable.StopAtBounds
                    section.property: "section"
                    section.delegate: Item {
                        required property string section
                        width: placesView.width
                        height: 40
                        Text {
                            anchors.left: parent.left
                            anchors.leftMargin: 4
                            anchors.bottom: sectionRule.top
                            anchors.bottomMargin: 2
                            text: parent.section.toUpperCase()
                            color: Theme.inkFaint
                            font.family: Theme.family
                            font.pixelSize: 10
                            font.weight: Font.Medium
                            font.letterSpacing: 1
                        }
                        CrossRule {
                            id: sectionRule
                            anchors.left: parent.left
                            anchors.right: parent.right
                            anchors.bottom: parent.bottom
                            anchors.bottomMargin: 2
                        }
                    }

                    delegate: NavItem {
                        id: placeEntry
                        required property var modelData
                        readonly property bool adds: placeEntry.modelData.add === true
                        readonly property bool remote: placeEntry.modelData.remote === true
                        readonly property bool recent: placeEntry.modelData.recent === true
                        readonly property bool trash: placeEntry.modelData.trash === true
                        readonly property bool marked: placeEntry.modelData.bookmark !== undefined
                        readonly property bool drive: placeEntry.modelData.drive !== undefined
                        readonly property string target: placeEntry.remote || placeEntry.marked || placeEntry.drive
                                                         ? placeEntry.modelData.target
                                                         : root.placePath(placeEntry.modelData.folder)
                        width: placesView.width
                        icon: placeEntry.modelData.icon || ""
                        glyph: placeEntry.adds
                               ? "+"
                               : (placeEntry.recent
                                  ? "◴"
                                  : placeEntry.trash
                                  ? "⌫"
                                  : placeEntry.marked
                                  ? "★"
                                  : placeEntry.drive
                                  ? (root.bridge.driveEjectable(placeEntry.modelData.drive) ? "⏏" : "◫")
                                  : placeEntry.remote
                                  ? "◈"
                                  : (placeEntry.modelData.folder === "/"
                                     ? "◆"
                                     : (placeEntry.modelData.folder.length === 0 ? "⌂" : "▸")))
                        label: placeEntry.modelData.label
                        active: placeEntry.recent
                                ? root.recentOpen
                                : placeEntry.trash
                                ? root.trashOpen
                                : (!placeEntry.adds && !root.recentOpen && !root.trashOpen
                                   && root.bridge.currentPath === placeEntry.target)
                        dropTarget: placeEntry.recent || placeEntry.trash || placeEntry.adds || placeEntry.remote
                                    ? "" : placeEntry.target
                        trailing: placeEntry.remote ? "remote"
                                  : placeEntry.drive && placeEntry.target.length === 0 ? "not mounted" : ""
                        onMenuRequested: {
                            if (placeEntry.drive) {
                                driveMenu.index = placeEntry.modelData.drive
                                driveMenu.popup()
                                return
                            }
                            if (!placeEntry.marked)
                                return
                            bookmarkMenu.index = placeEntry.modelData.bookmark
                            bookmarkMenu.popup()
                        }
                        onActivated: {
                            if (placeEntry.recent) {
                                root.showRecent()
                            } else if (placeEntry.trash) {
                                root.showTrash()
                            } else if (placeEntry.drive) {
                                root.recentOpen = false
                                root.trashOpen = false
                                root.bridge.openDrive(placeEntry.modelData.drive)
                            } else if (placeEntry.adds) {
                                root.remoteDraftName = ""
                                root.providerQuery = ""
                                root.remoteSheetOpen = true
                            } else {
                                root.recentOpen = false
                                root.trashOpen = false
                                root.bridge.navigate(placeEntry.target)
                            }
                        }
                    }
                }

                // How full the disk is. The ring earns its height here only
                // because what this folder holds now sits beside its name.
                Rectangle {
                    id: ringPanel
                    anchors.left: parent.left
                    anchors.right: parent.right
                    anchors.bottom: parent.bottom
                    anchors.margins: 12
                    height: 128
                    radius: 10
                    color: root.raised
                    border.width: 1
                    border.color: root.line

                    Overline {
                        id: storageHead
                        anchors.left: parent.left
                        anchors.top: parent.top
                        anchors.margins: 11
                        text: "STORAGE"
                    }

                    Item {
                        id: ring
                        anchors.left: parent.left
                        anchors.leftMargin: 11
                        anchors.top: storageHead.bottom
                        anchors.topMargin: 8
                        width: 62
                        height: 62

                        // What this folder takes of the disk it sits on. It fills
                        // in as the count runs, so a large tree shows progress
                        // rather than nothing.
                        readonly property real share: {
                            void root.revision
                            return root.bridge.folderShare()
                        }
                        readonly property bool counting: {
                            void root.revision
                            return root.bridge.folderMeasuring()
                        }

                        Shape {
                            anchors.fill: parent
                            preferredRendererType: Shape.CurveRenderer

                            // The whole disk.
                            ShapePath {
                                fillColor: "transparent"
                                strokeColor: root.raisedHigh
                                strokeWidth: 9
                                capStyle: ShapePath.FlatCap
                                PathAngleArc {
                                    centerX: ring.width / 2
                                    centerY: ring.height / 2
                                    radiusX: ring.width / 2 - 5
                                    radiusY: ring.height / 2 - 5
                                    startAngle: -90
                                    sweepAngle: 360
                                }
                            }

                            // What is on it.
                            ShapePath {
                                fillColor: "transparent"
                                strokeColor: ring.share > 0.9 ? root.ember : root.spectral
                                strokeWidth: 9
                                capStyle: ShapePath.FlatCap
                                PathAngleArc {
                                    centerX: ring.width / 2
                                    centerY: ring.height / 2
                                    radiusX: ring.width / 2 - 5
                                    radiusY: ring.height / 2 - 5
                                    startAngle: -90
                                    sweepAngle: 360 * ring.share
                                }
                            }
                        }

                        Mono {
                            anchors.centerIn: parent
                            text: {
                                void root.revision
                                return root.readableSize(root.bridge.folderBytes())
                            }
                            color: ring.counting ? root.inkFaint : root.inkMuted
                            font.pixelSize: 10
                        }
                    }

                    Column {
                        anchors.left: ring.right
                        anchors.leftMargin: 12
                        anchors.right: parent.right
                        anchors.rightMargin: 11
                        anchors.verticalCenter: ring.verticalCenter
                        spacing: 7

                        Repeater {
                            model: {
                                void root.revision
                                if (!root.bridge.storageKnown())
                                    return []
                                return [
                                    {
                                        label: "This folder",
                                        value: root.readableSize(root.bridge.folderBytes()),
                                        shade: root.spectral
                                    },
                                    {
                                        label: "Disk free",
                                        value: root.readableSize(root.bridge.storageTotal()
                                                                 - root.bridge.storageUsed()),
                                        shade: root.raisedHigh
                                    }
                                ]
                            }

                            delegate: Row {
                                id: storageLegend
                                required property var modelData
                                spacing: 6

                                Rectangle {
                                    anchors.verticalCenter: parent.verticalCenter
                                    width: 6
                                    height: 6
                                    radius: 3
                                    color: storageLegend.modelData.shade
                                }

                                Text {
                                    anchors.verticalCenter: parent.verticalCenter
                                    text: storageLegend.modelData.label
                                    color: root.inkFaint
                                    font.family: root.sansFamily
                                    font.pixelSize: 10
                                }

                                Mono {
                                    anchors.verticalCenter: parent.verticalCenter
                                    text: storageLegend.modelData.value
                                    color: root.inkGhost
                                    font.pixelSize: 10
                                }
                            }
                        }

                        Text {
                            visible: {
                                void root.revision
                                return root.bridge.storageKnown()
                            }
                            text: {
                                void root.revision
                                const files = root.bridge.folderFiles()
                                return ring.counting
                                       ? "counting… " + files + " files"
                                       : files + " files in all"
                            }
                            color: root.inkGhost
                            font.family: root.sansFamily
                            font.pixelSize: 10
                        }

                        Text {
                            visible: {
                                void root.revision
                                return !root.bridge.storageKnown()
                            }
                            text: "Not this machine's disk"
                            color: root.inkGhost
                            font.family: root.sansFamily
                            font.pixelSize: 10
                        }
                    }
                }
            }

        }

        // ============================== MAIN ==============================
        Item {
            id: main
            width: parent.width - root.sidebarWidth - (root.previewOpen ? root.inspectorWidth : 0)
            height: parent.height

            // ---- Header: one line, as the mockup has it ----
            // Where you are and how to get elsewhere, left to right: the mark,
            // the way back, the path, the search, the reload.
            Item {
                id: header
                anchors.left: parent.left
                anchors.right: parent.right
                anchors.top: parent.top
                anchors.leftMargin: 16
                anchors.rightMargin: 16
                anchors.topMargin: 14
                height: root.controlHeight

                // The mark, and the one thing it offers: somewhere new.
                Rectangle {
                    id: brand
                    anchors.left: parent.left
                    anchors.verticalCenter: parent.verticalCenter
                    width: brandRow.implicitWidth + 22
                    height: root.controlHeight
                    radius: 9
                    color: root.inset
                    border.width: 1
                    border.color: root.line

                    Row {
                        id: brandRow
                        anchors.centerIn: parent
                        spacing: 9

                        Text {
                            anchors.verticalCenter: parent.verticalCenter
                            text: "FILES"
                            color: root.ink
                            font.family: root.sansFamily
                            font.pixelSize: 14
                            font.weight: Font.DemiBold
                            font.letterSpacing: 1.6
                        }

                        Rectangle {
                            anchors.verticalCenter: parent.verticalCenter
                            width: 1
                            height: 16
                            color: root.line
                        }

                        Text {
                            anchors.verticalCenter: parent.verticalCenter
                            text: "+"
                            color: newFolderMouse.containsMouse ? root.spectral : root.inkMuted
                            font.family: root.sansFamily
                            font.pixelSize: 17

                            MouseArea {
                                id: newFolderMouse
                                anchors.fill: parent
                                anchors.margins: -6
                                hoverEnabled: true
                                cursorShape: Qt.PointingHandCursor
                                onClicked: root.remoteSheetOpen = true
                            }
                        }
                    }
                }

                // One chip per tab, shown once there is more than one.
                Row {
                    id: tabStrip
                    anchors.left: headerNav.right
                    anchors.leftMargin: root.tabs.length > 1 ? 10 : 0
                    anchors.verticalCenter: parent.verticalCenter
                    spacing: 4
                    Repeater {
                        model: root.tabs.length > 1 ? root.tabs : []
                        delegate: Rectangle {
                            id: tabChip
                            required property var modelData
                            required property int index
                            readonly property bool current: tabChip.index === root.currentTab
                            width: Math.min(150, tabLabel.implicitWidth + 34)
                            height: root.controlHeight
                            radius: 9
                            color: tabChip.current ? root.raisedHigh : root.inset
                            border.width: 1
                            border.color: tabChip.current ? root.lineStrong : root.line
                            Text {
                                id: tabLabel
                                anchors.left: parent.left
                                anchors.leftMargin: 10
                                anchors.right: tabClose.left
                                anchors.rightMargin: 4
                                anchors.verticalCenter: parent.verticalCenter
                                text: root.leafName(tabChip.modelData.currentPath)
                                elide: Text.ElideMiddle
                                color: tabChip.current ? root.ink : root.inkMuted
                                font.family: root.sansFamily
                                font.pixelSize: 12
                            }
                            DropArea {
                                anchors.fill: parent
                                onDropped: function (drop) { root.dropInto(drop, tabChip.modelData.currentPath) }
                            }
                            MouseArea {
                                anchors.fill: parent
                                acceptedButtons: Qt.LeftButton | Qt.MiddleButton
                                onClicked: function (mouse) {
                                    if (mouse.button === Qt.MiddleButton)
                                        root.closeTab(tabChip.index)
                                    else
                                        root.switchTab(tabChip.index)
                                }
                            }
                            Text {
                                id: tabClose
                                anchors.right: parent.right
                                anchors.rightMargin: 8
                                anchors.verticalCenter: parent.verticalCenter
                                text: "×"
                                color: tabCloseMouse.containsMouse ? root.ink : root.inkGhost
                                font.pixelSize: 14
                                MouseArea {
                                    id: tabCloseMouse
                                    anchors.fill: parent
                                    anchors.margins: -4
                                    hoverEnabled: true
                                    onClicked: root.closeTab(tabChip.index)
                                }
                            }
                        }
                    }
                }

                Row {
                    id: headerNav
                    anchors.left: brand.right
                    anchors.leftMargin: 10
                    anchors.verticalCenter: parent.verticalCenter
                    spacing: 5

                    IconAction {
                        implicitWidth: root.controlHeight
                        implicitHeight: root.controlHeight
                        glyph: "←"
                        hint: "Back"
                        onActivated: root.bridge.back()
                    }

                    IconAction {
                        implicitWidth: root.controlHeight
                        implicitHeight: root.controlHeight
                        glyph: "→"
                        hint: "Forward"
                        onActivated: root.bridge.forward()
                    }

                    IconAction {
                        implicitWidth: root.controlHeight
                        implicitHeight: root.controlHeight
                        glyph: "↑"
                        hint: "Parent folder"
                        onActivated: root.bridge.up()
                    }
                }

                // The path, given the width the mockup gives it.
                Rectangle {
                    id: pathBar
                    anchors.left: tabStrip.right
                    anchors.leftMargin: 10
                    anchors.right: headerSearch.left
                    anchors.rightMargin: 10
                    anchors.verticalCenter: parent.verticalCenter
                    height: root.controlHeight
                    radius: 9
                    color: root.inset
                    border.width: 1
                    border.color: root.line

                    // Ctrl+L swaps the crumbs for a field that takes a typed
                    // path; Enter goes there, Escape puts the crumbs back.
                    TextInput {
                        id: pathEditor
                        visible: false
                        anchors.fill: parent
                        anchors.leftMargin: 12
                        anchors.rightMargin: 12
                        verticalAlignment: TextInput.AlignVCenter
                        clip: true
                        color: root.ink
                        selectionColor: root.spectralDim
                        selectedTextColor: root.ink
                        font.family: root.monoFamily
                        font.pixelSize: 12
                        function finish() {
                            visible = false
                            root.release()
                        }
                        function go() {
                            let target = text.trim()
                            if (target === "~" || target.startsWith("~/"))
                                target = root.bridge.homePath + target.substring(1)
                            if (target.length > 0)
                                root.openTarget(target)
                            finish()
                        }
                        Keys.onReturnPressed: go()
                        Keys.onEnterPressed: go()
                        Keys.onEscapePressed: finish()
                        onActiveFocusChanged: if (!activeFocus && visible) finish()
                    }

                    Flickable {
                        visible: !pathEditor.visible
                        anchors.left: parent.left
                        anchors.right: parent.right
                        anchors.leftMargin: 12
                        anchors.rightMargin: 12
                        anchors.verticalCenter: parent.verticalCenter
                        height: 20
                        contentWidth: crumbRow.width
                        clip: true
                        flickableDirection: Flickable.HorizontalFlick

                        Row {
                            id: crumbRow
                            height: parent.height
                            spacing: 3

                            Repeater {
                                model: root.crumbs
                                delegate: Row {
                                    id: crumb
                                    required property int index
                                    required property var modelData
                                    readonly property bool last: crumb.index === root.crumbs.length - 1
                                    spacing: 3
                                    anchors.verticalCenter: parent.verticalCenter

                                    Text {
                                        text: crumb.modelData.label
                                        color: crumb.last
                                               ? root.ink
                                               : (crumbMouse.containsMouse ? root.ink : root.inkFaint)
                                        font.family: root.monoFamily
                                        font.pixelSize: 12

                                        MouseArea {
                                            id: crumbMouse
                                            anchors.fill: parent
                                            hoverEnabled: true
                                            cursorShape: Qt.PointingHandCursor
                                            onClicked: root.bridge.navigate(crumb.modelData.path)
                                        }
                                    }

                                    Text {
                                        visible: !crumb.last
                                        text: "\u203a"
                                        color: root.inkGhost
                                        font.family: root.sansFamily
                                        font.pixelSize: 11
                                    }
                                }
                            }
                        }
                    }
                }

                Rectangle {
                    id: headerSearch
                    anchors.right: headerReload.left
                    anchors.rightMargin: 10
                    anchors.verticalCenter: parent.verticalCenter
                    width: 260
                    height: root.controlHeight
                    radius: 9
                    color: root.inset
                    border.width: 1
                    border.color: searchInput.activeFocus ? root.spectralDim : root.line

                    Glyph {
                        id: searchMark
                        anchors.left: parent.left
                        anchors.leftMargin: 11
                        anchors.verticalCenter: parent.verticalCenter
                        text: root.deepSearch ? "\u2318" : "\u2315"
                        color: root.deepSearch ? root.spectral : root.inkGhost
                        font.pixelSize: 13
                    }

                    TextInput {
                        id: searchInput
                        anchors.left: searchMark.right
                        anchors.leftMargin: 8
                        anchors.right: clearSearch.left
                        anchors.rightMargin: 6
                        anchors.verticalCenter: parent.verticalCenter
                        clip: true
                        color: root.ink
                        selectionColor: root.spectralDim
                        selectedTextColor: root.ink
                        font.family: root.sansFamily
                        font.pixelSize: 13
                        text: root.query
                        onTextChanged: root.query = text
                        Keys.onEscapePressed: {
                            if (root.deepSearch)
                                root.endDeepSearch()
                            else
                                root.query = ""
                        }
                        Keys.onReturnPressed: root.beginDeepSearch()
                        Keys.onEnterPressed: root.beginDeepSearch()

                        Text {
                            anchors.verticalCenter: parent.verticalCenter
                            visible: searchInput.text.length === 0
                            text: "search"
                            color: root.inkGhost
                            font.family: root.sansFamily
                            font.pixelSize: 12
                        }
                    }

                    IconAction {
                        id: clearSearch
                        anchors.right: parent.right
                        anchors.rightMargin: 3
                        anchors.verticalCenter: parent.verticalCenter
                        visible: root.query.length > 0
                        implicitWidth: 24
                        implicitHeight: 24
                        glyph: "\u2715"
                        hint: "Clear search"
                        onActivated: {
                            root.endDeepSearch()
                            root.query = ""
                        }
                    }
                }

                IconAction {
                    id: headerReload
                    anchors.right: parent.right
                    anchors.verticalCenter: parent.verticalCenter
                    implicitWidth: root.controlHeight
                    implicitHeight: root.controlHeight
                    glyph: "\u27f3"
                    hint: "Refresh"
                    onActivated: root.bridge.refresh()
                }
            }

            // ---- Folder bar: the name, what is in here, how to look at it ----
            Item {
                id: folderBar
                anchors.left: parent.left
                anchors.right: parent.right
                anchors.top: header.bottom
                anchors.leftMargin: 16
                anchors.rightMargin: 16
                anchors.topMargin: 12
                height: root.bandHeight

                // The name sits on a tab that runs into the pane below it, so
                // the folder reads as the label of what is being shown.
                Item {
                    id: folderTab
                    anchors.left: parent.left
                    anchors.bottom: parent.bottom
                    width: Math.min(folderName.implicitWidth + 34, parent.width * 0.42)
                    height: parent.height

                    Shape {
                        anchors.fill: parent
                        preferredRendererType: Shape.CurveRenderer

                        ShapePath {
                            fillColor: root.raised
                            strokeColor: root.line
                            strokeWidth: 1
                            // Square at the top-left, cut away at the
                            // bottom-right so the tab leans into the pane.
                            startX: 1; startY: 1
                            PathLine { x: folderTab.width - 1; y: 1 }
                            PathLine { x: folderTab.width - 1; y: folderTab.height - 15 }
                            PathLine { x: folderTab.width - 15; y: folderTab.height - 1 }
                            PathLine { x: 1; y: folderTab.height - 1 }
                            PathLine { x: 1; y: 1 }
                        }
                    }

                    Text {
                        id: folderName
                        anchors.left: parent.left
                        anchors.leftMargin: 16
                        anchors.right: parent.right
                        anchors.rightMargin: 18
                        anchors.verticalCenter: parent.verticalCenter
                        anchors.verticalCenterOffset: -3
                        text: root.recentOpen
                              ? "Recent"
                              : root.trashOpen
                              ? "Trash"
                              : root.leafName(root.bridge.currentPath)
                        elide: Text.ElideMiddle
                        color: root.ink
                        font.family: root.sansFamily
                        font.pixelSize: 19
                        font.weight: Font.DemiBold
                    }
                }

                // What this folder is made of, beside its name.
                Rectangle {
                    id: mixPanel
                    anchors.left: folderTab.right
                    anchors.leftMargin: 14
                    anchors.bottom: parent.bottom
                    // Bounded, not stretched. Given the whole gap it grew to a
                    // thousand pixels of bar for five numbers, which is what
                    // made the top of the window look lopsided.
                    width: Math.max(0, Math.min(440, parent.width - folderTab.width
                                                     - viewControls.width - 40))
                    height: parent.height
                    radius: 9
                    color: root.inset
                    border.width: 1
                    border.color: root.line
                    visible: width > 180

                    Column {
                        id: mixColumn
                        anchors.left: parent.left
                        anchors.right: parent.right
                        anchors.verticalCenter: parent.verticalCenter
                        anchors.margins: 10
                        spacing: 6

                        Item {
                            width: parent.width
                            height: 11

                            Overline {
                                anchors.left: parent.left
                                anchors.verticalCenter: parent.verticalCenter
                                text: "THIS FOLDER"
                            }

                            Mono {
                                anchors.right: parent.right
                                anchors.verticalCenter: parent.verticalCenter
                                text: root.compositionTotal + " items"
                                font.pixelSize: 10
                            }
                        }

                        Row {
                            width: parent.width
                            height: 5
                            spacing: 2

                            Repeater {
                                model: root.composition

                                delegate: Rectangle {
                                    id: mixSegment
                                    required property var modelData
                                    readonly property real share: root.compositionTotal > 0
                                                                  ? mixSegment.modelData.count / root.compositionTotal
                                                                  : 0
                                    visible: mixSegment.modelData.count > 0
                                    width: Math.max(0, (mixColumn.width
                                                        - 2 * Math.max(0, root.compositionShown - 1))
                                                       * mixSegment.share)
                                    height: 5
                                    radius: 3
                                    color: mixSegment.modelData.shade
                                }
                            }

                            Rectangle {
                                visible: root.compositionTotal === 0
                                width: mixColumn.width
                                height: 5
                                radius: 3
                                color: root.raisedHigh
                            }
                        }

                        Row {
                            width: parent.width
                            spacing: 10
                            clip: true

                            Repeater {
                                model: root.composition

                                delegate: Row {
                                    id: mixLegend
                                    required property var modelData
                                    visible: mixLegend.modelData.count > 0
                                    spacing: 5

                                    Rectangle {
                                        anchors.verticalCenter: parent.verticalCenter
                                        width: 5
                                        height: 5
                                        radius: 3
                                        color: mixLegend.modelData.shade
                                    }

                                    Text {
                                        anchors.verticalCenter: parent.verticalCenter
                                        text: mixLegend.modelData.label
                                        color: root.inkFaint
                                        font.family: root.sansFamily
                                        font.pixelSize: 10
                                    }

                                    Mono {
                                        anchors.verticalCenter: parent.verticalCenter
                                        text: mixLegend.modelData.count
                                        color: root.inkGhost
                                        font.pixelSize: 10
                                    }
                                }
                            }
                        }
                    }
                }

                Row {
                    id: viewControls
                    anchors.right: parent.right
                    anchors.verticalCenter: parent.verticalCenter
                    spacing: 6

                    IconAction {
                        glyph: root.showHidden ? "\u25c9" : "\u25cc"
                        active: root.showHidden
                        hint: root.showHidden
                              ? "Hide dotfiles  (Ctrl+H)"
                              : "Show hidden files  (Ctrl+H)"
                        onActivated: root.showHidden = !root.showHidden
                    }

                    IconAction {
                        glyph: root.previewOpen ? "\u25e8" : "\u25e7"
                        hint: root.previewOpen ? "Hide preview" : "Show preview"
                        onActivated: root.previewOpen = !root.previewOpen
                    }

                    // List and grid share one segmented control.
                    Rectangle {
                        width: 72
                        height: 30
                        radius: 8
                        color: root.inset
                        border.width: 1
                        border.color: root.lineSoft

                        Row {
                            anchors.centerIn: parent
                            spacing: 2

                            Repeater {
                                model: [{ mode: "list", glyph: "\u2261" }, { mode: "grid", glyph: "\u25a6" }]
                                delegate: Rectangle {
                                    id: viewOption
                                    required property var modelData
                                    readonly property bool current: root.view === viewOption.modelData.mode
                                    width: 32
                                    height: 24
                                    radius: 6
                                    color: viewOption.current ? root.raisedHigh : "transparent"
                                    border.width: viewOption.current ? 1 : 0
                                    border.color: root.line

                                    Glyph {
                                        anchors.centerIn: parent
                                        text: viewOption.modelData.glyph
                                        color: viewOption.current ? root.ink : root.inkGhost
                                        font.pixelSize: 13
                                    }

                                    MouseArea {
                                        anchors.fill: parent
                                        cursorShape: Qt.PointingHandCursor
                                        onClicked: root.view = viewOption.modelData.mode
                                    }
                                }
                            }
                        }
                    }
                }
            }

            // ---- Browser ----
            Item {
                id: browser
                anchors.left: parent.left
                anchors.right: parent.right
                anchors.top: folderBar.bottom
                anchors.bottom: parent.bottom
                anchors.leftMargin: 16
                anchors.rightMargin: 16
                anchors.topMargin: 0
                anchors.bottomMargin: 14
                clip: true

                // The pane is cut away at the bottom-right, as the mockup draws
                // it. A Shape rather than a Rectangle, since a corner cannot be
                // taken out of a radius.
                Shape {
                    anchors.fill: parent
                    preferredRendererType: Shape.CurveRenderer

                    ShapePath {
                        fillColor: root.raised
                        strokeColor: root.line
                        strokeWidth: 1
                        startX: 12; startY: 1
                        PathLine { x: browser.width - 12; y: 1 }
                        PathArc {
                            x: browser.width - 1; y: 12
                            radiusX: 11; radiusY: 11
                        }
                        PathLine { x: browser.width - 1; y: browser.height - 54 }
                        PathLine { x: browser.width - 54; y: browser.height - 1 }
                        PathLine { x: 12; y: browser.height - 1 }
                        PathArc {
                            x: 1; y: browser.height - 12
                            radiusX: 11; radiusY: 11
                        }
                        PathLine { x: 1; y: 12 }
                        PathArc {
                            x: 12; y: 1
                            radiusX: 11; radiusY: 11
                        }
                    }
                }

                // Selection toolbar, present only while something is held.
                Rectangle {
                    id: selectionBar
                    anchors.left: parent.left
                    anchors.right: parent.right
                    anchors.top: parent.top
                    height: root.selectedCount > 0 ? 44 : 0
                    visible: height > 0
                    color: root.raisedHigh

                    Behavior on height {
                        NumberAnimation { duration: 90 }
                    }

                    Rectangle {
                        anchors.bottom: parent.bottom
                        width: parent.width
                        height: 1
                        color: root.line
                    }

                    Row {
                        anchors.left: parent.left
                        anchors.leftMargin: 12
                        anchors.verticalCenter: parent.verticalCenter
                        spacing: 10

                        IconAction {
                            glyph: "✕"
                            hint: "Clear selection"
                            anchors.verticalCenter: parent.verticalCenter
                            onActivated: root.bridge.clearSelection()
                        }

                        Text {
                            anchors.verticalCenter: parent.verticalCenter
                            text: root.selectedCount + " selected"
                            color: root.ink
                            font.family: root.sansFamily
                            font.pixelSize: 12
                            font.weight: Font.Medium
                        }
                    }

                    Row {
                        anchors.right: parent.right
                        anchors.rightMargin: 12
                        anchors.verticalCenter: parent.verticalCenter
                        spacing: 6

                        // Whatever is held, opening it is only meaningful when
                        // it is one directory.
                        TextAction {
                            label: "Open"
                            visible: root.selectedCount === 1 && root.focusRow >= 0
                                     && root.bridge.entryIsDirectory(root.focusRow)
                            anchors.verticalCenter: parent.verticalCenter
                            onActivated: root.bridge.navigateRow(root.focusRow)
                        }

                        // Copy and move need somewhere to land; the field is
                        // seeded with the folder on screen.
                        Rectangle {
                            width: 200
                            height: 26
                            radius: 6
                            color: root.inset
                            border.width: 1
                            border.color: destinationInput.activeFocus ? root.lineStrong : root.line
                            anchors.verticalCenter: parent.verticalCenter

                            Text {
                                id: destinationMark
                                anchors.left: parent.left
                                anchors.leftMargin: 8
                                anchors.verticalCenter: parent.verticalCenter
                                text: "→"
                                color: root.inkGhost
                                font.family: root.sansFamily
                                font.pixelSize: 11
                            }

                            TextInput {
                                id: destinationInput
                                anchors.fill: parent
                                anchors.leftMargin: 24
                                anchors.rightMargin: 8
                                verticalAlignment: TextInput.AlignVCenter
                                clip: true
                                color: root.inkMuted
                                selectionColor: root.spectralDim
                                selectedTextColor: root.ink
                                font.family: root.monoFamily
                                font.pixelSize: 11
                                text: root.bridge.currentPath
                            }
                        }

                        TextAction {
                            label: "Copy"
                            anchors.verticalCenter: parent.verticalCenter
                            onActivated: root.bridge.copySelected(destinationInput.text)
                        }

                        TextAction {
                            label: "Move"
                            anchors.verticalCenter: parent.verticalCenter
                            onActivated: root.bridge.moveSelected(destinationInput.text)
                        }

                        TextAction {
                            label: "Trash"
                            danger: true
                            anchors.verticalCenter: parent.verticalCenter
                            onActivated: root.bridge.trashSelected()
                        }

                        TextAction {
                            label: "Delete"
                            danger: true
                            anchors.verticalCenter: parent.verticalCenter
                            onActivated: deleteConfirm.open()
                        }
                    }
                }

                // Column heads, list view only.
                Item {
                    id: columnHeads
                    anchors.left: parent.left
                    anchors.right: parent.right
                    anchors.top: selectionBar.bottom
                    anchors.leftMargin: 14
                    anchors.rightMargin: 14
                    height: visible ? 30 : 0
                    visible: root.view === "list" && !root.trashOpen && !root.recentOpen

                    SortHead {
                        anchors.left: parent.left
                        anchors.leftMargin: 25
                        anchors.right: kindHead.left
                        anchors.verticalCenter: parent.verticalCenter
                        label: "Name"
                        key: "name"
                    }

                    SortHead {
                        id: kindHead
                        anchors.right: sizeHead.left
                        anchors.rightMargin: 18
                        anchors.verticalCenter: parent.verticalCenter
                        width: 84
                        label: "Kind"
                        key: "kind"
                        numeric: true
                    }

                    Overline {
                        id: sizeHead
                        anchors.right: modifiedHead.left
                        anchors.rightMargin: 18
                        anchors.verticalCenter: parent.verticalCenter
                        width: 84
                        horizontalAlignment: Text.AlignRight
                        text: "SIZE"
                    }

                    SortHead {
                        id: modifiedHead
                        anchors.right: parent.right
                        anchors.verticalCenter: parent.verticalCenter
                        width: 116
                        label: "Modified"
                        key: "modified"
                        numeric: true
                    }

                    Rectangle {
                        anchors.bottom: parent.bottom
                        width: parent.width
                        height: 1
                        color: root.lineSoft
                    }
                }

                // A directory is being read. On a remote that is a network
                // round trip, so it is worth saying so rather than showing an
                // empty folder that is about to fill.
                Column {
                    anchors.centerIn: parent
                    spacing: 10
                    visible: !root.deepSearch && !root.recentOpen && !root.trashOpen && root.bridge.loading && root.rows.length === 0

                    Item {
                        id: spinnerTrack
                        width: 26
                        height: 26
                        anchors.horizontalCenter: parent.horizontalCenter

                        Rectangle {
                            anchors.fill: parent
                            radius: width / 2
                            color: "transparent"
                            border.width: 2
                            border.color: root.line
                        }

                        // One lit bead running the ring: at this size that
                        // reads as motion where a spinning glyph reads as
                        // jitter. The bead is carried by a turning frame so the
                        // ring itself stays still.
                        Item {
                            id: spinnerArm
                            anchors.fill: parent
                            transformOrigin: Item.Center

                            Rectangle {
                                width: 6
                                height: 6
                                radius: 3
                                color: root.spectral
                                x: spinnerTrack.width / 2 - width / 2
                                y: -1
                            }

                            RotationAnimator {
                                target: spinnerArm
                                running: root.bridge.loading
                                from: 0
                                to: 360
                                duration: 900
                                loops: Animation.Infinite
                            }
                        }
                    }

                    Text {
                        anchors.horizontalCenter: parent.horizontalCenter
                        text: "Reading this folder…"
                        color: root.inkFaint
                        font.family: root.sansFamily
                        font.pixelSize: 13
                    }
                }

                // Nothing to show: say which of the two reasons it is.
                Column {
                    anchors.centerIn: parent
                    spacing: 6
                    visible: !root.deepSearch && !root.recentOpen && !root.trashOpen && !root.bridge.loading && root.rows.length === 0

                    Glyph {
                        anchors.horizontalCenter: parent.horizontalCenter
                        text: root.query.length > 0 ? "⌕" : "∅"
                        font.pixelSize: 20
                        color: root.inkGhost
                    }

                    Text {
                        anchors.horizontalCenter: parent.horizontalCenter
                        text: root.query.length > 0
                              ? "Nothing matches “" + root.query + "”"
                              : "This folder is empty"
                        color: root.inkFaint
                        font.family: root.sansFamily
                        font.pixelSize: 13
                    }
                }

                // ---- Recent ----
                // The desktop's own record of what was opened, by any
                // application. Nothing is written back to it: browsing is not
                // using, and the rest of the desktop reads this list too.
                // Trash actions, above the list.
                Row {
                    visible: root.trashOpen
                    anchors.top: selectionBar.bottom
                    anchors.topMargin: 12
                    anchors.right: parent.right
                    anchors.rightMargin: 16
                    spacing: 8
                    Mono {
                        anchors.verticalCenter: parent.verticalCenter
                        text: root.bridge.trashBusy() ? "Deleting…" : ""
                        font.pixelSize: 11
                    }
                    Button {
                        text: "Restore"
                        enabled: root.trashPicked.length > 0
                        onClicked: {
                            root.bridge.restoreTrash(root.trashPicked)
                            root.trashPicked = []
                        }
                    }
                    Button {
                        text: "Delete permanently"
                        enabled: root.trashPicked.length > 0 && !root.bridge.trashBusy()
                        onClicked: {
                            root.bridge.purgeTrash(root.trashPicked)
                            root.trashPicked = []
                        }
                    }
                    Button {
                        text: "Empty Trash"
                        enabled: trashBrowser.count > 0 && !root.bridge.trashBusy()
                        onClicked: emptyConfirm.open()
                    }
                }

                // The trash: what was thrown away, where from, and when.
                ListView {
                    id: trashBrowser
                    anchors.left: parent.left
                    anchors.right: parent.right
                    anchors.top: selectionBar.bottom
                    anchors.bottom: parent.bottom
                    anchors.margins: 12
                    anchors.topMargin: 52
                    visible: root.trashOpen
                    clip: true
                    model: {
                        void root.revision
                        return root.bridge.trashCount()
                    }
                    boundsBehavior: Flickable.StopAtBounds
                    cacheBuffer: 800

                    delegate: Rectangle {
                        id: trashRow
                        required property int index
                        readonly property string name: {
                            void root.revision
                            return root.bridge.trashName(trashRow.index)
                        }
                        readonly property bool picked: root.trashPicked.indexOf(trashRow.index) >= 0
                        readonly property string kind: root.kindOf(trashRow.name, false)

                        width: ListView.view.width
                        height: 44
                        radius: 6
                        color: trashRow.picked ? root.spectralDim
                               : trashMouse.containsMouse ? root.hover : "transparent"

                        EntryIcon {
                            id: trashGlyph
                            anchors.left: parent.left
                            anchors.leftMargin: 10
                            anchors.verticalCenter: parent.verticalCenter
                            width: 20
                            height: 20
                            entry: -1
                            allowThumbnail: false
                            kind: trashRow.kind
                            tint: root.markTone(trashRow.kind)
                            markSize: trashRow.kind === "Code" ? 10 : 12
                        }

                        Text {
                            id: trashName
                            anchors.left: trashGlyph.right
                            anchors.leftMargin: 10
                            anchors.right: trashWhen.left
                            anchors.rightMargin: 12
                            anchors.top: parent.top
                            anchors.topMargin: 6
                            text: trashRow.name
                            elide: Text.ElideMiddle
                            color: root.ink
                            font.family: root.sansFamily
                            font.pixelSize: 13
                        }

                        Mono {
                            anchors.left: trashName.left
                            anchors.right: trashWhen.left
                            anchors.rightMargin: 12
                            anchors.top: trashName.bottom
                            anchors.topMargin: 1
                            text: root.bridge.trashOrigin(trashRow.index)
                            elide: Text.ElideLeft
                            font.pixelSize: 10
                        }

                        Mono {
                            id: trashWhen
                            anchors.right: parent.right
                            anchors.rightMargin: 12
                            anchors.verticalCenter: parent.verticalCenter
                            text: root.modifiedText(root.bridge.trashDeleted(trashRow.index))
                            font.pixelSize: 11
                        }

                        MouseArea {
                            id: trashMouse
                            anchors.fill: parent
                            hoverEnabled: true
                            acceptedButtons: Qt.LeftButton | Qt.RightButton
                            onClicked: function (mouse) {
                                if (mouse.button === Qt.RightButton) {
                                    if (!trashRow.picked)
                                        root.trashPicked = [trashRow.index]
                                    trashMenu.popup()
                                    return
                                }
                                root.pickTrash(trashRow.index, (mouse.modifiers & Qt.ControlModifier) !== 0)
                            }
                        }
                    }

                    Column {
                        anchors.centerIn: parent
                        spacing: 6
                        visible: trashBrowser.count === 0
                        Glyph {
                            anchors.horizontalCenter: parent.horizontalCenter
                            text: "⌫"
                            font.pixelSize: 20
                            color: root.inkGhost
                        }
                        Text {
                            anchors.horizontalCenter: parent.horizontalCenter
                            text: "The trash is empty"
                            color: root.inkMuted
                            font.family: root.sansFamily
                            font.pixelSize: 13
                        }
                    }
                }

                ListView {
                    id: recentBrowser
                    anchors.left: parent.left
                    anchors.right: parent.right
                    anchors.top: selectionBar.bottom
                    anchors.bottom: parent.bottom
                    anchors.margins: 12
                    visible: root.recentOpen
                    clip: true
                    model: {
                        void root.revision
                        return root.bridge.recentCount()
                    }
                    boundsBehavior: Flickable.StopAtBounds
                    cacheBuffer: 800

                    delegate: Rectangle {
                        id: recentRow
                        required property int index
                        readonly property string name: {
                            void root.revision
                            return root.bridge.recentName(recentRow.index)
                        }
                        readonly property string where: {
                            void root.revision
                            return root.bridge.recentFolder(recentRow.index)
                        }
                        readonly property string kind: root.kindOf(recentRow.name, false)

                        width: ListView.view.width
                        height: 44
                        radius: 6
                        color: recentMouse.containsMouse ? root.hover : "transparent"

                        EntryIcon {
                            id: recentGlyph
                            anchors.left: parent.left
                            anchors.leftMargin: 10
                            anchors.verticalCenter: parent.verticalCenter
                            width: 20
                            height: 20
                            entry: -1
                            allowThumbnail: false
                            kind: recentRow.kind
                            tint: root.markTone(recentRow.kind)
                            markSize: recentRow.kind === "Code" ? 10 : 12
                        }

                        Text {
                            id: recentName
                            anchors.left: recentGlyph.right
                            anchors.leftMargin: 10
                            anchors.right: parent.right
                            anchors.rightMargin: 12
                            anchors.top: parent.top
                            anchors.topMargin: 6
                            text: recentRow.name
                            elide: Text.ElideMiddle
                            color: root.inkMuted
                            font.family: root.sansFamily
                            font.pixelSize: 13
                        }

                        Mono {
                            anchors.left: recentName.left
                            anchors.right: parent.right
                            anchors.rightMargin: 12
                            anchors.top: recentName.bottom
                            anchors.topMargin: 1
                            text: recentRow.where
                            elide: Text.ElideLeft
                            font.pixelSize: 10
                        }

                        MouseArea {
                            id: recentMouse
                            anchors.fill: parent
                            hoverEnabled: true
                            cursorShape: Qt.PointingHandCursor
                            onClicked: {
                                root.recentOpen = false
                                root.bridge.navigate(recentRow.where)
                            }
                            onDoubleClicked: root.bridge.openPath(root.bridge.recentPath(recentRow.index))
                        }
                    }

                    Column {
                        anchors.centerIn: parent
                        spacing: 6
                        visible: recentBrowser.count === 0

                        Glyph {
                            anchors.horizontalCenter: parent.horizontalCenter
                            text: "◴"
                            font.pixelSize: 20
                            color: root.inkGhost
                        }

                        Text {
                            anchors.horizontalCenter: parent.horizontalCenter
                            text: "The desktop has no record of recent files"
                            color: root.inkFaint
                            font.family: root.sansFamily
                            font.pixelSize: 13
                        }
                    }
                }

                // ---- Search results ----
                // Everything beneath this folder whose name matches. Results
                // appear while the walk is still running, so a match in the
                // first directory read is on screen in milliseconds.
                ListView {
                    id: resultsBrowser
                    anchors.left: parent.left
                    anchors.right: parent.right
                    anchors.top: selectionBar.bottom
                    anchors.bottom: resultsFoot.top
                    anchors.margins: 12
                    visible: root.deepSearch && !root.recentOpen && !root.trashOpen
                    clip: true
                    model: root.searchHits
                    boundsBehavior: Flickable.StopAtBounds
                    cacheBuffer: 800

                    delegate: Rectangle {
                        id: resultRow
                        required property int index
                        readonly property string name: {
                            void root.revision
                            return root.bridge.searchName(resultRow.index)
                        }
                        readonly property string where: {
                            void root.revision
                            return root.bridge.searchFolder(resultRow.index)
                        }
                        readonly property bool folder: {
                            void root.revision
                            return root.bridge.searchIsDirectory(resultRow.index)
                        }
                        readonly property string kind: root.kindOf(resultRow.name, resultRow.folder)

                        width: ListView.view.width
                        height: 44
                        radius: 6
                        color: resultMouse.containsMouse ? root.hover : "transparent"

                        EntryIcon {
                            id: resultGlyph
                            anchors.left: parent.left
                            anchors.leftMargin: 10
                            anchors.verticalCenter: parent.verticalCenter
                            width: 20
                            height: 20
                            // A result is not a row of the open directory, so
                            // there is no entry to read a thumbnail from.
                            entry: -1
                            allowThumbnail: false
                            kind: resultRow.kind
                            tint: resultRow.folder ? root.ink : root.markTone(resultRow.kind)
                            markSize: resultRow.kind === "Code" ? 10 : 12
                        }

                        Text {
                            id: resultName
                            anchors.left: resultGlyph.right
                            anchors.leftMargin: 10
                            anchors.right: parent.right
                            anchors.rightMargin: 12
                            anchors.top: parent.top
                            anchors.topMargin: 6
                            text: resultRow.name
                            elide: Text.ElideMiddle
                            color: resultRow.folder ? root.ink : root.inkMuted
                            font.family: root.sansFamily
                            font.pixelSize: 13
                            font.weight: resultRow.folder ? Font.Medium : Font.Normal
                        }

                        Mono {
                            anchors.left: resultName.left
                            anchors.right: parent.right
                            anchors.rightMargin: 12
                            anchors.top: resultName.bottom
                            anchors.topMargin: 1
                            text: resultRow.where
                            elide: Text.ElideLeft
                            font.pixelSize: 10
                        }

                        MouseArea {
                            id: resultMouse
                            anchors.fill: parent
                            hoverEnabled: true
                            cursorShape: Qt.PointingHandCursor
                            // One click goes to where the result lives, which is
                            // what a result is usually being looked for.
                            onClicked: {
                                root.endDeepSearch()
                                root.query = ""
                                root.bridge.navigate(resultRow.folder
                                                       ? root.bridge.searchPath(resultRow.index)
                                                       : resultRow.where)
                            }
                            onDoubleClicked: {
                                if (!resultRow.folder)
                                    root.bridge.openPath(root.bridge.searchPath(resultRow.index))
                            }
                        }
                    }
                }

                // What the walk has found so far, and whether it is still going.
                Rectangle {
                    id: resultsFoot
                    anchors.left: parent.left
                    anchors.right: parent.right
                    anchors.bottom: parent.bottom
                    height: root.deepSearch ? 30 : 0
                    visible: root.deepSearch
                    color: "transparent"

                    Text {
                        anchors.left: parent.left
                        anchors.leftMargin: 14
                        anchors.verticalCenter: parent.verticalCenter
                        text: {
                            void root.revision
                            const found = root.searchHits
                            const capped = root.bridge.searchTruncated()
                            if (root.bridge.searching)
                                return "Searching…  " + found + " found"
                            if (found === 0)
                                return "Nothing beneath this folder matches “" + root.query + "”"
                            return found + (capped ? "+ matches (showing the first " + found + ")" : " matches")
                        }
                        color: root.inkFaint
                        font.family: root.sansFamily
                        font.pixelSize: 12
                    }

                    Text {
                        anchors.right: parent.right
                        anchors.rightMargin: 14
                        anchors.verticalCenter: parent.verticalCenter
                        text: "Esc to leave"
                        color: root.inkGhost
                        font.family: root.sansFamily
                        font.pixelSize: 11
                    }
                }

                // Dropping on the open folder rather than on a folder in it.
                DropArea {
                    anchors.left: parent.left
                    anchors.right: parent.right
                    anchors.top: selectionBar.bottom
                    anchors.bottom: parent.bottom
                    enabled: !root.recentOpen && !root.trashOpen && !root.deepSearch
                    onDropped: function (drop) { root.dropInto(drop, root.bridge.currentPath) }
                }

                // ---- List ----
                ListView {
                    id: listBrowser
                    anchors.left: parent.left
                    anchors.right: parent.right
                    anchors.top: columnHeads.bottom
                    anchors.bottom: parent.bottom
                    anchors.leftMargin: 14
                    anchors.rightMargin: 14
                    anchors.bottomMargin: 8
                    visible: !root.deepSearch && !root.recentOpen && !root.trashOpen && root.view === "list" && root.rows.length > 0
                    clip: true
                    model: root.rows
                    boundsBehavior: Flickable.StopAtBounds
                    // Resizing changes the layout constantly. Holding a screen
                    // of delegates either side keeps that from rebuilding them,
                    // and every binding in them, on the way past.
                    cacheBuffer: 800

                    // Empty ground. It sits under the delegates and stretches
                    // past the last one, so a click landing on no entry lets go
                    // of the selection and a drag sweeps a band.
                    MouseArea {
                        id: listGround
                        z: -1
                        width: parent.width
                        height: Math.max(listBrowser.contentHeight, listBrowser.height)
                        // The view would otherwise read the drag as a flick.
                        preventStealing: true
                        acceptedButtons: Qt.LeftButton | Qt.RightButton

                        onPressed: function (mouse) {
                            if (mouse.button === Qt.LeftButton)
                                root.sweepBegin(mouse.x, mouse.y)
                        }
                        onPositionChanged: function (mouse) {
                            if (listGround.pressed && (listGround.pressedButtons & Qt.LeftButton))
                                root.sweepMove(mouse.x, mouse.y)
                        }
                        onReleased: root.sweepRelease()
                        onClicked: function (mouse) {
                            if (mouse.button === Qt.RightButton) {
                                groundMenu.popup()
                                return
                            }
                            if (!root.sweepConsumed())
                                root.bridge.clearSelection()
                        }
                        onCanceled: root.sweepRelease()
                    }

                    Rectangle {
                        id: listBand
                        z: 2
                        visible: root.sweeping
                        x: root.band.x
                        y: root.band.y
                        width: root.band.width
                        height: root.band.height
                        color: Qt.rgba(root.spectral.r, root.spectral.g, root.spectral.b, 0.12)
                        border.width: 1
                        border.color: root.spectral
                        radius: 3
                    }

                    delegate: Rectangle {
                        id: listRow
                        required property int modelData
                        readonly property int entry: modelData
                        // A sweep in flight answers first, so the highlight
                        // follows the band without a round trip per move.
                        readonly property bool selected: {
                            void root.revision
                            return root.sweepMark[entry] === true || root.bridge.entrySelected(entry)
                        }
                        readonly property bool folder: {
                            void root.revision
                            return root.bridge.entryIsDirectory(entry)
                        }
                        readonly property string kind: {
                            void root.revision
                            return root.kindOf(root.bridge.entryName(entry), listRow.folder)
                        }
                        // A hidden entry is shown at half strength: present when
                        // asked for, never competing with the rest.
                        readonly property real dim: {
                            void root.revision
                            return root.isHidden(root.bridge.entryName(entry)) ? 0.5 : 1.0
                        }

                        width: ListView.view.width
                        height: root.rowHeight
                        radius: 6
                        color: listRow.selected
                               ? root.raisedHigh
                               : (rowMouse.containsMouse ? root.hover : "transparent")

                        // The focused row wears the emission line.
                        Rectangle {
                            anchors.left: parent.left
                            anchors.verticalCenter: parent.verticalCenter
                            width: 2
                            height: parent.height - 14
                            radius: 1
                            color: root.spectral
                            visible: root.focusRow === listRow.entry
                        }

                        Tick {
                            id: rowTick
                            anchors.left: parent.left
                            anchors.leftMargin: 9
                            anchors.verticalCenter: parent.verticalCenter
                            checked: listRow.selected
                            onToggled: {
                                root.focusRow = listRow.entry
                                root.bridge.toggleSelection(listRow.entry)
                            }
                        }

                        // The seam: directories above, everything else below.
                        DashRule {
                            anchors.left: rowTick.right
                            anchors.leftMargin: 10
                            anchors.right: parent.right
                            anchors.rightMargin: 4
                            anchors.top: parent.top
                            visible: root.firstFileRow === listRow.entry && root.rows.length > 0
                        }

                        EntryIcon {
                            id: rowGlyph
                            opacity: listRow.dim
                            anchors.left: rowTick.right
                            anchors.leftMargin: 10
                            anchors.verticalCenter: parent.verticalCenter
                            width: 20
                            height: 20
                            entry: listRow.entry
                            kind: listRow.kind
                            tint: listRow.selected || listRow.folder ? root.ink : root.markTone(listRow.kind)
                            markSize: listRow.kind === "Code" ? 10 : 12
                        }

                        Text {
                            opacity: listRow.dim
                            anchors.left: rowGlyph.right
                            anchors.leftMargin: 8
                            anchors.right: rowKind.left
                            anchors.rightMargin: 12
                            anchors.verticalCenter: parent.verticalCenter
                            text: {
                                void root.revision
                                return root.bridge.entryName(listRow.entry)
                            }
                            elide: Text.ElideMiddle
                            color: listRow.selected || listRow.folder ? root.ink : root.inkMuted
                            font.family: root.sansFamily
                            font.pixelSize: 14
                            font.weight: listRow.folder ? Font.Medium : Font.Normal
                        }

                        Text {
                            id: rowKind
                            opacity: listRow.dim
                            anchors.right: rowSize.left
                            anchors.rightMargin: 18
                            anchors.verticalCenter: parent.verticalCenter
                            width: 84
                            horizontalAlignment: Text.AlignRight
                            elide: Text.ElideRight
                            text: listRow.kind
                            color: root.inkGhost
                            font.family: root.sansFamily
                            font.pixelSize: 12
                        }

                        Mono {
                            id: rowSize
                            opacity: listRow.dim
                            anchors.right: rowModified.left
                            anchors.rightMargin: 18
                            anchors.verticalCenter: parent.verticalCenter
                            width: 84
                            horizontalAlignment: Text.AlignRight
                            text: {
                                void root.revision
                                return root.bridge.entryDetail(listRow.entry)
                            }
                        }

                        Mono {
                            id: rowModified
                            opacity: listRow.dim
                            anchors.right: parent.right
                            anchors.rightMargin: 4
                            anchors.verticalCenter: parent.verticalCenter
                            width: 116
                            horizontalAlignment: Text.AlignRight
                            text: {
                                void root.revision
                                return root.modifiedText(root.bridge.entryModified(listRow.entry))
                            }
                        }

                        // A folder takes what is dropped on it.
                        DropArea {
                            anchors.fill: parent
                            enabled: root.bridge.entryIsDirectory(listRow.entry) && !listRow.selected
                            onDropped: function (drop) { root.dropInto(drop, root.bridge.entryPath(listRow.entry)) }
                            Rectangle {
                                anchors.fill: parent
                                visible: parent.containsDrag
                                color: "transparent"
                                radius: 6
                                border.width: 1
                                border.color: root.spectral
                            }
                        }

                        MouseArea {
                            id: rowMouse
                            anchors.fill: parent
                            anchors.leftMargin: 26
                            hoverEnabled: true
                            cursorShape: Qt.PointingHandCursor
                            preventStealing: true
                            acceptedButtons: Qt.LeftButton | Qt.RightButton
                            onPressed: function (mouse) {
                                if (mouse.button !== Qt.LeftButton)
                                    return
                                const at = rowMouse.mapToItem(listBrowser.contentItem, mouse.x, mouse.y)
                                if (root.armDrag(listRow.entry))
                                    return
                                root.sweepBegin(at.x, at.y)
                            }
                            onPositionChanged: function (mouse) {
                                if (!rowMouse.pressed || !(rowMouse.pressedButtons & Qt.LeftButton))
                                    return
                                const at = rowMouse.mapToItem(listBrowser.contentItem, mouse.x, mouse.y)
                                if (root.dragArmed) {
                                    root.followDrag(at.x, at.y)
                                    return
                                }
                                root.sweepMove(at.x, at.y)
                            }
                            onReleased: {
                                root.dragArmed = false
                                root.sweepRelease()
                            }
                            onClicked: function (mouse) {
                                if (mouse.button === Qt.RightButton) {
                                    root.openEntryMenu(listRow.entry)
                                    return
                                }
                                if (root.sweepConsumed())
                                    return
                                root.pick(listRow.entry, mouse.modifiers & (Qt.ControlModifier | Qt.ShiftModifier))
                            }
                            onDoubleClicked: root.activateRow(listRow.entry)
                            onCanceled: root.sweepRelease()
                        }
                    }
                }

                // ---- Grid ----
                GridView {
                    id: gridBrowser
                    anchors.left: parent.left
                    anchors.right: parent.right
                    anchors.top: selectionBar.bottom
                    anchors.bottom: parent.bottom
                    anchors.margins: 12
                    visible: !root.deepSearch && !root.recentOpen && !root.trashOpen && root.view === "grid" && root.rows.length > 0
                    clip: true
                    model: root.rows
                    cellWidth: Math.floor(width / Math.max(3, Math.floor(width / 116)))
                    cellHeight: 118
                    boundsBehavior: Flickable.StopAtBounds
                    // Resizing changes the layout constantly. Holding a screen
                    // of delegates either side keeps that from rebuilding them,
                    // and every binding in them, on the way past.
                    cacheBuffer: 800

                    // Empty ground. It sits under the delegates and stretches
                    // past the last one, so a click landing on no entry lets go
                    // of the selection and a drag sweeps a band.
                    MouseArea {
                        id: gridGround
                        z: -1
                        width: parent.width
                        height: Math.max(gridBrowser.contentHeight, gridBrowser.height)
                        // The view would otherwise read the drag as a flick.
                        preventStealing: true
                        acceptedButtons: Qt.LeftButton | Qt.RightButton

                        onPressed: function (mouse) {
                            if (mouse.button === Qt.LeftButton)
                                root.sweepBegin(mouse.x, mouse.y)
                        }
                        onPositionChanged: function (mouse) {
                            if (gridGround.pressed && (gridGround.pressedButtons & Qt.LeftButton))
                                root.sweepMove(mouse.x, mouse.y)
                        }
                        onReleased: root.sweepRelease()
                        onClicked: function (mouse) {
                            if (mouse.button === Qt.RightButton) {
                                groundMenu.popup()
                                return
                            }
                            if (!root.sweepConsumed())
                                root.bridge.clearSelection()
                        }
                        onCanceled: root.sweepRelease()
                    }

                    Rectangle {
                        id: gridBand
                        z: 2
                        visible: root.sweeping
                        x: root.band.x
                        y: root.band.y
                        width: root.band.width
                        height: root.band.height
                        color: Qt.rgba(root.spectral.r, root.spectral.g, root.spectral.b, 0.12)
                        border.width: 1
                        border.color: root.spectral
                        radius: 3
                    }

                    delegate: Item {
                        id: tileHost
                        required property int modelData
                        readonly property int entry: modelData
                        // A sweep in flight answers first, so the highlight
                        // follows the band without a round trip per move.
                        readonly property bool selected: {
                            void root.revision
                            return root.sweepMark[entry] === true || root.bridge.entrySelected(entry)
                        }
                        readonly property bool folder: {
                            void root.revision
                            return root.bridge.entryIsDirectory(entry)
                        }
                        readonly property string kind: {
                            void root.revision
                            return root.kindOf(root.bridge.entryName(entry), tileHost.folder)
                        }
                        // A hidden entry is shown at half strength: present when
                        // asked for, never competing with the rest.
                        readonly property real dim: {
                            void root.revision
                            return root.isHidden(root.bridge.entryName(entry)) ? 0.5 : 1.0
                        }

                        width: GridView.view.cellWidth
                        height: GridView.view.cellHeight

                        // A resting tile draws nothing of its own: the icon and
                        // the name are the tile. The card only surfaces to
                        // answer the pointer, the selection or the keyboard.
                        Rectangle {
                            anchors.fill: parent
                            anchors.margins: 4
                            radius: 9
                            color: tileHost.selected
                                   ? root.raisedHigh
                                   : (tileMouse.containsMouse ? root.hover : "transparent")
                            border.width: root.focusRow === tileHost.entry ? 1 : 0
                            border.color: root.spectral

                            // The frame means "this is the file itself". Icons
                            // stand on their own; only a real thumbnail earns
                            // the plate and border around it.
                            Rectangle {
                                id: tilePreview
                                opacity: tileHost.dim
                                anchors.horizontalCenter: parent.horizontalCenter
                                anchors.top: parent.top
                                anchors.topMargin: 10
                                width: 54
                                height: 54
                                radius: 6
                                clip: true
                                color: tileIcon.thumbnailShown ? root.raised : "transparent"
                                border.width: tileIcon.thumbnailShown ? 1 : 0
                                border.color: root.line

                                EntryIcon {
                                    id: tileIcon
                                    anchors.fill: parent
                                    anchors.margins: 1
                                    entry: tileHost.entry
                                    kind: tileHost.kind
                                    tint: tileHost.selected || tileHost.folder
                                          ? root.ink
                                          : root.markTone(tileHost.kind)
                                    glyphSize: 44
                                    markSize: tileHost.kind === "Code" ? 20 : 26
                                }
                            }

                            // Reaching for the tick is a deliberate act, so it
                            // only appears once the tile is under the pointer
                            // or already carries a selection.
                            Tick {
                                anchors.left: parent.left
                                anchors.top: parent.top
                                anchors.margins: 6
                                visible: tileHost.selected || tileMouse.containsMouse
                                checked: tileHost.selected
                                onToggled: {
                                    root.focusRow = tileHost.entry
                                    root.bridge.toggleSelection(tileHost.entry)
                                }
                            }

                            // Two lines of name, centred under the icon. A name
                            // too long for that elides rather than widening the
                            // cell, which would break the grid's rhythm.
                            Text {
                                id: tileName
                                opacity: tileHost.dim
                                anchors.left: parent.left
                                anchors.right: parent.right
                                anchors.top: tilePreview.bottom
                                anchors.margins: 5
                                anchors.topMargin: 8
                                text: {
                                    void root.revision
                                    return root.bridge.entryName(tileHost.entry)
                                }
                                horizontalAlignment: Text.AlignHCenter
                                wrapMode: Text.Wrap
                                maximumLineCount: 2
                                elide: Text.ElideRight
                                lineHeight: 1.15
                                color: tileHost.selected || tileHost.folder ? root.ink : root.inkMuted
                                font.family: root.sansFamily
                                font.pixelSize: 12
                                font.weight: tileHost.folder ? Font.Medium : Font.Normal
                            }

                            // A drag that starts on a tile sweeps a band like
                            // any other, so selecting a run of files does not
                            // depend on finding the gutter between them.
                            // A folder takes what is dropped on it.
                            DropArea {
                                anchors.fill: parent
                                enabled: root.bridge.entryIsDirectory(tileHost.entry) && !tileHost.selected
                                onDropped: function (drop) { root.dropInto(drop, root.bridge.entryPath(tileHost.entry)) }
                                Rectangle {
                                    anchors.fill: parent
                                    visible: parent.containsDrag
                                    color: "transparent"
                                    radius: 6
                                    border.width: 1
                                    border.color: root.spectral
                                }
                            }

                            MouseArea {
                                id: tileMouse
                                anchors.fill: parent
                                hoverEnabled: true
                                cursorShape: Qt.PointingHandCursor
                                preventStealing: true
                                acceptedButtons: Qt.LeftButton | Qt.RightButton
                                onPressed: function (mouse) {
                                    if (mouse.button !== Qt.LeftButton)
                                        return
                                    const at = tileMouse.mapToItem(gridBrowser.contentItem, mouse.x, mouse.y)
                                    if (root.armDrag(tileHost.entry))
                                        return
                                    root.sweepBegin(at.x, at.y)
                                }
                                onPositionChanged: function (mouse) {
                                    if (!tileMouse.pressed || !(tileMouse.pressedButtons & Qt.LeftButton))
                                        return
                                    const at = tileMouse.mapToItem(gridBrowser.contentItem, mouse.x, mouse.y)
                                    if (root.dragArmed) {
                                        root.followDrag(at.x, at.y)
                                        return
                                    }
                                    root.sweepMove(at.x, at.y)
                                }
                                onReleased: {
                                root.dragArmed = false
                                root.sweepRelease()
                            }
                                onClicked: function (mouse) {
                                    if (mouse.button === Qt.RightButton) {
                                        root.openEntryMenu(tileHost.entry)
                                        return
                                    }
                                    if (root.sweepConsumed())
                                        return
                                    root.pick(tileHost.entry, mouse.modifiers & (Qt.ControlModifier | Qt.ShiftModifier))
                                }
                                onDoubleClicked: root.activateRow(tileHost.entry)
                                onCanceled: root.sweepRelease()
                            }
                        }
                    }
                }
            }
        }
        // =========================== PREVIEW =============================
        // A panel, not a fixture: it folds away and gives the width back.
        Rectangle {
            id: inspector
            width: root.previewOpen ? root.inspectorWidth : 0
            height: parent.height
            visible: width > 0
            clip: true
            color: root.canvas

            Behavior on width {
                NumberAnimation { duration: 130; easing.type: Easing.OutCubic }
            }

            readonly property bool single: root.focusRow >= 0 && root.focusRow < root.bridge.entryCount()
            readonly property string entryName: {
                void root.revision
                return inspector.single ? root.bridge.entryName(root.focusRow) : ""
            }
            readonly property bool entryFolder: {
                void root.revision
                return inspector.single && root.bridge.entryIsDirectory(root.focusRow)
            }
            readonly property string entryKind: root.kindOf(inspector.entryName, inspector.entryFolder)

            Rectangle {
                anchors.left: parent.left
                width: 1
                height: parent.height
                color: root.line
            }

            // ---- Panel head ----
            Item {
                id: previewHead
                anchors.left: parent.left
                anchors.right: parent.right
                anchors.top: parent.top
                anchors.leftMargin: 16
                anchors.rightMargin: 10
                anchors.topMargin: 14
                height: 30

                Text {
                    id: previewGlyph
                    anchors.left: parent.left
                    anchors.verticalCenter: parent.verticalCenter
                    text: "▣"
                    color: root.inkFaint
                    font.family: root.sansFamily
                    font.pixelSize: 12
                }

                Text {
                    anchors.left: previewGlyph.right
                    anchors.leftMargin: 8
                    anchors.verticalCenter: parent.verticalCenter
                    text: "File Preview"
                    color: root.inkMuted
                    font.family: root.sansFamily
                    font.pixelSize: 12
                    font.weight: Font.Medium
                }

                IconAction {
                    anchors.right: parent.right
                    anchors.verticalCenter: parent.verticalCenter
                    glyph: "⤢"
                    hint: "Hide preview"
                    onActivated: root.previewOpen = false
                }
            }

            // ---- Nothing focused ----
            Column {
                anchors.centerIn: parent
                width: parent.width - 64
                spacing: 8
                visible: !inspector.single

                Glyph {
                    anchors.horizontalCenter: parent.horizontalCenter
                    text: root.selectedCount > 1 ? root.selectedCount : "◇"
                    font.pixelSize: root.selectedCount > 1 ? 22 : 20
                    color: root.selectedCount > 1 ? root.emberInk : root.inkGhost
                }

                Text {
                    anchors.horizontalCenter: parent.horizontalCenter
                    horizontalAlignment: Text.AlignHCenter
                    width: parent.width
                    wrapMode: Text.WordWrap
                    text: root.selectedCount > 1
                          ? root.selectedCount + " items held"
                          : "Pick a file to read its details."
                    color: root.inkFaint
                    font.family: root.sansFamily
                    font.pixelSize: 12
                }
            }

            // ---- The file itself ----
            Flickable {
                id: previewBody
                anchors.left: parent.left
                anchors.right: parent.right
                anchors.top: previewHead.bottom
                anchors.bottom: previewFoot.top
                anchors.topMargin: 12
                visible: inspector.single
                clip: true
                contentHeight: previewColumn.implicitHeight
                boundsBehavior: Flickable.StopAtBounds

                Column {
                    id: previewColumn
                    anchors.left: parent.left
                    anchors.right: parent.right
                    anchors.leftMargin: 16
                    anchors.rightMargin: 16
                    spacing: 14

                    DashRule { width: previewColumn.width }

                    // The format mark. There is no thumbnail here because
                    // nothing has been decoded, so the box shrinks to the mark
                    // instead of standing empty at full height.
                    Rectangle {
                        width: previewColumn.width
                        height: inspector.entryFolder ? 84 : 132
                        radius: 10
                        color: root.raised
                        border.width: 1
                        border.color: root.line

                        Behavior on height {
                            NumberAnimation { duration: 110 }
                        }

                        Rectangle {
                            anchors.centerIn: parent
                            anchors.verticalCenterOffset: -6
                            width: inspector.entryFolder ? 52 : 68
                            height: width
                            radius: 13
                            color: inspector.entryFolder ? "transparent" : root.raisedHigh
                            border.width: inspector.entryFolder ? 1 : 0
                            border.color: root.line

                            clip: true

                            EntryIcon {
                                anchors.fill: parent
                                entry: inspector.single ? root.focusRow : -1
                                kind: inspector.entryKind
                                tint: root.markTone(inspector.entryKind)
                                glyphSize: 34
                                markSize: inspector.entryKind === "Code" ? 17 : 24
                            }
                        }

                        Overline {
                            anchors.horizontalCenter: parent.horizontalCenter
                            anchors.bottom: parent.bottom
                            anchors.bottomMargin: 10
                            text: inspector.entryKind.toUpperCase()
                            color: root.inkGhost
                        }
                    }

                    DashRule { width: previewColumn.width }

                    Column {
                        width: parent.width
                        spacing: 5

                        Text {
                            width: parent.width
                            text: inspector.entryName
                            wrapMode: Text.WrapAnywhere
                            maximumLineCount: 3
                            elide: Text.ElideRight
                            color: root.ink
                            font.family: root.sansFamily
                            font.pixelSize: 15
                            font.weight: Font.DemiBold
                        }

                        Mono {
                            width: parent.width
                            text: {
                                void root.revision
                                if (!inspector.single)
                                    return ""
                                return root.bridge.entryDetail(root.focusRow)
                                        + "   ·   "
                                        + root.modifiedText(root.bridge.entryModified(root.focusRow))
                            }
                            font.pixelSize: 11
                        }
                    }

                    // Location, spelled out rather than summarised.
                    Column {
                        width: parent.width
                        spacing: 6

                        Overline { text: "LOCATION" }

                        Text {
                            width: parent.width
                            text: {
                                void root.revision
                                return inspector.single ? root.bridge.entryPath(root.focusRow) : ""
                            }
                            wrapMode: Text.WrapAnywhere
                            color: root.inkFaint
                            font.family: root.monoFamily
                            font.pixelSize: 11
                        }
                    }

                    Column {
                        width: parent.width
                        spacing: 8
                        bottomPadding: 8

                        Overline { text: "DETAILS" }

                        Repeater {
                            model: {
                                void root.revision
                                if (!inspector.single)
                                    return []
                                return [
                                    { key: "Kind", value: inspector.entryKind },
                                    { key: "Size", value: root.bridge.entryDetail(root.focusRow) },
                                    { key: "Modified", value: root.modifiedText(root.bridge.entryModified(root.focusRow)) },
                                    { key: "Held", value: root.bridge.entrySelected(root.focusRow) ? "Yes" : "No" }
                                ]
                            }

                            delegate: Item {
                                id: detailRow
                                required property var modelData
                                width: previewColumn.width
                                height: 17

                                Text {
                                    anchors.left: parent.left
                                    anchors.verticalCenter: parent.verticalCenter
                                    text: detailRow.modelData.key
                                    color: root.inkGhost
                                    font.family: root.sansFamily
                                    font.pixelSize: 11
                                }

                                Mono {
                                    anchors.right: parent.right
                                    anchors.verticalCenter: parent.verticalCenter
                                    text: detailRow.modelData.value
                                    color: root.inkMuted
                                    font.pixelSize: 11
                                }
                            }
                        }
                    }
                }
            }

            // ---- Foot: the three things this panel can actually do ----
            Item {
                id: previewFoot
                anchors.left: parent.left
                anchors.right: parent.right
                anchors.bottom: taskPanel.top
                anchors.bottomMargin: root.taskCount > 0 ? 12 : 0
                height: inspector.single ? 60 : 0
                visible: inspector.single

                DashRule {
                    anchors.left: parent.left
                    anchors.right: parent.right
                    anchors.top: parent.top
                    anchors.leftMargin: 16
                    anchors.rightMargin: 16
                }

                Row {
                    anchors.fill: parent
                    anchors.topMargin: 8
                    anchors.leftMargin: 8
                    anchors.rightMargin: 8

                    PanelAction {
                        width: (parent.width - 2) / 3
                        height: parent.height
                        glyph: "↗"
                        label: "Open"
                        onActivated: root.bridge.navigateRow(root.focusRow)
                    }

                    Rectangle {
                        width: 1
                        height: 26
                        anchors.verticalCenter: parent.verticalCenter
                        color: root.line
                    }

                    PanelAction {
                        width: (parent.width - 2) / 3
                        height: parent.height
                        glyph: "⌗"
                        label: "Hold"
                        onActivated: {
                            root.bridge.clearSelection()
                            root.bridge.toggleSelection(root.focusRow)
                        }
                    }

                    Rectangle {
                        width: 1
                        height: 26
                        anchors.verticalCenter: parent.verticalCenter
                        color: root.line
                    }

                    PanelAction {
                        width: (parent.width - 2) / 3
                        height: parent.height
                        glyph: "⌫"
                        label: "Trash"
                        danger: true
                        onActivated: {
                            root.bridge.clearSelection()
                            root.bridge.toggleSelection(root.focusRow)
                            root.bridge.trashSelected()
                        }
                    }
                }
            }

            // Live work sits at the foot of the panel, in spectral green.
            Rectangle {
                id: taskPanel
                anchors.left: parent.left
                anchors.right: parent.right
                anchors.bottom: parent.bottom
                anchors.margins: 16
                anchors.bottomMargin: root.taskCount > 0 ? 16 : 0
                height: root.taskCount > 0 ? Math.min(220, 26 + root.taskCount * 46) : 0
                visible: root.taskCount > 0
                radius: 10
                color: root.raised
                border.width: 1
                border.color: root.line
                clip: true

                Overline {
                    id: taskHeading
                    anchors.left: parent.left
                    anchors.top: parent.top
                    anchors.margins: 12
                    text: "TRANSFERS"
                    color: root.spectral
                }

                ListView {
                    anchors.left: parent.left
                    anchors.right: parent.right
                    anchors.top: taskHeading.bottom
                    anchors.bottom: parent.bottom
                    anchors.margins: 10
                    anchors.topMargin: 6
                    clip: true
                    spacing: 6
                    model: root.tasks
                    boundsBehavior: Flickable.StopAtBounds

                    delegate: Item {
                        id: taskRow
                        required property int modelData
                        readonly property int slot: taskRow.modelData

                        width: ListView.view.width
                        height: 40

                        Text {
                            id: taskTitle
                            anchors.left: parent.left
                            anchors.right: taskCancel.left
                            anchors.rightMargin: 6
                            anchors.top: parent.top
                            elide: Text.ElideMiddle
                            text: {
                                void root.revision
                                return root.bridge.taskTitle(taskRow.slot)
                            }
                            color: root.inkMuted
                            font.family: root.sansFamily
                            font.pixelSize: 11
                        }

                        IconAction {
                            id: taskCancel
                            anchors.right: parent.right
                            anchors.top: parent.top
                            anchors.topMargin: -6
                            implicitWidth: 22
                            implicitHeight: 22
                            glyph: "✕"
                            hint: "Cancel"
                            onActivated: root.bridge.cancelTask(root.bridge.taskId(taskRow.slot))
                        }

                        Mono {
                            anchors.left: parent.left
                            anchors.top: taskTitle.bottom
                            anchors.topMargin: 2
                            text: {
                                void root.revision
                                return root.bridge.taskStatus(taskRow.slot)
                            }
                            font.pixelSize: 10
                        }

                        Rectangle {
                            anchors.left: parent.left
                            anchors.right: parent.right
                            anchors.bottom: parent.bottom
                            height: 3
                            radius: 2
                            color: root.raisedHigh

                            Rectangle {
                                readonly property int progress: {
                                    void root.revision
                                    return root.bridge.taskProgress(taskRow.slot)
                                }
                                height: parent.height
                                radius: 2
                                width: parent.width * Math.max(0, Math.min(100, progress)) / 100
                                color: root.spectral
                            }
                        }
                    }
                }
            }
        }
    }

    // ========================= ADD A REMOTE ==============================
    // rclone asks one question at a time, and so does this. Every word here
    // except the frame is rclone's own — the wording, the suggested answers
    // and the authorisation link — which is why a storage provider rclone
    // gains later needs nothing added here.
    Rectangle {
        id: remoteSheet
        anchors.fill: parent
        visible: root.remoteSheetOpen
        color: "#CC0A0B0C"
        z: 60

        // Nothing underneath is reachable while this is up.
        MouseArea {
            anchors.fill: parent
            hoverEnabled: true
            onClicked: {}
        }

        Rectangle {
            id: sheetCard
            anchors.centerIn: parent
            width: Math.min(600, parent.width - 72)
            height: Math.min(620, parent.height - 72)
            radius: 12
            color: root.raised
            border.width: 1
            border.color: root.lineStrong

            // Whether rclone is waiting on an answer rather than on a choice
            // of provider.
            readonly property bool asking: {
                void root.configTick
                return root.bridge.configActive()
            }
            readonly property string failure: {
                void root.configTick
                void root.revision
                return root.bridge.lastError
            }
            readonly property var providers: {
                void root.configTick
                const total = root.bridge.providerCount()
                const needle = root.providerQuery.toLowerCase()
                const out = []
                for (let i = 0; i < total; ++i) {
                    const label = root.bridge.providerLabel(i)
                    const kind = root.bridge.providerKind(i)
                    if (needle.length === 0
                        || label.toLowerCase().indexOf(needle) >= 0
                        || kind.toLowerCase().indexOf(needle) >= 0)
                        out.push({ label: label, kind: kind })
                }
                return out
            }
            // Everything rclone is currently asking. These are plain calls into
            // the backend, so they only re-read when `configTick` says the
            // conversation has moved; without that they would answer once, at
            // creation, and never again.
            readonly property string field: {
                void root.configTick
                return root.bridge.configField()
            }
            readonly property string help: {
                void root.configTick
                return root.bridge.configHelp()
            }
            readonly property string link: {
                void root.configTick
                return root.bridge.configLink()
            }
            readonly property string fallback: {
                void root.configTick
                return root.bridge.configDefault()
            }
            readonly property bool masked: {
                void root.configTick
                return root.bridge.configSecret()
            }
            readonly property bool waiting: {
                void root.configTick
                void root.revision
                return root.bridge.configWaiting()
            }
            readonly property var choices: {
                void root.configTick
                const total = root.bridge.configChoiceCount()
                const out = []
                for (let i = 0; i < total; ++i)
                    out.push({ value: root.bridge.configChoiceValue(i),
                               help: root.bridge.configChoiceHelp(i) })
                return out
            }

            function answer(text) {
                root.bridge.answerRemote(text)
                answerField.text = ""
                root.configTick++
                if (!root.bridge.configActive() && root.bridge.lastError.length === 0)
                    root.remoteSheetOpen = false
            }

            function dismiss() {
                root.bridge.cancelRemote()
                root.remoteSheetOpen = false
                root.configTick++
            }

            // ---- Header ----
            Item {
                id: sheetHeader
                anchors.left: parent.left
                anchors.right: parent.right
                anchors.top: parent.top
                anchors.margins: 16
                height: 26

                Text {
                    anchors.left: parent.left
                    anchors.verticalCenter: parent.verticalCenter
                    text: sheetCard.asking ? sheetCard.field : "Add a remote"
                    color: root.ink
                    font.family: root.sansFamily
                    font.pixelSize: 16
                    font.weight: Font.Medium
                }

                IconAction {
                    anchors.right: parent.right
                    anchors.verticalCenter: parent.verticalCenter
                    glyph: "✕"
                    hint: "Close"
                    onActivated: sheetCard.dismiss()
                }
            }

            Rectangle {
                id: sheetRule
                anchors.left: parent.left
                anchors.right: parent.right
                anchors.top: sheetHeader.bottom
                anchors.topMargin: 12
                height: 1
                color: root.line
            }

            // ---- Choosing a provider ----
            Item {
                anchors.left: parent.left
                anchors.right: parent.right
                anchors.top: sheetRule.bottom
                anchors.bottom: sheetFooter.top
                anchors.margins: 16
                visible: !sheetCard.asking

                Overline {
                    id: nameLabel
                    anchors.left: parent.left
                    anchors.top: parent.top
                    text: "NAME"
                }

                Field {
                    id: nameField
                    anchors.left: parent.left
                    anchors.right: parent.right
                    anchors.top: nameLabel.bottom
                    anchors.topMargin: 6
                    placeholder: "gdrive"
                    text: root.remoteDraftName
                    onTextChanged: root.remoteDraftName = text
                }

                Overline {
                    id: storageLabel
                    anchors.left: parent.left
                    anchors.top: nameField.bottom
                    anchors.topMargin: 14
                    text: "STORAGE"
                }

                Field {
                    id: providerFilter
                    anchors.left: parent.left
                    anchors.right: parent.right
                    anchors.top: storageLabel.bottom
                    anchors.topMargin: 6
                    placeholder: "Search providers…"
                    text: root.providerQuery
                    onTextChanged: root.providerQuery = text
                }

                ListView {
                    anchors.left: parent.left
                    anchors.right: parent.right
                    anchors.top: providerFilter.bottom
                    anchors.topMargin: 10
                    anchors.bottom: parent.bottom
                    clip: true
                    spacing: 2
                    model: sheetCard.providers
                    boundsBehavior: Flickable.StopAtBounds

                    delegate: Rectangle {
                        id: providerRow
                        required property var modelData
                        width: ListView.view.width
                        height: 34
                        radius: 7
                        color: providerMouse.containsMouse ? root.hover : "transparent"

                        Text {
                            anchors.left: parent.left
                            anchors.leftMargin: 10
                            anchors.right: providerKind.left
                            anchors.rightMargin: 10
                            anchors.verticalCenter: parent.verticalCenter
                            text: providerRow.modelData.label
                            elide: Text.ElideRight
                            color: root.inkMuted
                            font.family: root.sansFamily
                            font.pixelSize: 13
                        }

                        Mono {
                            id: providerKind
                            anchors.right: parent.right
                            anchors.rightMargin: 10
                            anchors.verticalCenter: parent.verticalCenter
                            text: providerRow.modelData.kind
                            font.pixelSize: 10
                            color: root.inkGhost
                        }

                        MouseArea {
                            id: providerMouse
                            anchors.fill: parent
                            hoverEnabled: true
                            cursorShape: Qt.PointingHandCursor
                            onClicked: {
                                if (root.remoteDraftName.length === 0)
                                    return
                                root.bridge.beginRemote(root.remoteDraftName,
                                                          providerRow.modelData.kind)
                                root.configTick++
                                // A provider that asks nothing is already done.
                                if (!root.bridge.configActive()
                                    && root.bridge.lastError.length === 0)
                                    root.remoteSheetOpen = false
                            }
                        }
                    }
                }
            }

            // ---- Answering rclone ----
            Flickable {
                id: questionArea
                anchors.left: parent.left
                anchors.right: parent.right
                anchors.top: sheetRule.bottom
                anchors.bottom: sheetFooter.top
                anchors.margins: 16
                visible: sheetCard.asking
                clip: true
                contentHeight: questionColumn.implicitHeight
                boundsBehavior: Flickable.StopAtBounds

                Column {
                    id: questionColumn
                    width: questionArea.width
                    spacing: 12

                    Text {
                        width: parent.width
                        text: sheetCard.help
                        visible: text.length > 0
                        wrapMode: Text.WordWrap
                        color: root.inkMuted
                        font.family: root.sansFamily
                        font.pixelSize: 13
                        lineHeight: 1.25
                    }

                    // Sign-in happens in the user's own browser. It is opened
                    // for them as soon as the link exists; the link stays here
                    // selectable, and openable again, in case it did not.
                    Column {
                        width: parent.width
                        spacing: 8
                        visible: sheetCard.waiting || sheetCard.link.length > 0

                        Text {
                            width: parent.width
                            visible: sheetCard.waiting
                            text: sheetCard.link.length > 0
                                  ? "Waiting for you to finish signing in. The browser should have opened."
                                  : "Starting sign-in…"
                            wrapMode: Text.WordWrap
                            color: root.inkMuted
                            font.family: root.sansFamily
                            font.pixelSize: 13
                        }

                        Rectangle {
                            width: parent.width
                            visible: sheetCard.link.length > 0
                            height: linkText.implicitHeight + 18
                            radius: 7
                            color: root.inset
                            border.width: 1
                            border.color: root.emberEdge

                            TextEdit {
                                id: linkText
                                anchors.left: parent.left
                                anchors.right: openLink.left
                                anchors.margins: 10
                                anchors.verticalCenter: parent.verticalCenter
                                text: sheetCard.link
                                readOnly: true
                                selectByMouse: true
                                wrapMode: TextEdit.WrapAnywhere
                                color: root.emberInk
                                selectionColor: root.emberWash
                                font.family: root.monoFamily
                                font.pixelSize: 12
                            }

                            SheetButton {
                                id: openLink
                                anchors.right: parent.right
                                anchors.rightMargin: 8
                                anchors.verticalCenter: parent.verticalCenter
                                label: "Open"
                                onActivated: root.bridge.openLink(sheetCard.link)
                            }
                        }
                    }

                    // rclone's own suggested answers, each one a single click.
                    Column {
                        width: parent.width
                        spacing: 4
                        visible: sheetCard.choices.length > 0

                        Repeater {
                            model: sheetCard.choices

                            delegate: Rectangle {
                                id: choiceRow
                                required property var modelData
                                width: questionColumn.width
                                // rclone writes these descriptions over several
                                // lines, so the row is measured, never fixed.
                                height: choiceBody.implicitHeight + 16
                                radius: 7
                                color: choiceMouse.containsMouse ? root.hover : root.inset
                                border.width: 1
                                border.color: root.line

                                Column {
                                    id: choiceBody
                                    anchors.left: parent.left
                                    anchors.right: parent.right
                                    anchors.margins: 10
                                    anchors.verticalCenter: parent.verticalCenter
                                    spacing: 3

                                    Mono {
                                        text: choiceRow.modelData.value
                                        font.pixelSize: 11
                                        color: root.ink
                                    }

                                    Text {
                                        width: parent.width
                                        visible: text.length > 0
                                        text: choiceRow.modelData.help
                                        wrapMode: Text.WordWrap
                                        maximumLineCount: 4
                                        elide: Text.ElideRight
                                        color: root.inkFaint
                                        font.family: root.sansFamily
                                        font.pixelSize: 12
                                        lineHeight: 1.2
                                    }
                                }

                                MouseArea {
                                    id: choiceMouse
                                    anchors.fill: parent
                                    hoverEnabled: true
                                    cursorShape: Qt.PointingHandCursor
                                    onClicked: sheetCard.answer(choiceRow.modelData.value)
                                }
                            }
                        }
                    }

                    Field {
                        id: answerField
                        width: parent.width
                        masked: sheetCard.masked
                        placeholder: sheetCard.fallback.length > 0 ? sheetCard.fallback : "Your answer"
                        onAccepted: sheetCard.answer(answerField.text)
                    }
                }
            }

            // ---- Footer ----
            Item {
                id: sheetFooter
                anchors.left: parent.left
                anchors.right: parent.right
                anchors.bottom: parent.bottom
                anchors.margins: 16
                height: 30

                Text {
                    anchors.left: parent.left
                    anchors.right: footerButtons.left
                    anchors.rightMargin: 12
                    anchors.verticalCenter: parent.verticalCenter
                    text: sheetCard.failure
                    visible: sheetCard.failure.length > 0
                    elide: Text.ElideRight
                    color: root.destructive
                    font.family: root.sansFamily
                    font.pixelSize: 12
                }

                Row {
                    id: footerButtons
                    anchors.right: parent.right
                    anchors.verticalCenter: parent.verticalCenter
                    spacing: 8

                    SheetButton {
                        label: "Cancel"
                        onActivated: sheetCard.dismiss()
                    }

                    SheetButton {
                        label: sheetCard.asking ? "Continue" : "Choose a provider"
                        primary: true
                        // Naming comes first: rclone writes the remote under
                        // this name and will not ask for it.
                        enabled: sheetCard.asking || root.remoteDraftName.length > 0
                        onActivated: {
                            if (sheetCard.asking)
                                sheetCard.answer(answerField.text)
                        }
                    }
                }
            }
        }

        // Read the provider list whenever the sheet comes up, and once at
        // creation: a sheet that is already visible when it is built would
        // otherwise never see a visibility change and stay empty.
        Component.onCompleted: remoteSheet.refresh()
        onVisibleChanged: if (visible) remoteSheet.refresh()

        function refresh() {
            root.bridge.loadProviders()
            root.configTick++
        }
    }
}
