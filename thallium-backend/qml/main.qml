pragma ComponentBehavior: Bound

import QtQuick
import QtQuick.Controls
import QtQuick.Shapes
import QtQuick.Effects
import org.thallium.FileManager 1.0

// Thallium Files.
//
// Drawn in the Thallium palette the rest of the desktop uses (see Theme):
// surfaces step in lightness, state is an accent wash with a full accent
// outline, and the only ornament is the cross-ended hairline.
ApplicationWindow {
    id: root

    // The palette lives in Theme. These names are what the rest of this
    // file was written against: the old spectral and ember roles are the
    // Thallium accent now, and destructive is the palette's red.
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

    readonly property color spectral: Theme.accent
    readonly property color spectralDim: Theme.accentWash

    readonly property color ember: Theme.accent
    readonly property color emberInk: Theme.ink
    readonly property color emberWash: Theme.accentWash
    readonly property color emberEdge: Theme.outline
    readonly property color destructive: Theme.danger

    readonly property string sansFamily: Theme.family
    readonly property string monoFamily: Theme.mono

    // Follow the Thallium theme picked in Settings. The file is a few bytes,
    // so re-reading it each second is cheaper than a watcher's plumbing.
    Timer {
        interval: 1000
        repeat: true
        running: true
        triggeredOnStart: true
        onTriggered: Theme.conf = root.bridge.themeConf()
    }

    readonly property int unit: 4
    // Below these window widths a column folds away so the file pane keeps
    // room for names; tiled at half a screen, only the rail stays.
    readonly property bool roomForPreview: root.width >= 1240
    // The rail shows icons; opened, their names too.
    property bool railOpen: true
    readonly property int sidebarWidth: root.railOpen ? Theme.railOpenWidth : Theme.railWidth
    // In a narrow file pane the Kind and then the Size column give their
    // width to the name.
    readonly property bool kindColumn: main.width >= 760
    readonly property bool sizeColumn: main.width >= 520
    // Every control on the header line is this tall, and every band
    // in the folder bar is the next one. Mixing 30 with 34 is what
    // made the top of the window look ragged.
    readonly property int controlHeight: 34
    readonly property int bandHeight: 52
    readonly property int inspectorWidth: Theme.previewWidth + 10
    readonly property int rowHeight: root.view === "dense" ? Theme.rowHeightDense : 44

    property string view: "list"
    property bool searchShown: false
    readonly property bool bookmarked: {
        void root.revision
        for (let b = 0; b < root.bridge.bookmarkCount(); ++b)
            if (root.bridge.bookmarkPath(b) === root.bridge.currentPath)
                return true
        return false
    }

    function openSearch() {
        root.searchShown = true
        searchInput.forceActiveFocus()
    }
    property string query: ""
    property bool showHidden: false
    // The preview switch remembers one choice for wide windows (open by
    // default) and one for narrow ones (closed by default).
    property bool previewWide: true
    property bool previewNarrow: false
    readonly property bool previewWanted: root.roomForPreview ? root.previewWide : root.previewNarrow
    readonly property bool previewOpen: root.previewWanted && !root.settingsOpen
    function togglePreview() {
        if (root.roomForPreview)
            root.previewWide = !root.previewWide
        else
            root.previewNarrow = !root.previewNarrow
    }
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

    // The path as crumbs. Under home it starts at Home rather than at the
    // root, which is how people think of their own files.
    readonly property var crumbs: {
        const path = root.bridge.currentPath
        const home = root.bridge.homePath
        if (!path || path === "/")
            return [{ label: "Filesystem", path: "/" }]
        let out, walked, rest
        if (home.length > 0 && (path === home || path.startsWith(home + "/"))) {
            out = [{ label: "Home", path: home }]
            walked = home
            rest = path.substring(home.length)
        } else {
            out = [{ label: "Filesystem", path: "/" }]
            walked = ""
            rest = path
        }
        const parts = rest.split("/").filter(function (part) { return part.length > 0 })
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
        // The first group has no heading: the Places tab above names it.
        const out = [ { label: "Home", folder: "", icon: "house", section: "" },
                      { label: "Recent", folder: null, recent: true, icon: "clock", section: "" } ]
        for (let i = 0; i < known.length; ++i) {
            if (seen.length === 0 || seen.indexOf(known[i]) >= 0)
                out.push({ label: known[i], folder: known[i], icon: icons[known[i]], section: "" })
        }
        // A file dialog only needs places to pick from.
        if (!root.picking)
            out.push({ label: "Trash", folder: null, trash: true, icon: "trash-2", section: "" })
        out.push({ label: "Filesystem", folder: "/", icon: "hard-drive", section: "Locations" })
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
            if (listed.indexOf(path) < 0) {
                // Pinned folders sit with the places, before Trash.
                const at = out.findIndex(function (place) { return place.trash === true || place.section !== "" })
                out.splice(at < 0 ? out.length : at, 0, { label: root.bridge.bookmarkLabel(b), folder: null, target: path,
                                                           bookmark: b, icon: "folder", section: "" })
            }
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

    function pad2(n) { return n < 10 ? "0" + n : String(n) }

    // The sidebar's head: today's date, and user@host, read at run time.
    property string today: ""
    readonly property string whoami: root.bridge.userName() + "@" + root.bridge.hostName()
    Timer {
        interval: 30000
        repeat: true
        running: true
        triggeredOnStart: true
        onTriggered: root.today = Qt.formatDate(new Date(), "dddd, MMMM d").toUpperCase()
    }

    // The place the open folder lies in: the deepest local place whose
    // folder holds it. Filesystem is never a place to expand.
    readonly property string activePlace: {
        void root.revision
        const path = root.bridge.currentPath
        let best = ""
        for (let i = 0; i < root.places.length; ++i) {
            const place = root.places[i]
            if (place.section !== "" || place.recent === true || place.trash === true)
                continue
            const target = place.bookmark !== undefined ? place.target : root.placePath(place.folder)
            if (!target || target === "/")
                continue
            if ((path === target || path.startsWith(target + "/")) && target.length > best.length)
                best = target
        }
        return best
    }
    // Its folders, and how many entries it holds.
    readonly property var placeTree: {
        void root.revision
        if (root.activePlace.length === 0)
            return []
        const text = root.bridge.subfolders(root.activePlace, root.showHidden)
        if (text.length === 0)
            return []
        return text.split("\n").map(function (line) {
            const tab = line.lastIndexOf("\t")
            return { name: line.substring(0, tab), count: parseInt(line.substring(tab + 1)) }
        })
    }
    readonly property int placeTotal: {
        void root.revision
        return root.activePlace.length > 0 ? root.bridge.entryTotal(root.activePlace, root.showHidden) : -1
    }
    // How many tree rows the lit guide runs through: down to the row on the
    // open folder's path, or none.
    readonly property int placeTreeLit: {
        const path = root.bridge.currentPath
        for (let i = 0; i < root.placeTree.length; ++i) {
            const branch = root.activePlace + "/" + root.placeTree[i].name
            if (path === branch || path.startsWith(branch + "/"))
                return i + 1
        }
        return 0
    }
    readonly property bool storageKnown: {
        void root.revision
        return root.bridge.storageKnown()
    }
    readonly property real storageShare: {
        void root.revision
        return root.bridge.storageShare()
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
            { label: "Folders", count: buckets.Folders },
            { label: "Code", count: buckets.Code },
            { label: "Documents", count: buckets.Documents },
            { label: "Media", count: buckets.Media },
            { label: "Other", count: buckets.Other + buckets.Archives }
        ]
    }

    width: 1480
    height: 920
    minimumWidth: 560
    minimumHeight: 640
    visible: true
    color: canvas
    title: root.picking ? root.pickTitle : leafName(root.bridge.currentPath) + " — Thallium Files"

    // ---- Picking ----
    // `--pick open|open-multiple|folder|save` turns the window into a file
    // dialog for the portal: what is chosen goes to stdout and the app quits.
    property string pickMode: ""
    property string pickTitle: ""
    property string pickAccept: ""
    readonly property bool picking: root.pickMode !== ""
    readonly property bool pickSaving: root.pickMode === "save"
    readonly property bool pickReplaces: {
        void root.revision
        return root.pickSaving && root.rowNamed(pickName.text) >= 0
                && !root.bridge.entryIsDirectory(root.rowNamed(pickName.text))
    }

    function readPickArgs(args) {
        const value = function (flag) {
            const at = args.indexOf(flag)
            return at > 0 && at + 1 < args.length ? args[at + 1] : ""
        }
        root.pickMode = value("--pick")
        if (!root.picking)
            return false
        root.pickAccept = value("--pick-accept")
        root.pickTitle = value("--pick-title") || ({ "save": "Save", "folder": "Choose a folder" })[root.pickMode] || "Open"
        pickName.text = value("--pick-name")
        root.width = 1080
        root.height = 720
        root.bridge.navigate(value("--pick-folder") || root.bridge.homePath)
        if (root.pickSaving)
            pickName.focusInput()
        return true
    }

    function rowNamed(name) {
        if (name.length === 0)
            return -1
        for (let row = 0; row < root.bridge.entryCount(); row++)
            if (root.bridge.entryName(row) === name)
                return row
        return -1
    }

    function fileUri(path) {
        return "file://" + path.split("/").map(encodeURIComponent).join("/")
    }

    function finishPick(uris) {
        root.bridge.pickDone(uris.join("\n"))
        Qt.quit()
    }

    function selectedFiles() {
        const out = []
        for (let row = 0; row < root.bridge.entryCount(); row++)
            if (root.bridge.entrySelected(row) && !root.bridge.entryIsDirectory(row))
                out.push(root.fileUri(root.bridge.entryPath(row)))
        return out
    }

    // The accept button, or Enter on the name. A lone selected folder is
    // entered rather than chosen, except when choosing folders.
    function acceptPick() {
        const row = root.selectedCount === 1 ? root.firstSelectedRow() : -1
        if (root.pickSaving) {
            const name = pickName.text.trim()
            const existing = root.rowNamed(name)
            if (name.length === 0)
                return
            if (existing >= 0 && root.bridge.entryIsDirectory(existing)) {
                root.bridge.navigateRow(existing)
                pickName.text = ""
                return
            }
            const dir = root.bridge.currentPath
            root.finishPick([root.fileUri((dir.endsWith("/") ? dir : dir + "/") + name)])
        } else if (root.pickMode === "folder") {
            root.finishPick([root.fileUri(row >= 0 && root.bridge.entryIsDirectory(row)
                                          ? root.bridge.entryPath(row) : root.bridge.currentPath)])
        } else if (row >= 0 && root.bridge.entryIsDirectory(row)) {
            root.bridge.navigateRow(row)
        } else {
            const files = root.selectedFiles()
            if (files.length > 0)
                root.finishPick(root.pickMode === "open" ? files.slice(0, 1) : files)
        }
    }

    // Choosing a file: Save takes its name, Open takes the file.
    function pickRow(row) {
        if (root.pickSaving) {
            pickName.text = root.bridge.entryName(row)
            pickName.focusInput()
        } else if (root.pickMode !== "folder") {
            root.finishPick([root.fileUri(root.bridge.entryPath(row))])
        }
    }

    function leafName(path) {
        if (!path || path === "/")
            return "Filesystem"
        const clean = path.endsWith("/") ? path.slice(0, -1) : path
        const parts = clean.split("/")
        return parts[parts.length - 1] || "/"
    }

    // Go to an entry of `places`: a folder, drive, bookmark or remote.
    function openPlace(place) {
        if (place.add === true) {
            root.remoteDraftName = ""
            root.providerQuery = ""
            root.remoteSheetOpen = true
            return
        }
        root.settingsOpen = false
        root.recentOpen = false
        root.trashOpen = false
        if (place.drive !== undefined)
            root.bridge.openDrive(place.drive)
        else
            root.bridge.navigate(place.folder === null ? place.target : root.placePath(place.folder))
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

    // Settings, shown in place of the folder. Going anywhere closes it.
    property bool settingsOpen: false
    property bool defaultManager: true

    function openSettings() {
        root.defaultManager = root.bridge.isDefaultManager()
        root.settingsOpen = true
    }
    Connections {
        target: root.bridge
        function onCurrentPathChanged() { root.settingsOpen = false }
    }

    // The desktop's recent files, shown in place of a directory.
    property bool recentOpen: false

    function showRecent() {
        root.endDeepSearch()
        root.settingsOpen = false
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
        root.settingsOpen = false
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

    // The Thallium 81 type tag: DIR, CODE, DOC, or TXT for plain text and
    // everything else.
    function tagOf(name, folder) {
        const kind = root.kindOf(name, folder)
        if (kind === "Folders")
            return "DIR"
        if (kind === "Code")
            return "CODE"
        if (kind === "Documents") {
            const extension = name.slice(name.lastIndexOf(".") + 1).toLowerCase()
            return extension === "txt" || extension === "md" ? "TXT" : "DOC"
        }
        return "TXT"
    }

    // The Kind column: what a person would call the file.
    function kindLabel(name, folder) {
        if (folder)
            return "Folder"
        const dot = name.lastIndexOf(".")
        const extension = dot > 0 ? name.slice(dot + 1).toLowerCase() : ""
        const named = {
            txt: "Plain text", md: "Markdown", pdf: "PDF document", odt: "Document", doc: "Document",
            docx: "Document", rtf: "Rich text", csv: "Spreadsheet", ods: "Spreadsheet", xlsx: "Spreadsheet",
            xls: "Spreadsheet", odp: "Presentation", pptx: "Presentation", json: "JSON data",
            toml: "TOML source", yaml: "YAML source", yml: "YAML source", lock: "Lock file", sh: "Shell script",
            png: "PNG image", jpg: "JPEG image", jpeg: "JPEG image", gif: "GIF image", webp: "WebP image",
            svg: "SVG image", mp4: "Video", mkv: "Video", webm: "Video", mov: "Video", mp3: "Audio",
            flac: "Audio", wav: "Audio", ogg: "Audio", zip: "Archive", tar: "Archive", gz: "Archive",
            xz: "Archive", zst: "Archive", "7z": "Archive", deb: "Package", iso: "Disc image"
        }
        if (named[extension])
            return named[extension]
        if (root.kindOf(name, false) === "Code")
            return extension.toUpperCase() + " source"
        return extension.length > 0 ? extension.toUpperCase() + " file" : "File"
    }

    // Times as the Thallium 81 list writes them: Today, 09:58 · 31 Oct, 18:04.
    function stampText(seconds) {
        if (seconds <= 0)
            return "—"
        const value = new Date(seconds * 1000)
        const now = new Date()
        const day = new Date(now.getFullYear(), now.getMonth(), now.getDate())
        const then = new Date(value.getFullYear(), value.getMonth(), value.getDate())
        const days = Math.round((day - then) / 86400000)
        const time = Qt.formatTime(value, "HH:mm")
        if (days === 0)
            return "Today, " + time
        if (days === 1)
            return "Yesterday, " + time
        if (value.getFullYear() === now.getFullYear())
            return value.getDate() + " " + Qt.formatDate(value, "MMM") + ", " + time
        return value.getDate() + " " + Qt.formatDate(value, "MMM yyyy")
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

    Shortcut { sequences: ["F3"]; onActivated: root.toggleSplit() }

    // ---- Theme ----
    // Follow Thallium, or a custom theme of five colours, plus the user's own
    // presets. Saved shortly after the last change, so dragging in a colour
    // picker does not write the file on every step.
    property var userPresets: []

    function validColors(c) {
        return !!c && Theme.editable.every(function (e) { return Theme.isColor(c[e.key]) })
    }
    function cleanColors(c) {
        const out = {}
        Theme.editable.forEach(function (e) { out[e.key] = String(c[e.key]).toLowerCase() })
        return out
    }
    function loadFilesTheme() {
        let saved = {}
        try {
            saved = JSON.parse(root.bridge.filesTheme() || "{}")
        } catch (error) {
            return
        }
        if (root.validColors(saved.colors))
            Theme.base = root.cleanColors(saved.colors)
        root.userPresets = Array.isArray(saved.presets)
                ? saved.presets.filter(function (p) { return p && typeof p.name === "string" && root.validColors(p.colors) })
                : []
        Theme.custom = saved.custom === true
    }
    function saveFilesTheme() {
        root.bridge.saveFilesTheme(JSON.stringify({ custom: Theme.custom, colors: Theme.base, presets: root.userPresets }, null, 2))
    }
    Timer {
        id: themeSave
        interval: 400
        onTriggered: root.saveFilesTheme()
    }
    function setThemeColors(colors) {
        Theme.base = root.cleanColors(colors)
        themeSave.restart()
    }
    function setThemeColor(key, value) {
        const next = Object.assign({}, Theme.base)
        next[key] = value
        root.setThemeColors(next)
    }
    function setThemeCustom(custom) {
        Theme.custom = custom
        themeSave.restart()
    }
    // Presets are keyed "b<index>" for built-ins and "u<index>" for the user's.
    readonly property var themePresets: {
        const all = Theme.presets.map(function (p, i) { return { key: "b" + i, label: p.name, colors: p.colors } })
        return all.concat(root.userPresets.map(function (p, i) { return { key: "u" + i, label: p.name, colors: p.colors } }))
    }
    readonly property string themePreset: {
        const now = JSON.stringify(root.cleanColors(Theme.base))
        for (let i = 0; i < root.themePresets.length; ++i)
            if (JSON.stringify(root.cleanColors(root.themePresets[i].colors)) === now)
                return root.themePresets[i].key
        return ""
    }
    function saveThemePreset(name) {
        const label = name.trim()
        if (label.length === 0)
            return
        const kept = root.userPresets.filter(function (p) { return p.name !== label })
        root.userPresets = kept.concat([{ name: label, colors: Theme.base }])
        root.saveFilesTheme()
    }
    function deleteThemePreset(key) {
        const at = Number(key.slice(1))
        root.userPresets = root.userPresets.filter(function (p, i) { return i !== at })
        root.saveFilesTheme()
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

    // ---- Split view ----
    // A second bridge shown beside the main pane, for moving files between
    // two folders. It opens on the folder the main pane shows.
    property BackendBridge splitBridge: null
    readonly property bool splitOpen: root.splitBridge !== null

    function toggleSplit() {
        if (root.splitOpen) {
            const closing = root.splitBridge
            root.splitBridge = null
            closing.destroy()
            return
        }
        const side = bridgeFactory.createObject(root)
        side.navigate(root.bridge.currentPath)
        root.splitBridge = side
    }

    // Copy (or move) what is held in the main pane into the split pane's folder.
    function sendToSplit(move) {
        const uris = root.bridge.selectedUris()
        if (uris.length > 0)
            root.bridge.dropUris(uris, root.splitBridge.currentPath, move ? 2 : 1)
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
            root.loadFilesTheme()
            const args = Qt.application.arguments
            if (root.readPickArgs(args))
                return
            firstBridge.serveDesktop()
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
            elide: Text.ElideMiddle
            color: menuLine.enabled ? root.ink : root.inkGhost
            font.family: root.sansFamily
            font.pixelSize: 13
            verticalAlignment: Text.AlignVCenter
        }
        background: Rectangle {
            color: menuLine.highlighted ? root.hover : "transparent"
            radius: 0
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
            radius: 0
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
        // Only with split view open; a hidden menu item still takes room
        // unless its height goes too.
        MenuLine {
            text: "Copy to other pane"
            visible: root.splitOpen
            height: visible ? implicitHeight : 0
            onTriggered: root.sendToSplit(false)
        }
        MenuLine {
            text: "Move to other pane"
            visible: root.splitOpen
            height: visible ? implicitHeight : 0
            onTriggered: root.sendToSplit(true)
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
        onAboutToShow: {
            pasteable = root.bridge.canPaste()
            undoable = root.bridge.canUndo()
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
        MenuLine { text: "New tab"; onTriggered: root.newTab(root.bridge.currentPath) }
        MenuLine { text: root.splitOpen ? "Close split view" : "Split view"; onTriggered: root.toggleSplit() }
        MenuLine { text: "Go to location…"; onTriggered: root.editPath() }
        MenuLine {
            text: root.bookmarked ? "Bookmarked" : "Bookmark this folder"
            enabled: !root.bookmarked
            onTriggered: root.bridge.addBookmark(root.bridge.currentPath)
        }
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

    Sheet {
        id: emptyConfirm
        width: 440
        title: "Empty the Trash?"
        icon: "trash-2"
        onClosed: root.release()

        Text {
            width: parent.width
            wrapMode: Text.Wrap
            text: "Everything in the Trash is deleted permanently."
            color: Theme.inkSoft
            font.family: Theme.family
            font.pixelSize: Theme.text
        }

        buttons: [
            TextButton { text: "Cancel"; onClicked: emptyConfirm.close() },
            TextButton {
                primary: true
                text: "Empty Trash"
                onClicked: {
                    root.bridge.emptyTrash()
                    root.trashPicked = []
                    emptyConfirm.close()
                }
            }
        ]
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

    Sheet {
        id: propertiesSheet
        width: 480
        title: propertiesSheet.text("name")
        icon: propertiesSheet.number("directory") === 1 ? "folder" : "file"
        onClosed: root.release()
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

        Repeater {
            model: [
                ["Type", propertiesSheet.text("type")],
                ["Size", propertiesSheet.number("directory") === 1
                    ? root.readableSize(propertiesSheet.number("size")) + " in "
                      + propertiesSheet.number("files")
                      + (propertiesSheet.number("files") === 1 ? " file" : " files")
                      + (propertiesSheet.number("measuring") === 1 ? " (counting…)" : "")
                    : root.readableSize(propertiesSheet.number("size"))],
                ["Location", propertiesSheet.text("folder")],
                ["Links to", propertiesSheet.text("link")],
                ["Modified", root.modifiedText(propertiesSheet.number("modified"))],
                ["Accessed", root.modifiedText(propertiesSheet.number("accessed"))],
                ["Created", root.modifiedText(propertiesSheet.number("created"))],
                ["Owner", propertiesSheet.text("owner") + " : " + propertiesSheet.text("group")]
            ].filter(function (pair) { return pair[1].length > 0 })
            delegate: Row {
                id: propRow
                required property var modelData
                spacing: 14
                Text {
                    width: 90
                    text: propRow.modelData[0]
                    color: Theme.inkDim
                    font.family: Theme.family
                    font.pixelSize: 12
                }
                Text {
                    width: 310
                    text: propRow.modelData[1]
                    wrapMode: Text.WrapAnywhere
                    color: Theme.ink
                    font.family: Theme.family
                    font.pixelSize: 12
                }
            }
        }

        CrossRule { width: parent.width }

        Row {
            spacing: 14
            Text {
                width: 90
                text: "Access"
                color: Theme.inkDim
                font.family: Theme.family
                font.pixelSize: 12
            }
            Text {
                text: propertiesSheet.text("mode")
                color: Theme.ink
                font.family: Theme.family
                font.pixelSize: 12
            }
        }

        // Read, write and run for the owner, the group and everyone else,
        // applied as they are ticked.
        Grid {
            columns: 4
            columnSpacing: 10
            rowSpacing: 8
            enabled: propertiesSheet.owned
            Repeater {
                model: ["", "Read", "Write", propertiesSheet.number("directory") === 1 ? "Enter" : "Run"]
                delegate: Text {
                    required property var modelData
                    width: modelData.length === 0 ? 94 : 70
                    text: modelData
                    color: Theme.inkDim
                    font.family: Theme.family
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
                        width: index === 0 ? 94 : 70
                        height: 24
                        readonly property int bit: index === 0 ? 0 : (4 >> (index - 1)) << permRow.shift
                        Text {
                            visible: permCell.index === 0
                            anchors.verticalCenter: parent.verticalCenter
                            text: permRow.who
                            color: Theme.ink
                            font.family: Theme.family
                            font.pixelSize: 12
                        }
                        Check {
                            visible: permCell.index > 0
                            anchors.verticalCenter: parent.verticalCenter
                            checked: (propertiesSheet.mode & permCell.bit) !== 0
                            onToggled: function (checked) {
                                root.bridge.setMode(checked ? propertiesSheet.mode | permCell.bit
                                                            : propertiesSheet.mode & ~permCell.bit)
                            }
                        }
                    }
                }
            }
        }
        Text {
            visible: !propertiesSheet.owned
            text: "Only the owner can change permissions."
            color: Theme.inkDim
            font.family: Theme.family
            font.pixelSize: 11
        }

        buttons: [
            TextButton { primary: true; text: "Close"; onClicked: propertiesSheet.close() }
        ]
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
        bulkSheet.modeKey = "replace"
        bulkSheet.start = 1
        bulkFirst.text = ""
        bulkSecond.text = ""
        bulkSheet.update()
        bulkSheet.open()
    }

    Sheet {
        id: bulkSheet
        width: 600
        title: "Rename " + root.selectedCount + " items"
        icon: "pencil"
        onOpened: bulkFirst.forceActiveFocus()
        onClosed: root.release()
        property string modeKey: "replace"
        property int start: 1
        readonly property var modes: [
            { key: "replace", label: "Replace text", first: "Find", second: "Replace with" },
            { key: "number", label: "Number", first: "Name pattern, ### is the number", second: "" },
            { key: "affix", label: "Add text", first: "Before the name", second: "After the name" },
            { key: "lower", label: "lowercase", first: "", second: "" },
            { key: "upper", label: "UPPERCASE", first: "", second: "" },
            { key: "title", label: "Title Case", first: "", second: "" }
        ]
        readonly property var mode: {
            for (let i = 0; i < modes.length; ++i)
                if (modes[i].key === bulkSheet.modeKey)
                    return modes[i]
            return modes[0]
        }
        readonly property string error: {
            void root.bulkTick
            return root.bridge.bulkError()
        }
        function update() {
            root.bridge.bulkRule(bulkSheet.modeKey, bulkFirst.text, bulkSecond.text, bulkSheet.start, bulkCase.checked)
            root.bulkTick++
        }

        Chips {
            width: parent.width
            options: bulkSheet.modes
            current: bulkSheet.modeKey
            onPicked: function (key) {
                bulkSheet.modeKey = key
                bulkSheet.update()
            }
        }

        Row {
            spacing: 8
            visible: bulkSheet.mode.first.length > 0
            Rectangle {
                width: bulkSheet.mode.second.length > 0 ? 268 : (bulkSheet.modeKey === "number" ? 420 : 544)
                height: 40
                radius: Theme.radiusSmall
                color: Theme.surface
                border.width: 1
                border.color: bulkFirst.activeFocus ? Theme.outline : Theme.outlineSoft
                TextInput {
                    id: bulkFirst
                    anchors.fill: parent
                    anchors.leftMargin: 12
                    anchors.rightMargin: 12
                    verticalAlignment: TextInput.AlignVCenter
                    clip: true
                    color: Theme.ink
                    selectionColor: Theme.controlHigh
                    font.family: Theme.family
                    font.pixelSize: Theme.text
                    onTextChanged: bulkSheet.update()
                    Keys.onEscapePressed: bulkSheet.close()
                    Text {
                        anchors.verticalCenter: parent.verticalCenter
                        visible: parent.text.length === 0
                        text: bulkSheet.mode.first
                        color: Theme.inkFaint
                        font: parent.font
                    }
                }
            }
            Rectangle {
                visible: bulkSheet.mode.second.length > 0
                width: 268
                height: 40
                radius: Theme.radiusSmall
                color: Theme.surface
                border.width: 1
                border.color: bulkSecond.activeFocus ? Theme.outline : Theme.outlineSoft
                TextInput {
                    id: bulkSecond
                    anchors.fill: parent
                    anchors.leftMargin: 12
                    anchors.rightMargin: 12
                    verticalAlignment: TextInput.AlignVCenter
                    clip: true
                    color: Theme.ink
                    selectionColor: Theme.controlHigh
                    font.family: Theme.family
                    font.pixelSize: Theme.text
                    onTextChanged: bulkSheet.update()
                    Keys.onEscapePressed: bulkSheet.close()
                    Text {
                        anchors.verticalCenter: parent.verticalCenter
                        visible: parent.text.length === 0
                        text: bulkSheet.mode.second
                        color: Theme.inkFaint
                        font: parent.font
                    }
                }
            }
            // The first number, for numbering.
            Row {
                visible: bulkSheet.modeKey === "number"
                spacing: 4
                anchors.verticalCenter: parent.verticalCenter
                IconButton { icon: "chevron-left"; iconSize: 14; onClicked: { bulkSheet.start = Math.max(0, bulkSheet.start - 1); bulkSheet.update() } }
                Text {
                    width: 52
                    anchors.verticalCenter: parent.verticalCenter
                    horizontalAlignment: Text.AlignHCenter
                    text: "from " + bulkSheet.start
                    color: Theme.ink
                    font.family: Theme.family
                    font.pixelSize: 12
                }
                IconButton { icon: "chevron-right"; iconSize: 14; onClicked: { bulkSheet.start += 1; bulkSheet.update() } }
            }
        }

        Check {
            id: bulkCase
            visible: bulkSheet.modeKey === "replace"
            text: "Ignore case"
            onToggled: bulkSheet.update()
        }

        CrossRule { width: parent.width }

        ListView {
            width: parent.width
            height: 220
            clip: true
            spacing: 4
            boundsBehavior: Flickable.StopAtBounds
            model: {
                void root.bulkTick
                return root.bridge.bulkCount()
            }
            delegate: Row {
                required property int index
                spacing: 12
                Text {
                    width: 250
                    elide: Text.ElideMiddle
                    text: {
                        void root.bulkTick
                        return root.bridge.bulkOld(index)
                    }
                    color: Theme.inkDim
                    font.family: Theme.family
                    font.pixelSize: 12
                }
                Icon { name: "arrow-right"; size: 13; shade: 0.4 }
                Text {
                    width: 250
                    elide: Text.ElideMiddle
                    text: {
                        void root.bulkTick
                        return root.bridge.bulkNew(index)
                    }
                    color: Theme.ink
                    font.family: Theme.family
                    font.pixelSize: 12
                }
            }
        }

        Text {
            width: parent.width
            visible: bulkSheet.error.length > 0
            text: bulkSheet.error
            wrapMode: Text.Wrap
            color: Theme.inkSoft
            font.family: Theme.family
            font.pixelSize: 12
        }

        buttons: [
            TextButton { text: "Cancel"; onClicked: bulkSheet.close() },
            TextButton {
                primary: true
                text: "Rename"
                enabled: bulkSheet.error.length === 0
                onClicked: {
                    root.bridge.bulkApply()
                    bulkSheet.close()
                }
            }
        ]
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
    // "Other application…": every installed application, filtered as you
    // type, with the choice of making it the default for the type.
    Sheet {
        id: appPicker
        width: 460
        title: "Open with"
        icon: "external-link"
        onOpened: appFilter.forceActiveFocus()
        onClosed: root.release()
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

        Rectangle {
            width: parent.width
            height: 40
            radius: Theme.radiusSmall
            color: Theme.surface
            border.width: 1
            border.color: appFilter.activeFocus ? Theme.outline : Theme.outlineSoft
            TextInput {
                id: appFilter
                anchors.fill: parent
                anchors.leftMargin: 12
                anchors.rightMargin: 12
                verticalAlignment: TextInput.AlignVCenter
                clip: true
                color: Theme.ink
                selectionColor: Theme.controlHigh
                selectedTextColor: Theme.ink
                font.family: Theme.family
                font.pixelSize: Theme.text
                Keys.onReturnPressed: if (appPicker.matches.length > 0) appPicker.choose(appPicker.matches[Math.max(0, appList.currentIndex)])
                Keys.onEnterPressed: if (appPicker.matches.length > 0) appPicker.choose(appPicker.matches[Math.max(0, appList.currentIndex)])
                Keys.onEscapePressed: appPicker.close()
                Keys.onDownPressed: appList.incrementCurrentIndex()
                Keys.onUpPressed: appList.decrementCurrentIndex()
                Text {
                    anchors.verticalCenter: parent.verticalCenter
                    visible: parent.text.length === 0
                    text: "Find an application"
                    color: Theme.inkFaint
                    font: parent.font
                }
            }
        }

        ListView {
            id: appList
            width: parent.width
            height: 300
            clip: true
            spacing: 2
            model: appPicker.matches
            currentIndex: 0
            boundsBehavior: Flickable.StopAtBounds
            delegate: Rectangle {
                id: appRow
                required property int index
                required property var modelData
                width: ListView.view.width
                height: 38
                radius: Theme.radiusSmall
                color: ListView.isCurrentItem ? Theme.selected : (appMouse.containsMouse ? Theme.surfaceHigh : "transparent")
                border.width: ListView.isCurrentItem ? 1 : 0
                border.color: Theme.outline
                Text {
                    anchors.left: parent.left
                    anchors.leftMargin: 12
                    anchors.verticalCenter: parent.verticalCenter
                    text: root.bridge.offeredName(appRow.modelData)
                    color: Theme.ink
                    font.family: Theme.family
                    font.pixelSize: Theme.text
                }
                MouseArea {
                    id: appMouse
                    anchors.fill: parent
                    hoverEnabled: true
                    onClicked: appList.currentIndex = appRow.index
                    onDoubleClicked: appPicker.choose(appRow.modelData)
                }
            }
        }
        Check {
            id: appDefault
            text: "Always use for " + root.bridge.offeredType()
        }

        buttons: [
            TextButton { text: "Cancel"; onClicked: appPicker.close() },
            TextButton {
                primary: true
                text: "Open"
                enabled: appPicker.matches.length > 0
                onClicked: appPicker.choose(appPicker.matches[Math.max(0, appList.currentIndex)])
            }
        ]
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

    Sheet {
        id: namePrompt
        property string mode: "rename"
        property int row: -1
        property string template: ""
        width: 440
        title: namePrompt.mode === "rename" ? "Rename"
             : namePrompt.mode === "bookmark" ? "Bookmark name"
             : namePrompt.mode === "compress" ? "Compress"
             : namePrompt.mode === "folder" ? "New folder" : "New file"
        icon: namePrompt.mode === "rename" ? "pencil"
            : namePrompt.mode === "bookmark" ? "bookmark"
            : namePrompt.mode === "compress" ? "file-archive"
            : namePrompt.mode === "folder" ? "folder-plus" : "file-plus"
        onOpened: promptField.forceActiveFocus()
        onClosed: root.release()

        function accept() {
            const name = promptField.text.trim()
            if (name.length === 0 || name.indexOf("/") >= 0)
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

        Text {
            visible: namePrompt.mode === "compress"
            text: "The ending picks the format: .zip, .tar.gz, .tar.xz or .tar.zst"
            color: Theme.inkDim
            font.family: Theme.family
            font.pixelSize: 12
        }
        Rectangle {
            width: parent.width
            height: 40
            radius: Theme.radiusSmall
            color: Theme.surface
            border.width: 1
            border.color: promptField.activeFocus ? Theme.outline : Theme.outlineSoft
            TextInput {
                id: promptField
                anchors.fill: parent
                anchors.leftMargin: 12
                anchors.rightMargin: 12
                verticalAlignment: TextInput.AlignVCenter
                clip: true
                color: Theme.ink
                selectionColor: Theme.controlHigh
                selectedTextColor: Theme.ink
                font.family: Theme.family
                font.pixelSize: Theme.text
                Keys.onReturnPressed: namePrompt.accept()
                Keys.onEnterPressed: namePrompt.accept()
                Keys.onEscapePressed: namePrompt.close()
            }
        }
        Text {
            visible: promptField.text.indexOf("/") >= 0
            text: "A name cannot contain “/”."
            color: Theme.inkSoft
            font.family: Theme.family
            font.pixelSize: 12
        }

        buttons: [
            TextButton { text: "Cancel"; onClicked: namePrompt.close() },
            TextButton {
                primary: true
                enabled: promptField.text.trim().length > 0 && promptField.text.indexOf("/") < 0
                text: namePrompt.mode === "rename" || namePrompt.mode === "bookmark" ? "Rename"
                    : namePrompt.mode === "compress" ? "Compress" : "Create"
                onClicked: namePrompt.accept()
            }
        ]
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
        else if (root.picking)
            root.pickRow(row)
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
            else if (root.picking)
                root.pickRow(row)
            else
                root.bridge.openEntry(row)
            return
        }
        if (root.picking) {
            root.acceptPick()
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

    readonly property bool typing: root.settingsOpen || namePrompt.opened || pathEditor.visible || deleteConfirm.opened
                                   || emptyConfirm.opened || appPicker.opened || propertiesSheet.opened
                                   || bulkSheet.opened || pickName.editing

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
            // A file dialog with nothing left to clear is cancelled.
            if (root.picking && root.selectedCount === 0 && root.query === "" && !root.searchShown) {
                Qt.quit()
                return
            }
            // A filter left behind after focus moved on is still cleared.
            if (root.deepSearch)
                root.endDeepSearch()
            root.query = ""
            root.searchShown = false
            root.bridge.clearSelection()
            root.typed = ""
            root.release()
        }
    }
    Shortcut { sequences: ["Delete"]; enabled: !root.typing && root.selectedCount > 0; onActivated: root.bridge.trashSelected() }
    Shortcut { sequences: ["Shift+Delete"]; enabled: !root.typing && root.selectedCount > 0; onActivated: deleteConfirm.open() }
    Shortcut { sequences: ["Ctrl+L", "F6"]; enabled: !root.typing; onActivated: root.editPath() }
    Shortcut { sequences: [StandardKey.Find, "Ctrl+K"]; enabled: !root.typing; onActivated: root.openSearch() }
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
    Shortcut { sequences: ["Ctrl+B"]; enabled: !root.typing; onActivated: root.railOpen = !root.railOpen }
    Shortcut { sequences: ["Ctrl+1"]; onActivated: root.view = "list" }
    Shortcut { sequences: ["Ctrl+2"]; onActivated: root.view = "grid" }

    // Permanent deletion cannot be undone, so it always asks.
    Sheet {
        id: deleteConfirm
        width: 440
        title: "Delete permanently?"
        icon: "trash-2"
        onOpened: deleteYes.forceActiveFocus()
        onClosed: root.release()

        Text {
            width: parent.width
            wrapMode: Text.Wrap
            text: root.selectedCount === 1
                ? "“" + root.bridge.entryName(root.firstSelectedRow()) + "” will be deleted, not moved to the Trash."
                : root.selectedCount + " items will be deleted, not moved to the Trash."
            color: Theme.inkSoft
            font.family: Theme.family
            font.pixelSize: Theme.text
        }
        Text {
            text: "This cannot be undone."
            color: Theme.inkDim
            font.family: Theme.family
            font.pixelSize: 12
        }

        buttons: [
            TextButton {
                text: "Cancel"
                onClicked: deleteConfirm.close()
                Keys.onEscapePressed: deleteConfirm.close()
            },
            TextButton {
                id: deleteYes
                primary: true
                text: "Delete"
                onClicked: {
                    root.bridge.deleteSelected()
                    deleteConfirm.close()
                }
                Keys.onEscapePressed: deleteConfirm.close()
            }
        ]
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
            if (root.splitOpen)
                root.splitBridge.poll()
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

    // A bordered single-line input, used by the remote sheet.
    component Field: Rectangle {
        id: field
        property alias text: fieldInput.text
        property string placeholder: ""
        property bool masked: false
        signal accepted
        readonly property bool editing: fieldInput.activeFocus

        function focusInput() {
            fieldInput.forceActiveFocus()
            fieldInput.selectAll()
        }

        implicitHeight: 32
        radius: 0
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
        radius: 0
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

    // A chart key: swatch, name, and the figure at the right.
    component Legend: Row {
        required property var modelData
        spacing: 8
        Rectangle {
            anchors.verticalCenter: parent.verticalCenter
            width: 10
            height: 10
            radius: 0
            color: parent.modelData.color
            border.width: 1
            border.color: Theme.outlineSoft
        }
        Text {
            anchors.verticalCenter: parent.verticalCenter
            width: 76
            text: parent.modelData.label
            color: Theme.inkSoft
            font.family: Theme.family
            font.pixelSize: 12
        }
        Text {
            anchors.verticalCenter: parent.verticalCenter
            text: parent.modelData.value
            color: Theme.ink
            font.family: Theme.mono
            font.pixelSize: 11
        }
    }

    component Overline: Text {
        color: root.inkGhost
        font.family: Theme.mono
        font.pixelSize: 10
        font.weight: Font.Medium
        font.letterSpacing: Theme.letterSpace
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
        radius: 0
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
        radius: 0
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
        radius: 0
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


    // A rail entry: its icon, and its name beside it while the rail is
    // open. The current one is filled and outlined all the way round.
    component NavItem: Rectangle {
        id: navItem
        property string glyph: ""
        property string icon: ""
        property string label: ""
        property string trailing: ""
        // Right-aligned mono readout: a count, LINKED, a share.
        property string meta: ""
        property color metaColor: Theme.cMuted
        property bool accentLabel: false
        property bool active: false
        // A segment bar under the label (the Filesystem's fill), 0..1; -1 for none.
        property real share: -1
        signal activated
        signal menuRequested
        // A folder this item names, so files can be dropped on it.
        property string dropTarget: ""

        width: parent ? parent.width : 0
        implicitHeight: navItem.share >= 0 && root.railOpen ? 50 : 36
        radius: 0
        color: navItem.active ? Theme.cWash : (navMouse.containsMouse ? Theme.cDim : "transparent")

        Behavior on color { ColorAnimation { duration: Theme.tFast } }

        // The selected place carries a 2px accent bar on its left edge.
        Rectangle {
            anchors.left: parent.left
            anchors.top: parent.top
            anchors.bottom: parent.bottom
            width: 2
            visible: navItem.active
            color: Theme.cGreen
        }

        Item {
            id: navGlyph
            anchors.left: parent.left
            anchors.leftMargin: root.railOpen ? 12 : (navItem.width - width) / 2
            anchors.top: parent.top
            anchors.topMargin: (36 - height) / 2
            width: 16
            height: 16
            Icon {
                visible: navItem.icon.length > 0
                anchors.centerIn: parent
                name: navItem.icon.length > 0 ? navItem.icon : "folder"
                size: 15
                shade: navItem.active ? 1 : (navMouse.containsMouse ? 0.9 : 0.7)
                tint: navItem.active || navItem.accentLabel ? Theme.cGreen : "transparent"
            }
            Text {
                visible: navItem.icon.length === 0
                anchors.centerIn: parent
                text: navItem.glyph
                font.family: Theme.fontMono
                font.pixelSize: 14
                color: navItem.active || navItem.accentLabel ? Theme.cGreen : Theme.cMuted
            }
        }

        Text {
            id: navLabel
            visible: root.railOpen
            anchors.left: navGlyph.right
            anchors.leftMargin: 12
            anchors.right: navMeta.visible ? navMeta.left : parent.right
            anchors.rightMargin: 10
            anchors.verticalCenter: navGlyph.verticalCenter
            text: navItem.label
            elide: Text.ElideRight
            color: navItem.accentLabel ? Theme.cGreen : navItem.active ? Theme.cFg : Theme.cFg
            font.family: Theme.fontHuman
            font.pixelSize: 13
            font.weight: navItem.active ? Font.Medium : Font.Normal
        }
        Text {
            id: navMeta
            visible: root.railOpen && navItem.meta.length > 0
            anchors.right: parent.right
            anchors.rightMargin: 12
            anchors.verticalCenter: navGlyph.verticalCenter
            text: navItem.meta
            color: navItem.metaColor
            font.family: Theme.fontMono
            font.pixelSize: 11
            font.letterSpacing: 1.5
        }
        SegmentBar {
            visible: root.railOpen && navItem.share >= 0
            anchors.left: navLabel.left
            anchors.right: parent.right
            anchors.rightMargin: 14
            anchors.top: navGlyph.bottom
            anchors.topMargin: 10
            height: 5
            count: 22
            share: Math.max(0, navItem.share)
        }

        MouseArea {
            id: navMouse
            anchors.fill: parent
            hoverEnabled: true
            cursorShape: Qt.PointingHandCursor
            acceptedButtons: Qt.LeftButton | Qt.RightButton
            ToolTip.visible: !root.railOpen && navMouse.containsMouse
            ToolTip.text: navItem.trailing.length > 0 ? navItem.label + " (" + navItem.trailing + ")" : navItem.label
            ToolTip.delay: 300
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
                border.width: 1
                border.color: Theme.cGreen
            }
        }
    }

    // One folder in the sidebar's tree: a guide line down the left, an
    // elbow, a square node, the name and how many entries it holds. The row
    // on the way to the open folder lights the guide, elbow and node green.
    component TreeRow: Rectangle {
        id: treeRow
        property string label: ""
        property int count: 0
        property bool onPath: false
        property bool last: false
        property bool lit: false
        property string target: ""
        signal activated

        width: parent ? parent.width : 0
        height: 30
        color: treeRow.onPath ? Theme.cWash : (treeMouse.containsMouse ? Theme.cDim : "transparent")

        readonly property int guideX: 20
        // The guide runs the full row, or stops at the elbow on the last one.
        Rectangle {
            x: treeRow.guideX
            y: 0
            width: 1
            height: treeRow.last ? treeRow.height / 2 : treeRow.height
            color: treeRow.lit ? Theme.cGreen : Theme.cLine
        }
        Rectangle {
            x: treeRow.guideX
            y: treeRow.height / 2
            width: 12
            height: 1
            color: treeRow.onPath ? Theme.cGreen : Theme.cLine
        }
        Rectangle {
            id: treeNode
            x: treeRow.guideX + 12
            anchors.verticalCenter: parent.verticalCenter
            width: 7
            height: 7
            color: treeRow.onPath ? Theme.cGreen : Theme.cBase
            border.width: 1
            border.color: treeRow.onPath ? Theme.cGreen : Theme.cMuted
        }
        Text {
            id: treeToggle
            anchors.left: treeNode.right
            anchors.leftMargin: 8
            anchors.verticalCenter: parent.verticalCenter
            text: treeRow.onPath ? "▾" : "▸"
            color: treeRow.onPath ? Theme.cGreen : Theme.cLine
            font.family: Theme.fontMono
            font.pixelSize: 9
        }
        Text {
            anchors.left: treeToggle.right
            anchors.leftMargin: 8
            anchors.right: treeCount.left
            anchors.rightMargin: 8
            anchors.verticalCenter: parent.verticalCenter
            text: treeRow.label
            elide: Text.ElideRight
            color: Theme.cFg
            font.family: Theme.fontHuman
            font.pixelSize: 13
        }
        Text {
            id: treeCount
            anchors.right: parent.right
            anchors.rightMargin: 12
            anchors.verticalCenter: parent.verticalCenter
            text: treeRow.count < 10 ? "0" + treeRow.count : String(treeRow.count)
            color: treeRow.onPath ? Theme.cGreen : Theme.cMuted
            font.family: Theme.fontMono
            font.pixelSize: 11
        }
        MouseArea {
            id: treeMouse
            anchors.fill: parent
            hoverEnabled: true
            cursorShape: Qt.PointingHandCursor
            onClicked: treeRow.activated()
        }
        DropArea {
            anchors.fill: parent
            onDropped: function (drop) { root.dropInto(drop, treeRow.target) }
        }
    }

    // A clickable column head that carries the sort direction.
    component SortHead: Item {
        id: sortHead
        property string label: ""
        property string key: ""
        property bool numeric: false

        implicitHeight: 16
        implicitWidth: sortRow.implicitWidth

        Row {
            id: sortRow
            anchors.verticalCenter: parent.verticalCenter
            spacing: 6

            Text {
                text: sortHead.label.toUpperCase()
                color: root.sortKey === sortHead.key ? Theme.cFg : Theme.cMuted
                font.family: Theme.fontMono
                font.pixelSize: 10
                font.letterSpacing: 3
                anchors.verticalCenter: parent.verticalCenter
            }
            Text {
                visible: root.sortKey === sortHead.key
                text: root.sortAscending ? "▲" : "▼"
                color: Theme.cGreen
                font.pixelSize: 7
                anchors.verticalCenter: parent.verticalCenter
            }
        }

        MouseArea {
            anchors.fill: parent
            anchors.margins: -6
            cursorShape: Qt.PointingHandCursor
            onClicked: root.applySort(sortHead.key)
        }
    }

    // ---- Layout ---------------------------------------------------------

    // The file dialog's head: says what is being asked for, so the window
    // is not mistaken for a plain Files window.
    Rectangle {
        id: pickHead
        visible: root.picking
        anchors.left: parent.left
        anchors.right: parent.right
        anchors.top: parent.top
        height: root.picking ? 64 : 0
        color: root.raised
        z: 2

        CrossRule { anchors.bottom: parent.bottom; width: parent.width }

        Rectangle {
            id: pickBadge
            anchors.left: parent.left
            anchors.leftMargin: 20
            anchors.verticalCenter: parent.verticalCenter
            width: 36
            height: 36
            radius: Theme.radiusSmall
            color: root.emberWash
            border.width: 1
            border.color: Theme.wash(Theme.accent, 0.6)

            Icon {
                anchors.centerIn: parent
                name: root.pickSaving ? "download" : root.pickMode === "folder" ? "folder-open" : "file"
                size: 18
                shade: 1
            }
        }

        Column {
            anchors.left: pickBadge.right
            anchors.right: parent.right
            anchors.leftMargin: 14
            anchors.rightMargin: 24
            anchors.verticalCenter: parent.verticalCenter
            spacing: 3

            Text {
                width: parent.width
                elide: Text.ElideRight
                text: root.pickTitle
                color: root.ink
                font.family: root.sansFamily
                font.pixelSize: 17
                font.weight: Font.Medium
            }
            Text {
                text: (root.pickSaving ? "SAVE A FILE" : root.pickMode === "folder" ? "CHOOSE A FOLDER"
                       : root.pickMode === "open" ? "CHOOSE A FILE" : "CHOOSE FILES") + "  ·  FILE PICKER"
                color: root.inkMuted
                font.family: root.monoFamily
                font.pixelSize: 10
                font.letterSpacing: 1.5
            }
        }
    }

    // A dialog, not a window of its own: the whole edge carries the accent.
    Rectangle {
        visible: root.picking
        anchors.fill: parent
        color: "transparent"
        border.width: 1
        border.color: Theme.accent
        z: 3
    }

    // The file dialog's foot, only while picking.
    Rectangle {
        id: pickBar
        visible: root.picking
        anchors.left: parent.left
        anchors.right: parent.right
        anchors.bottom: parent.bottom
        height: root.picking ? 64 : 0
        color: root.raised
        z: 2

        CrossRule { anchors.top: parent.top; width: parent.width }

        Text {
            id: pickLabel
            visible: root.pickSaving
            anchors.left: parent.left
            anchors.leftMargin: 24
            anchors.verticalCenter: parent.verticalCenter
            text: "NAME"
            color: root.inkMuted
            font.family: root.monoFamily
            font.pixelSize: 11
            font.letterSpacing: 1.5
        }

        Field {
            id: pickName
            visible: root.pickSaving
            anchors.left: pickLabel.right
            anchors.right: pickButtons.left
            anchors.leftMargin: 14
            anchors.rightMargin: 18
            anchors.verticalCenter: parent.verticalCenter
            placeholder: "File name"
            onAccepted: root.acceptPick()
            Keys.onEscapePressed: Qt.quit()
        }

        Text {
            visible: !root.pickSaving
            anchors.left: parent.left
            anchors.right: pickButtons.left
            anchors.leftMargin: 24
            anchors.rightMargin: 18
            anchors.verticalCenter: parent.verticalCenter
            elide: Text.ElideMiddle
            text: root.pickMode === "folder" ? root.bridge.currentPath
                : root.selectedCount === 0 ? "Choose " + (root.pickMode === "open" ? "a file" : "files")
                : root.selectedCount + " selected"
            color: root.inkMuted
            font.family: root.sansFamily
            font.pixelSize: 13
        }

        Row {
            id: pickButtons
            anchors.right: parent.right
            anchors.rightMargin: 24
            anchors.verticalCenter: parent.verticalCenter
            spacing: 10

            TextButton { text: "Cancel"; onClicked: Qt.quit() }
            TextButton {
                primary: true
                text: root.pickReplaces ? "Replace" : root.pickAccept || (root.pickSaving ? "Save" : root.pickMode === "folder" ? "Choose" : "Open")
                enabled: root.pickSaving ? pickName.text.trim().length > 0
                       : root.pickMode === "folder" || root.selectedCount > 0
                onClicked: root.acceptPick()
            }
        }
    }

    // ============================ HEADER =============================
    // One 64px bar across the window: the mark, the way back, where you
    // are, search, how to look, and what to make.
    Rectangle {
        id: appHeader
        z: 4
        anchors.left: parent.left
        anchors.right: parent.right
        anchors.top: parent.top
        anchors.topMargin: pickHead.height
        height: Theme.headerHeight
        color: Theme.cBar

        Rectangle {
            anchors.left: parent.left
            anchors.right: parent.right
            anchors.bottom: parent.bottom
            height: 1
            color: Theme.cDead
        }

        // ---- Mark ----
        Item {
            id: brandBlock
            anchors.left: parent.left
            anchors.top: parent.top
            anchors.bottom: parent.bottom
            width: root.sidebarWidth
            Rectangle {
                id: brandMark
                anchors.left: parent.left
                anchors.leftMargin: 20
                anchors.verticalCenter: parent.verticalCenter
                width: 34
                height: 34
                color: Theme.cGreen
                Text {
                    anchors.centerIn: parent
                    text: "T"
                    color: Theme.cBase
                    font.family: Theme.fontBrand
                    font.pixelSize: 17
                    font.weight: Font.ExtraBold
                }
            }
            Column {
                anchors.left: brandMark.right
                anchors.leftMargin: 12
                anchors.verticalCenter: parent.verticalCenter
                visible: root.sidebarWidth > 160
                spacing: 1
                Text {
                    text: "Thallium"
                    color: Theme.cFg
                    font.family: Theme.fontBrand
                    font.pixelSize: 17
                    font.weight: Font.DemiBold
                }
                Text {
                    text: "FILES"
                    color: Theme.cMuted
                    font.family: Theme.fontMono
                    font.pixelSize: 9
                    font.letterSpacing: 4
                }
            }
        }

        // ---- The way back ----
        Row {
            id: headerNav
            anchors.left: brandBlock.right
            anchors.leftMargin: 10
            anchors.verticalCenter: parent.verticalCenter
            spacing: 6
            IconButton { id: backButton; icon: "chevron-left"; hint: "Back (Alt+Left)"; iconSize: 15; onClicked: root.bridge.back() }
            IconButton { icon: "chevron-right"; hint: "Forward (Alt+Right)"; iconSize: 15; onClicked: root.bridge.forward() }
            IconButton { icon: "arrow-up"; hint: "Up (Alt+Up)"; iconSize: 15; onClicked: root.bridge.up() }
        }

        // ---- Where you are: one tab per folder ----
        Item {
            id: pathBar
            anchors.left: headerNav.right
            anchors.leftMargin: 18
            anchors.right: headerSearch.left
            anchors.rightMargin: 18
            anchors.top: parent.top
            anchors.bottom: parent.bottom

            // Ctrl+L swaps the tabs for a field that takes a typed path;
            // Enter goes there, Escape puts the tabs back.
            Rectangle {
                anchors.left: parent.left
                anchors.right: parent.right
                anchors.verticalCenter: parent.verticalCenter
                height: 36
                visible: pathEditor.visible
                color: Theme.cDim
                border.width: 1
                border.color: Theme.cGreen
            }
            TextInput {
                id: pathEditor
                visible: false
                anchors.left: parent.left
                anchors.right: parent.right
                anchors.leftMargin: 12
                anchors.rightMargin: 12
                anchors.verticalCenter: parent.verticalCenter
                height: 36
                verticalAlignment: TextInput.AlignVCenter
                clip: true
                color: Theme.cFg
                selectionColor: Theme.cWash
                selectedTextColor: Theme.cFg
                font.family: Theme.fontMono
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
                anchors.fill: parent
                contentWidth: crumbRow.width
                clip: true
                flickableDirection: Flickable.HorizontalFlick
                // Keep the deepest tab in view.
                contentX: Math.max(0, crumbRow.width - width)

                Row {
                    id: crumbRow
                    height: parent.height
                    spacing: 0

                    Repeater {
                        id: crumbRepeater
                        model: root.trashOpen ? [{ label: "Trash", path: "" }]
                             : root.recentOpen ? [{ label: "Recent", path: "" }]
                             : root.crumbs
                        delegate: Row {
                            id: crumb
                            required property int index
                            required property var modelData
                            readonly property bool last: crumb.index === crumbRepeater.count - 1
                            height: crumbRow.height
                            spacing: 0

                            Tab {
                                height: parent.height
                                text: crumb.modelData.label
                                active: crumb.last
                                underlineInset: 0
                                onClicked: if (crumb.modelData.path.length > 0) root.bridge.navigate(crumb.modelData.path)
                                DropArea {
                                    anchors.fill: parent
                                    enabled: crumb.modelData.path.length > 0
                                    onDropped: function (drop) { root.dropInto(drop, crumb.modelData.path) }
                                }
                                TapHandler { onDoubleTapped: root.editPath() }
                            }
                            Text {
                                visible: !crumb.last
                                anchors.verticalCenter: parent.verticalCenter
                                leftPadding: 4
                                rightPadding: 4
                                text: "›"
                                color: Theme.cLine
                                font.family: Theme.fontMono
                                font.pixelSize: 14
                            }
                        }
                    }
                }
            }
        }

        // ---- Search ----
        Rectangle {
            id: headerSearch
            anchors.right: viewTabs.left
            anchors.rightMargin: 22
            anchors.verticalCenter: parent.verticalCenter
            width: Math.max(180, Math.min(280, parent.width * 0.2))
            height: 36
            color: Theme.cDim
            border.width: 1
            border.color: searchInput.activeFocus ? Theme.cGreen : Theme.cLine

            Rectangle {
                // The focus ring: 3px of the accent at 14%.
                anchors.fill: parent
                anchors.margins: -3
                z: -1
                visible: searchInput.activeFocus
                color: Theme.wash(Theme.cGreen, 0.14)
            }

            Text {
                id: searchMark
                anchors.left: parent.left
                anchors.leftMargin: 12
                anchors.verticalCenter: parent.verticalCenter
                text: "⌕"
                color: root.deepSearch ? Theme.cGreen : Theme.cMuted
                font.family: Theme.fontMono
                font.pixelSize: 14
            }

            TextInput {
                id: searchInput
                anchors.left: searchMark.right
                anchors.leftMargin: 10
                anchors.right: searchKbd.left
                anchors.rightMargin: 8
                anchors.verticalCenter: parent.verticalCenter
                clip: true
                color: Theme.cFg
                selectionColor: Theme.cWash
                selectedTextColor: Theme.cFg
                font.family: Theme.fontHuman
                font.pixelSize: 13
                text: root.query
                onTextChanged: root.query = text
                onActiveFocusChanged: if (!activeFocus && text.length === 0) root.searchShown = false
                Keys.onEscapePressed: {
                    if (root.deepSearch)
                        root.endDeepSearch()
                    root.query = ""
                    root.searchShown = false
                    root.release()
                }
                Keys.onReturnPressed: root.beginDeepSearch()
                Keys.onEnterPressed: root.beginDeepSearch()

                Text {
                    anchors.verticalCenter: parent.verticalCenter
                    visible: searchInput.text.length === 0
                    text: "Search…"
                    color: Theme.cMuted
                    font.family: Theme.fontHuman
                    font.pixelSize: 13
                }
            }

            Kbd {
                id: searchKbd
                anchors.right: parent.right
                anchors.rightMargin: 12
                anchors.verticalCenter: parent.verticalCenter
                visible: root.query.length === 0
                text: "CTRL K"
            }
            Text {
                id: clearSearch
                anchors.right: parent.right
                anchors.rightMargin: 12
                anchors.verticalCenter: parent.verticalCenter
                visible: root.query.length > 0
                text: "×"
                color: clearMouse.containsMouse ? Theme.cGreen : Theme.cMuted
                font.family: Theme.fontMono
                font.pixelSize: 14
                MouseArea {
                    id: clearMouse
                    anchors.fill: parent
                    anchors.margins: -6
                    hoverEnabled: true
                    cursorShape: Qt.PointingHandCursor
                    onClicked: {
                        root.endDeepSearch()
                        root.query = ""
                    }
                }
            }
        }

        // ---- How to look ----
        Row {
            id: viewTabs
            anchors.right: newButton.left
            anchors.rightMargin: 22
            anchors.top: parent.top
            anchors.bottom: parent.bottom
            visible: !root.trashOpen && !root.recentOpen
            Tab { height: parent.height; text: "Grid"; active: root.view === "grid"; onClicked: root.view = "grid" }
            Tab { height: parent.height; text: "List"; active: root.view !== "grid"; onClicked: root.view = "list" }
        }

        // ---- What to make ----
        TextButton {
            id: newButton
            anchors.right: windowButtons.left
            anchors.rightMargin: 22
            anchors.verticalCenter: parent.verticalCenter
            primary: true
            height: 36
            text: "+ New"
            visible: !root.trashOpen && !root.recentOpen
            onClicked: groundMenu.popup(newButton, 0, newButton.height + 6)
        }

        Row {
            id: windowButtons
            anchors.right: parent.right
            anchors.rightMargin: 12
            anchors.top: parent.top
            anchors.bottom: parent.bottom
            spacing: 0
            Rectangle {
                width: 1
                height: 34
                anchors.verticalCenter: parent.verticalCenter
                color: Theme.cLine
            }
            Item { width: 10; height: 1 }
            Repeater {
                model: [
                    { glyph: "–", hint: "Minimise" },
                    { glyph: "□", hint: "Maximise" },
                    { glyph: "×", hint: "Close" }
                ]
                delegate: Item {
                    id: windowButton
                    required property var modelData
                    required property int index
                    width: 34
                    height: parent.height
                    Text {
                        anchors.centerIn: parent
                        text: windowButton.modelData.glyph
                        color: windowMouse.containsMouse
                               ? (windowButton.index === 2 ? Theme.cRed : Theme.cGreen) : Theme.cFg
                        font.family: Theme.fontMono
                        font.pixelSize: 13
                    }
                    MouseArea {
                        id: windowMouse
                        anchors.fill: parent
                        hoverEnabled: true
                        cursorShape: Qt.PointingHandCursor
                        onClicked: {
                            if (windowButton.index === 0)
                                root.showMinimized()
                            else if (windowButton.index === 1)
                                root.visibility === Window.Maximized ? root.showNormal() : root.showMaximized()
                            else
                                root.close()
                        }
                    }
                }
            }
        }
    }

    // ========================== STATUS BAR ===========================
    Rectangle {
        id: statusBar
        z: 4
        anchors.left: parent.left
        anchors.right: parent.right
        anchors.bottom: parent.bottom
        anchors.bottomMargin: pickBar.height
        height: Theme.statusHeight
        color: Theme.cBar

        function pad(n) { return n < 10 ? "0" + n : String(n) }

        Rectangle {
            anchors.left: parent.left
            anchors.right: parent.right
            anchors.top: parent.top
            height: 1
            color: Theme.cDead
        }
        Text {
            anchors.left: parent.left
            anchors.leftMargin: 20
            anchors.verticalCenter: parent.verticalCenter
            textFormat: Text.StyledText
            text: {
                const parts = [statusBar.pad(root.rows.length) + " ITEMS"]
                if (root.selectedCount > 0)
                    parts.push(statusBar.pad(root.selectedCount) + " SELECTED")
                if (root.selectedCount === 1 && inspector.sizeText.length > 0)
                    parts.push("<font color='" + Theme.cFg + "'>" + inspector.sizeText.toUpperCase() + "</font>")
                if (root.showHidden)
                    parts.push("HIDDEN SHOWN")
                return parts.join("  ·  ")
            }
            color: Theme.cMuted
            font.family: Theme.fontMono
            font.pixelSize: 11
            font.letterSpacing: 2
        }
        Row {
            anchors.right: parent.right
            anchors.rightMargin: 20
            anchors.verticalCenter: parent.verticalCenter
            spacing: 10
            readonly property int remotes: {
                void root.revision
                return root.bridge.remoteCount()
            }
            Rectangle {
                visible: parent.remotes > 0
                anchors.verticalCenter: parent.verticalCenter
                width: 6
                height: 6
                color: Theme.cGreen
            }
            Text {
                visible: parent.remotes > 0
                text: {
                    void root.revision
                    return (parent.remotes === 1 ? root.bridge.remoteName(0) : parent.remotes + " REMOTES").toUpperCase() + " LINKED"
                }
                color: Theme.cGreen
                font.family: Theme.fontMono
                font.pixelSize: 11
                font.letterSpacing: 2
            }
            Text {
                visible: parent.remotes > 0 && freeText.visible
                text: "·"
                color: Theme.cMuted
                font.family: Theme.fontMono
                font.pixelSize: 11
            }
            Text {
                id: freeText
                text: {
                    void root.revision
                    return root.readableSize(root.bridge.storageTotal() - root.bridge.storageUsed()).toUpperCase() + " FREE"
                }
                visible: {
                    void root.revision
                    return root.bridge.storageKnown()
                }
                color: Theme.cMuted
                font.family: Theme.fontMono
                font.pixelSize: 11
                font.letterSpacing: 2
            }
        }
    }

    Row {
        anchors.fill: parent
        anchors.bottomMargin: pickBar.height + statusBar.height
        anchors.topMargin: pickHead.height + appHeader.height
        spacing: 0

        // ============================ SIDEBAR ============================
        Rectangle {
            id: sidebar
            width: root.sidebarWidth
            height: parent.height
            color: Theme.cBase
            clip: true

            Rectangle {
                anchors.right: parent.right
                width: 1
                height: parent.height
                color: Theme.cDead
                z: 2
            }

            Flickable {
                id: placesView
                anchors.left: parent.left
                anchors.right: parent.right
                anchors.top: parent.top
                anchors.bottom: sidebarFoot.top
                anchors.rightMargin: 1
                contentHeight: placesColumn.implicitHeight + 24
                clip: true
                boundsBehavior: Flickable.StopAtBounds

                Column {
                    id: placesColumn
                    x: 14
                    y: 24
                    width: placesView.width - 28
                    spacing: 0

                    // ---- Today, and whose machine this is ----
                    Column {
                        visible: root.railOpen
                        width: parent.width
                        leftPadding: 6
                        spacing: 6
                        Text {
                            text: root.today
                            color: Theme.cGreen
                            font.family: Theme.fontMono
                            font.pixelSize: 10
                            font.letterSpacing: 3.5
                        }
                        Text {
                            text: root.whoami
                            color: Theme.cFg
                            font.family: Theme.fontBrand
                            font.pixelSize: 15
                            font.weight: Font.DemiBold
                        }
                    }
                    Item { width: 1; height: root.railOpen ? 26 : 0 }

                    HudHeader {
                        visible: root.railOpen
                        x: 6
                        width: parent.width - 6
                        title: "Places"
                        titleSize: 12
                    }
                    Item { width: 1; height: root.railOpen ? 12 : 0 }

                    Repeater {
                        model: root.places.filter(function (place) { return place.section === "" })
                        delegate: Column {
                            id: placeBlock
                            required property var modelData
                            readonly property bool recent: modelData.recent === true
                            readonly property bool trash: modelData.trash === true
                            readonly property bool marked: modelData.bookmark !== undefined
                            readonly property string target: placeBlock.marked ? modelData.target : root.placePath(modelData.folder)
                            readonly property bool active: placeBlock.recent ? root.recentOpen
                                                         : placeBlock.trash ? root.trashOpen
                                                         : !root.recentOpen && !root.trashOpen && !root.settingsOpen
                                                           && placeBlock.target === root.activePlace
                            readonly property bool expanded: placeBlock.active && root.railOpen && root.placeTree.length > 0
                                                             && !placeBlock.recent && !placeBlock.trash
                            width: placesColumn.width

                            NavItem {
                                width: parent.width
                                icon: placeBlock.modelData.icon || ""
                                label: placeBlock.modelData.label
                                active: placeBlock.active
                                meta: placeBlock.active && !placeBlock.recent && !placeBlock.trash && root.placeTotal >= 0
                                      ? root.pad2(root.placeTotal) + (placeBlock.expanded ? "  ▾" : "") : ""
                                dropTarget: placeBlock.recent || placeBlock.trash ? "" : placeBlock.target
                                onMenuRequested: {
                                    if (!placeBlock.marked)
                                        return
                                    bookmarkMenu.index = placeBlock.modelData.bookmark
                                    bookmarkMenu.popup()
                                }
                                onActivated: {
                                    if (placeBlock.recent)
                                        root.showRecent()
                                    else if (placeBlock.trash)
                                        root.showTrash()
                                    else
                                        root.openPlace(placeBlock.modelData)
                                }
                            }
                            Column {
                                visible: placeBlock.expanded
                                width: parent.width
                                topPadding: 2
                                bottomPadding: 6
                                Repeater {
                                    model: placeBlock.expanded ? root.placeTree : []
                                    delegate: TreeRow {
                                        id: branch
                                        required property var modelData
                                        required property int index
                                        readonly property string path: placeBlock.target + "/" + branch.modelData.name
                                        width: placesColumn.width
                                        label: branch.modelData.name
                                        count: branch.modelData.count
                                        target: branch.path
                                        last: branch.index === root.placeTree.length - 1
                                        onPath: root.bridge.currentPath === branch.path
                                                || root.bridge.currentPath.startsWith(branch.path + "/")
                                        lit: root.placeTreeLit > branch.index
                                        onActivated: root.bridge.navigate(branch.path)
                                    }
                                }
                            }
                        }
                    }

                    Item { width: 1; height: root.railOpen ? 26 : 10 }
                    HudHeader {
                        visible: root.railOpen
                        x: 6
                        width: parent.width - 6
                        title: "Locations"
                        titleSize: 12
                    }
                    Item { width: 1; height: root.railOpen ? 12 : 0 }

                    Repeater {
                        model: root.places.filter(function (place) { return place.section !== "" })
                        delegate: NavItem {
                            id: location
                            required property var modelData
                            readonly property bool adds: location.modelData.add === true
                            readonly property bool remote: location.modelData.remote === true
                            readonly property bool drive: location.modelData.drive !== undefined
                            readonly property bool marked: location.modelData.bookmark !== undefined
                            readonly property bool system: location.modelData.folder === "/"
                            readonly property string target: location.system ? "/" : (location.modelData.target || "")
                            width: placesColumn.width
                            icon: location.adds ? "" : (location.remote ? "globe" : location.modelData.icon || "")
                            glyph: location.adds ? "+" : ""
                            accentLabel: location.adds
                            label: location.adds ? "Add remote" : location.modelData.label
                            active: !location.adds && !root.recentOpen && !root.trashOpen && !root.settingsOpen
                                    && location.target.length > 0 && root.bridge.currentPath === location.target
                            meta: location.remote ? "LINKED"
                                  : location.system && root.storageKnown ? Math.round(root.storageShare * 100) + "%"
                                  : location.drive && location.target.length === 0 ? "OFF" : ""
                            metaColor: location.remote ? Theme.cGreen : Theme.cMuted
                            share: location.system && root.storageKnown ? root.storageShare : -1
                            trailing: location.remote ? "remote"
                                      : location.drive && location.target.length === 0 ? "not mounted" : ""
                            dropTarget: location.adds || location.remote ? "" : location.target
                            onMenuRequested: {
                                if (location.drive) {
                                    driveMenu.index = location.modelData.drive
                                    driveMenu.popup()
                                } else if (location.marked) {
                                    bookmarkMenu.index = location.modelData.bookmark
                                    bookmarkMenu.popup()
                                }
                            }
                            onActivated: root.openPlace(location.modelData)
                        }
                    }
                }
            }

            // Settings, and the way to fold the sidebar into a rail.
            Column {
                id: sidebarFoot
                anchors.left: parent.left
                anchors.right: parent.right
                anchors.bottom: parent.bottom
                anchors.leftMargin: 14
                anchors.rightMargin: 15
                anchors.bottomMargin: 12
                spacing: 2
                Rectangle { width: parent.width; height: 1; color: Theme.cDead }
                Item { width: 1; height: 8 }
                NavItem {
                    visible: !root.picking
                    icon: "settings"
                    label: "Settings"
                    active: root.settingsOpen
                    onActivated: root.openSettings()
                }
                NavItem {
                    id: railToggle
                    glyph: root.railOpen ? "«" : "»"
                    label: "Collapse"
                    meta: "CTRL B"
                    onActivated: root.railOpen = !root.railOpen
                }
            }
        }

        // ============================== MAIN ==============================
        Item {
            id: main
            width: parent.width - root.sidebarWidth - (root.previewOpen ? root.inspectorWidth : 0) - splitPane.width
            height: parent.height

            // ---- Header ----
            // The tabs, once there is more than one. The folder's name is in
            // the crumbs and the actions are on the folder bar.
            Item {
                id: header
                visible: root.tabs.length > 1
                anchors.left: parent.left
                anchors.right: parent.right
                anchors.top: parent.top
                anchors.leftMargin: 28
                anchors.rightMargin: 28
                anchors.topMargin: root.tabs.length > 1 ? 16 : 0
                height: root.tabs.length > 1 ? 46 : 0

                Row {
                    id: tabStrip
                    anchors.left: parent.left
                    anchors.right: parent.right
                    anchors.verticalCenter: parent.verticalCenter
                    spacing: 6
                    clip: true

                    Repeater {
                        model: root.tabs.length > 1 ? root.tabs : []
                        delegate: Rectangle {
                            id: tabChip
                            required property var modelData
                            required property int index
                            readonly property bool current: tabChip.index === root.currentTab
                            width: Math.min(160, tabLabel.implicitWidth + 40)
                            height: 30
                            radius: Theme.radiusSmall
                            color: tabChip.current ? Theme.selected : (tabMouse.containsMouse ? Theme.surfaceHigh : Theme.surface)
                            border.width: tabChip.current ? 1 : 0
                            border.color: Theme.outline

                            Text {
                                id: tabLabel
                                anchors.left: parent.left
                                anchors.leftMargin: 11
                                anchors.right: tabClose.left
                                anchors.rightMargin: 4
                                anchors.verticalCenter: parent.verticalCenter
                                text: root.leafName(tabChip.modelData.currentPath)
                                elide: Text.ElideMiddle
                                color: tabChip.current ? Theme.ink : Theme.inkDim
                                font.family: Theme.family
                                font.pixelSize: 12
                            }
                            MouseArea {
                                id: tabMouse
                                anchors.fill: parent
                                hoverEnabled: true
                                acceptedButtons: Qt.LeftButton | Qt.MiddleButton
                                onClicked: function (mouse) {
                                    if (mouse.button === Qt.MiddleButton)
                                        root.closeTab(tabChip.index)
                                    else
                                        root.switchTab(tabChip.index)
                                }
                            }
                            DropArea {
                                anchors.fill: parent
                                onDropped: function (drop) { root.dropInto(drop, tabChip.modelData.currentPath) }
                            }
                            Icon {
                                id: tabClose
                                anchors.right: parent.right
                                anchors.rightMargin: 8
                                anchors.verticalCenter: parent.verticalCenter
                                name: "x"
                                size: 13
                                shade: tabCloseMouse.containsMouse ? 1 : 0.45
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

            }

            CrossRule {
                id: headerRule
                visible: root.tabs.length > 1
                anchors.left: parent.left
                anchors.right: parent.right
                anchors.top: header.bottom
                anchors.leftMargin: 24
                anchors.rightMargin: 24
                anchors.topMargin: root.tabs.length > 1 ? 8 : 0
            }

            // The folder bar moved into the window's header.
            Item {
                id: folderBar
                anchors.left: parent.left
                anchors.right: parent.right
                anchors.top: headerRule.bottom
                height: 0
            }

            // The count lives in the window's status bar now.
            Item {
                id: statusLine
                anchors.left: parent.left
                anchors.right: parent.right
                anchors.bottom: parent.bottom
                height: 0
            }

            // ---- Browser ----
            Item {
                id: browser
                anchors.left: parent.left
                anchors.right: parent.right
                anchors.top: folderBar.bottom
                anchors.bottom: statusLine.top
                anchors.leftMargin: 32
                anchors.rightMargin: 32
                anchors.topMargin: 0
                anchors.bottomMargin: 18
                clip: true


                // Selection toolbar, present only while something is held.
                // Views anchor under this; it takes no room, so holding an
                // entry never moves the list.
                HudHeader {
                    id: filesHud
                    readonly property bool shown: !root.trashOpen && !root.recentOpen && !root.deepSearch
                    anchors.left: parent.left
                    anchors.right: parent.right
                    anchors.top: parent.top
                    anchors.topMargin: shown ? 26 : 0
                    height: shown ? 22 : 0
                    visible: shown
                    title: "Files"
                    sub: {
                        const total = root.rows.length
                        const folders = root.composition.length > 0 ? root.composition[0].count : 0
                        return total + (total === 1 ? " item" : " items") + "  ·  "
                             + folders + (folders === 1 ? " folder, " : " folders, ")
                             + (total - folders) + (total - folders === 1 ? " file" : " files")
                    }
                    trailing: root.sortKey.toUpperCase() + (root.sortAscending ? "  ▲" : "  ▼")
                    onTrailingClicked: root.applySort(root.sortKey)
                }

                Item {
                    id: selectionBar
                    anchors.left: parent.left
                    anchors.right: parent.right
                    anchors.top: filesHud.bottom
                    anchors.topMargin: filesHud.shown ? 18 : 0
                    height: 0
                }

                // Transfers float over the foot of the pane, above the dock, so
                // they show whether or not the preview panel is open.
                Rectangle {
                    id: taskPanel
                    z: 20
                    anchors.right: parent.right
                    anchors.bottom: parent.bottom
                    anchors.rightMargin: 18
                    anchors.bottomMargin: selectionDock.visible ? 18 + selectionDock.height + 10 : 18
                    width: Math.min(360, parent.width - 36)
                    height: root.taskCount > 0 ? Math.min(220, 34 + root.taskCount * 48) : 0
                    visible: root.taskCount > 0
                    radius: Theme.radius
                    color: Theme.surfaceHigh
                    border.width: 1
                    border.color: Theme.outlineSoft
                    clip: true

                    Text {
                        id: taskHeading
                        anchors.left: parent.left
                        anchors.top: parent.top
                        anchors.margins: 14
                        text: "Transfers"
                        color: Theme.ink
                        font.family: Theme.family
                        font.pixelSize: Theme.text
                        font.weight: Font.Medium
                    }

                    ListView {
                        anchors.left: parent.left
                        anchors.right: parent.right
                        anchors.top: taskHeading.bottom
                        anchors.bottom: parent.bottom
                        anchors.margins: 12
                        anchors.topMargin: 8
                        clip: true
                        spacing: 8
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
                                color: Theme.inkSoft
                                font.family: Theme.family
                                font.pixelSize: 12
                            }
                            IconButton {
                                id: taskCancel
                                anchors.right: parent.right
                                anchors.top: parent.top
                                anchors.topMargin: -5
                                implicitWidth: 24
                                implicitHeight: 24
                                icon: "x"
                                iconSize: 13
                                hint: "Cancel or dismiss"
                                onClicked: root.bridge.cancelTask(root.bridge.taskId(taskRow.slot))
                            }
                            Text {
                                anchors.left: parent.left
                                anchors.top: taskTitle.bottom
                                anchors.topMargin: 1
                                text: {
                                    void root.revision
                                    return root.bridge.taskStatus(taskRow.slot)
                                }
                                color: Theme.inkFaint
                                font.family: Theme.family
                                font.pixelSize: 10
                            }
                            Rectangle {
                                anchors.left: parent.left
                                anchors.right: parent.right
                                anchors.bottom: parent.bottom
                                height: 3
                                radius: 0
                                color: Theme.control
                                Rectangle {
                                    readonly property int progress: {
                                        void root.revision
                                        return root.bridge.taskProgress(taskRow.slot)
                                    }
                                    height: parent.height
                                    radius: 0
                                    width: parent.width * Math.max(0, Math.min(100, progress)) / 100
                                    color: Theme.inkSoft
                                }
                            }
                        }
                    }
                }

                // What can be done with the held entries, floating over the
                // foot of the pane while anything is held.
                Rectangle {
                    id: selectionDock
                    z: 20
                    anchors.horizontalCenter: parent.horizontalCenter
                    anchors.bottom: parent.bottom
                    anchors.bottomMargin: 18
                    visible: root.selectedCount > 0 && !root.trashOpen && !root.recentOpen
                    width: dockRow.implicitWidth + 16
                    height: 46
                    radius: Theme.radius
                    color: Theme.control
                    border.width: 1
                    border.color: Theme.outline

                    Row {
                        id: dockRow
                        anchors.centerIn: parent
                        spacing: 4

                        Text {
                            anchors.verticalCenter: parent.verticalCenter
                            leftPadding: 8
                            rightPadding: 10
                            text: root.selectedCount + " held"
                            color: Theme.ink
                            font.family: Theme.family
                            font.pixelSize: Theme.text
                            font.weight: Font.Medium
                        }
                        IconButton { icon: "scissors"; hint: "Cut (Ctrl+X)"; onClicked: root.bridge.offerSelected(true) }
                        IconButton { icon: "copy"; hint: "Copy (Ctrl+C)"; onClicked: root.bridge.offerSelected(false) }
                        IconButton {
                            icon: "pencil"
                            hint: "Rename (F2)"
                            onClicked: root.selectedCount > 1 ? root.beginBulkRename() : root.beginRename(root.firstSelectedRow())
                        }
                        IconButton { icon: "info"; hint: "Properties (Alt+Enter)"; enabled: root.selectedCount === 1; onClicked: root.showProperties(root.firstSelectedRow()) }
                        IconButton { icon: "trash-2"; hint: "Move to Trash (Delete)"; onClicked: root.bridge.trashSelected() }
                        Rectangle { width: 1; height: 22; color: Theme.outlineSoft; anchors.verticalCenter: parent.verticalCenter }
                        IconButton { icon: "x"; hint: "Let go (Escape)"; onClicked: root.bridge.clearSelection() }
                    }
                }

                // Column heads, list view only: the top of the table frame.
                Rectangle {
                    id: columnHeads
                    anchors.left: parent.left
                    anchors.right: parent.right
                    anchors.top: selectionBar.bottom
                    height: visible ? 40 : 0
                    visible: root.view !== "grid" && !root.trashOpen && !root.recentOpen && !root.deepSearch
                    color: Theme.cPanel
                    border.width: 1
                    border.color: Theme.cLine
                    // Where each column starts, as a share of the table.
                    readonly property real kindX: Math.round(width * 0.445)
                    readonly property real sizeX: Math.round(width * 0.64)
                    readonly property real modifiedX: Math.round(width * 0.785)
                    readonly property bool kindShown: width >= 560
                    readonly property bool sizeShown: width >= 420

                    SortHead { x: 16; anchors.verticalCenter: parent.verticalCenter; label: "Name"; key: "name" }
                    SortHead { x: columnHeads.kindX; visible: columnHeads.kindShown; anchors.verticalCenter: parent.verticalCenter; label: "Kind"; key: "kind" }
                    Text {
                        x: columnHeads.sizeX
                        visible: columnHeads.sizeShown
                        anchors.verticalCenter: parent.verticalCenter
                        text: "SIZE"
                        color: Theme.cMuted
                        font.family: Theme.fontMono
                        font.pixelSize: 10
                        font.letterSpacing: 3
                    }
                    SortHead { x: columnHeads.modifiedX; anchors.verticalCenter: parent.verticalCenter; label: "Modified"; key: "modified" }
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
                                radius: 0
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

                    Icon {
                        anchors.horizontalCenter: parent.horizontalCenter
                        name: root.query.length > 0 ? "search" : "folder-open"
                        size: 22
                        shade: 0.4
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
                    TextButton {
                        text: "Restore"
                        enabled: root.trashPicked.length > 0
                        onClicked: {
                            root.bridge.restoreTrash(root.trashPicked)
                            root.trashPicked = []
                        }
                    }
                    TextButton {
                        text: "Delete permanently"
                        enabled: root.trashPicked.length > 0 && !root.bridge.trashBusy()
                        onClicked: {
                            root.bridge.purgeTrash(root.trashPicked)
                            root.trashPicked = []
                        }
                    }
                    TextButton {
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
                        readonly property bool folder: {
                            void root.revision
                            return root.bridge.trashIsDir(trashRow.index)
                        }

                        width: ListView.view.width
                        height: Theme.rowHeight
                        color: "transparent"

                        // A card per item, filled and outlined all round when picked.
                        Rectangle {
                            anchors.fill: parent
                            anchors.topMargin: 4
                            anchors.bottomMargin: 4
                            radius: Theme.radius
                            color: trashRow.picked ? Theme.selected
                                 : trashMouse.containsMouse ? Theme.surfaceHigh : Theme.surface
                            border.width: trashRow.picked ? 1 : 0
                            border.color: Theme.outline
                        }

                        FileTile {
                            id: trashGlyph
                            anchors.left: parent.left
                            anchors.leftMargin: 18
                            anchors.verticalCenter: parent.verticalCenter
                            width: 30
                            height: width
                            name: trashRow.name
                            folder: trashRow.folder
                            lifted: trashRow.picked
                        }

                        Text {
                            id: trashName
                            anchors.left: trashGlyph.right
                            anchors.leftMargin: 16
                            anchors.right: trashWhen.left
                            anchors.rightMargin: 16
                            anchors.bottom: parent.verticalCenter
                            text: trashRow.name
                            elide: Text.ElideMiddle
                            color: Theme.ink
                            font.family: Theme.family
                            font.pixelSize: Theme.text
                        }

                        Text {
                            anchors.left: trashName.left
                            anchors.right: trashWhen.left
                            anchors.rightMargin: 16
                            anchors.top: parent.verticalCenter
                            anchors.topMargin: 1
                            text: root.bridge.trashOrigin(trashRow.index)
                            elide: Text.ElideLeft
                            color: Theme.inkDim
                            font.family: Theme.mono
                            font.pixelSize: Theme.textSmall
                        }

                        Text {
                            id: trashWhen
                            anchors.right: parent.right
                            anchors.rightMargin: 20
                            anchors.verticalCenter: parent.verticalCenter
                            text: root.modifiedText(root.bridge.trashDeleted(trashRow.index))
                            color: Theme.inkDim
                            font.family: Theme.mono
                            font.pixelSize: Theme.textSmall
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
                        Icon {
                            anchors.horizontalCenter: parent.horizontalCenter
                            name: "trash-2"
                            size: 22
                            shade: 0.4
                        }
                        Text {
                            anchors.horizontalCenter: parent.horizontalCenter
                            text: "The trash is empty"
                            color: Theme.inkDim
                            font.family: Theme.family
                            font.pixelSize: Theme.text
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

                        width: ListView.view.width
                        height: Theme.rowHeight
                        color: "transparent"

                        Rectangle {
                            anchors.fill: parent
                            anchors.topMargin: 4
                            anchors.bottomMargin: 4
                            radius: Theme.radius
                            color: recentMouse.containsMouse ? Theme.surfaceHigh : Theme.surface
                        }

                        FileTile {
                            id: recentGlyph
                            anchors.left: parent.left
                            anchors.leftMargin: 18
                            anchors.verticalCenter: parent.verticalCenter
                            width: 30
                            height: width
                            name: recentRow.name
                        }

                        Text {
                            id: recentName
                            anchors.left: recentGlyph.right
                            anchors.leftMargin: 16
                            anchors.right: parent.right
                            anchors.rightMargin: 20
                            anchors.bottom: parent.verticalCenter
                            text: recentRow.name
                            elide: Text.ElideMiddle
                            color: Theme.ink
                            font.family: Theme.family
                            font.pixelSize: Theme.text
                        }

                        Text {
                            anchors.left: recentName.left
                            anchors.right: parent.right
                            anchors.rightMargin: 20
                            anchors.top: parent.verticalCenter
                            anchors.topMargin: 1
                            text: recentRow.where
                            elide: Text.ElideLeft
                            color: Theme.inkDim
                            font.family: Theme.mono
                            font.pixelSize: Theme.textSmall
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

                        Icon {
                            anchors.horizontalCenter: parent.horizontalCenter
                            name: "clock"
                            size: 22
                            shade: 0.4
                        }

                        Text {
                            anchors.horizontalCenter: parent.horizontalCenter
                            text: "The desktop has no record of recent files"
                            color: Theme.inkDim
                            font.family: Theme.family
                            font.pixelSize: Theme.text
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

                        width: ListView.view.width
                        height: Theme.rowHeight
                        color: "transparent"

                        Rectangle {
                            anchors.fill: parent
                            anchors.topMargin: 4
                            anchors.bottomMargin: 4
                            radius: Theme.radius
                            color: resultMouse.containsMouse ? Theme.surfaceHigh : Theme.surface
                        }

                        FileTile {
                            id: resultGlyph
                            anchors.left: parent.left
                            anchors.leftMargin: 18
                            anchors.verticalCenter: parent.verticalCenter
                            width: 30
                            height: width
                            name: resultRow.name
                            folder: resultRow.folder
                        }

                        Text {
                            id: resultName
                            anchors.left: resultGlyph.right
                            anchors.leftMargin: 16
                            anchors.right: parent.right
                            anchors.rightMargin: 20
                            anchors.bottom: parent.verticalCenter
                            text: resultRow.name
                            elide: Text.ElideMiddle
                            color: Theme.ink
                            font.family: Theme.family
                            font.pixelSize: Theme.text
                            font.weight: resultRow.folder ? Font.Medium : Font.Normal
                        }

                        Text {
                            anchors.left: resultName.left
                            anchors.right: parent.right
                            anchors.rightMargin: 20
                            anchors.top: parent.verticalCenter
                            anchors.topMargin: 1
                            text: resultRow.where
                            elide: Text.ElideLeft
                            color: Theme.inkDim
                            font.family: Theme.mono
                            font.pixelSize: Theme.textSmall
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
                            return found + (capped ? "+ matches (showing the first " + found + ")" : found === 1 ? " match" : " matches")
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
                // The table's sides and foot, drawn around however many rows
                // there are.
                Rectangle {
                    z: 1
                    anchors.left: listBrowser.left
                    anchors.right: listBrowser.right
                    anchors.top: listBrowser.top
                    height: Math.min(listBrowser.contentHeight, listBrowser.height) + 1
                    visible: listBrowser.visible
                    color: "transparent"
                    border.width: 1
                    border.color: Theme.cLine
                }
                ListView {
                    id: listBrowser
                    anchors.left: parent.left
                    anchors.right: parent.right
                    anchors.top: columnHeads.bottom
                    anchors.bottom: parent.bottom
                    anchors.leftMargin: 0
                    anchors.rightMargin: 0
                    anchors.bottomMargin: 8
                    visible: !root.deepSearch && !root.recentOpen && !root.trashOpen && root.view !== "grid" && root.rows.length > 0
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
                        color: Theme.wash(Theme.highlight, 0.12)
                        border.width: 1
                        border.color: Theme.highlight
                        radius: 0
                    }

                    delegate: Item {
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
                        readonly property string name: {
                            void root.revision
                            return root.bridge.entryName(entry)
                        }
                        readonly property bool dense: root.view === "dense"
                        // A hidden entry is shown at half strength.
                        readonly property real dim: root.isHidden(listRow.name) ? 0.5 : 1.0

                        readonly property string tag: root.tagOf(listRow.name, listRow.folder)

                        width: ListView.view.width
                        height: root.rowHeight

                        // One table: a panel with a hairline between rows. The
                        // held row takes the wash and a 2px accent bar.
                        Rectangle {
                            anchors.fill: parent
                            color: listRow.selected ? Theme.cWash
                                 : rowMouse.containsMouse ? Theme.cDim : Theme.cPanel
                            Behavior on color { ColorAnimation { duration: Theme.tFast } }
                        }
                        Rectangle {
                            anchors.left: parent.left
                            anchors.right: parent.right
                            anchors.top: parent.top
                            height: 1
                            color: Theme.cDead
                        }
                        Rectangle {
                            anchors.left: parent.left
                            anchors.top: parent.top
                            anchors.bottom: parent.bottom
                            width: 2
                            visible: listRow.selected
                            color: Theme.cGreen
                        }
                        Rectangle {
                            anchors.fill: parent
                            visible: root.focusRow === listRow.entry && !listRow.selected
                            color: "transparent"
                            border.width: 1
                            border.color: Theme.cLine
                        }

                        TypeTag {
                            id: rowTag
                            opacity: listRow.dim
                            x: 16
                            anchors.verticalCenter: parent.verticalCenter
                            kind: listRow.tag
                        }

                        Text {
                            opacity: listRow.dim
                            x: 62
                            width: (columnHeads.kindShown ? columnHeads.kindX : columnHeads.sizeShown ? columnHeads.sizeX : columnHeads.modifiedX) - x - 16
                            anchors.verticalCenter: parent.verticalCenter
                            text: listRow.name
                            elide: Text.ElideMiddle
                            color: Theme.cFg
                            font.family: Theme.fontHuman
                            font.pixelSize: 13
                            font.weight: Font.DemiBold
                        }

                        Text {
                            id: rowKind
                            visible: columnHeads.kindShown
                            opacity: listRow.dim
                            x: columnHeads.kindX
                            width: columnHeads.sizeX - x - 12
                            anchors.verticalCenter: parent.verticalCenter
                            elide: Text.ElideRight
                            text: root.kindLabel(listRow.name, listRow.folder)
                            color: Theme.cFg
                            font.family: Theme.fontHuman
                            font.pixelSize: 13
                        }

                        Text {
                            id: rowSize
                            visible: columnHeads.sizeShown
                            opacity: listRow.dim
                            x: columnHeads.sizeX
                            width: columnHeads.modifiedX - x - 12
                            anchors.verticalCenter: parent.verticalCenter
                            elide: Text.ElideRight
                            text: {
                                void root.revision
                                if (!listRow.folder)
                                    return root.bridge.entryDetail(listRow.entry)
                                const held = root.bridge.entryTotal(root.bridge.entryPath(listRow.entry), root.showHidden)
                                return held < 0 ? "—" : held + (held === 1 ? " item" : " items")
                            }
                            color: Theme.cFg
                            font.family: Theme.fontMono
                            font.pixelSize: 12
                        }

                        Text {
                            id: rowModified
                            opacity: listRow.dim
                            x: columnHeads.modifiedX
                            width: parent.width - x - 12
                            anchors.verticalCenter: parent.verticalCenter
                            elide: Text.ElideRight
                            text: {
                                void root.revision
                                return root.stampText(root.bridge.entryModified(listRow.entry))
                            }
                            color: Theme.cFg
                            font.family: Theme.fontMono
                            font.pixelSize: 12
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
                                radius: 0
                                border.width: 1
                                border.color: Theme.highlight
                            }
                        }


                        MouseArea {
                            id: rowMouse
                            anchors.fill: parent
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
                    anchors.margins: 0
                    visible: !root.deepSearch && !root.recentOpen && !root.trashOpen && root.view === "grid" && root.rows.length > 0
                    clip: true
                    model: root.rows
                    cellWidth: Math.floor(width / Math.max(2, Math.floor(width / 200)))
                    cellHeight: 170
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
                        color: Theme.wash(Theme.highlight, 0.12)
                        border.width: 1
                        border.color: Theme.highlight
                        radius: 0
                    }

                    delegate: Item {
                        id: tileHost
                        required property int modelData
                        readonly property int entry: modelData
                        readonly property bool selected: {
                            void root.revision
                            return root.sweepMark[entry] === true || root.bridge.entrySelected(entry)
                        }
                        readonly property bool folder: {
                            void root.revision
                            return root.bridge.entryIsDirectory(entry)
                        }
                        readonly property string name: {
                            void root.revision
                            return root.bridge.entryName(entry)
                        }
                        readonly property string thumbnail: {
                            void root.revision
                            return tileHost.folder ? "" : root.bridge.entryThumbnail(tileHost.entry)
                        }
                        readonly property bool pictured: tileHost.thumbnail.length > 0 && cardPicture.status === Image.Ready

                        width: gridBrowser.cellWidth
                        height: gridBrowser.cellHeight
                        opacity: root.isHidden(tileHost.name) ? 0.5 : 1

                        readonly property string tag: root.tagOf(tileHost.name, tileHost.folder)

                        // The Store's app card: the type tag set large, the
                        // name, what it is, and a flag at the foot.
                        Rectangle {
                            id: card
                            anchors.fill: parent
                            anchors.margins: 7
                            color: tileHost.selected ? Theme.cWash : (tileMouse.containsMouse ? Theme.cDim : Theme.cPanel)
                            border.width: 1
                            border.color: tileHost.selected ? Theme.cGreen
                                        : root.focusRow === tileHost.entry || tileMouse.containsMouse ? Theme.cLine : Theme.cDead
                            clip: true
                            Behavior on color { ColorAnimation { duration: Theme.tFast } }

                            // A picture or video fills the top of its card.
                            Image {
                                id: cardPicture
                                anchors.left: parent.left
                                anchors.right: parent.right
                                anchors.top: parent.top
                                anchors.margins: 1
                                height: parent.height * 0.42
                                visible: tileHost.pictured
                                source: tileHost.thumbnail
                                fillMode: Image.PreserveAspectCrop
                                sourceSize: Qt.size(width * 2, height * 2)
                                asynchronous: true
                            }

                            Text {
                                x: 18
                                y: 16
                                visible: !tileHost.pictured
                                text: tileHost.tag
                                color: Theme.typeColor(tileHost.tag)
                                font.family: Theme.fontBrand
                                font.pixelSize: 26
                                font.weight: Font.ExtraBold
                            }

                            Column {
                                anchors.left: parent.left
                                anchors.right: parent.right
                                anchors.bottom: cardFlag.top
                                anchors.leftMargin: 18
                                anchors.rightMargin: 18
                                anchors.bottomMargin: 14
                                spacing: 5
                                Text {
                                    id: cardName
                                    width: parent.width
                                    text: tileHost.name
                                    elide: Text.ElideMiddle
                                    color: Theme.cFg
                                    font.family: Theme.fontHuman
                                    font.pixelSize: 14
                                    font.weight: Font.DemiBold
                                }
                                Text {
                                    width: parent.width
                                    elide: Text.ElideRight
                                    text: {
                                        void root.revision
                                        const when = root.stampText(root.bridge.entryModified(tileHost.entry))
                                        if (tileHost.folder) {
                                            const held = root.bridge.entryTotal(root.bridge.entryPath(tileHost.entry), root.showHidden)
                                            return (held < 0 ? "Folder" : held + (held === 1 ? " item" : " items")) + "  ·  " + when
                                        }
                                        return root.bridge.entryDetail(tileHost.entry) + "  ·  " + when
                                    }
                                    color: Theme.cMuted
                                    font.family: Theme.fontHuman
                                    font.pixelSize: 12
                                }
                            }

                            Row {
                                id: cardFlag
                                anchors.left: parent.left
                                anchors.bottom: parent.bottom
                                anchors.leftMargin: 18
                                anchors.bottomMargin: 16
                                spacing: 6
                                Rectangle { width: 6; height: 6; color: Theme.typeColor(tileHost.tag); anchors.verticalCenter: parent.verticalCenter }
                                Text {
                                    text: tileHost.tag
                                    color: Theme.typeColor(tileHost.tag)
                                    font.family: Theme.fontMono
                                    font.pixelSize: 9
                                    font.letterSpacing: 2
                                    font.bold: true
                                }
                                Rectangle { width: 6; height: 6; color: Theme.typeColor(tileHost.tag); anchors.verticalCenter: parent.verticalCenter }
                            }
                        }

                            // A folder takes what is dropped on it.
                            DropArea {
                                anchors.fill: parent
                                enabled: root.bridge.entryIsDirectory(tileHost.entry) && !tileHost.selected
                                onDropped: function (drop) { root.dropInto(drop, root.bridge.entryPath(tileHost.entry)) }
                                Rectangle {
                                    anchors.fill: parent
                                    visible: parent.containsDrag
                                    color: "transparent"
                                    radius: 0
                                    border.width: 1
                                    border.color: Theme.highlight
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

                        // The bookmark star a folder card carries, and the menu.
                        IconButton {
                            anchors.right: card.right
                            anchors.rightMargin: 40
                            anchors.top: card.top
                            anchors.topMargin: 8
                            width: 28
                            height: 28
                            implicitWidth: 28
                            implicitHeight: 28
                            visible: tileHost.folder && (tileMouse.containsMouse || hovered)
                            bare: true
                            icon: "star"
                            iconSize: 15
                            hint: "Bookmark"
                            onClicked: root.bridge.addBookmark(root.bridge.entryPath(tileHost.entry))
                        }
                        IconButton {
                            anchors.right: card.right
                            anchors.top: card.top
                            anchors.margins: 8
                            width: 28
                            height: 28
                            implicitWidth: 28
                            implicitHeight: 28
                            visible: tileMouse.containsMouse || hovered || tileHost.selected
                            bare: true
                            icon: "ellipsis"
                            iconSize: 15
                            hint: "More"
                            onClicked: root.openEntryMenu(tileHost.entry)
                        }
                    }
                }
            }

            // ---- Settings ----
            // A page in place of the folder: view options, the theme, and
            // how Thallium Files sits in the system.
            Rectangle {
                id: settingsPage
                anchors.fill: parent
                visible: root.settingsOpen
                color: Theme.canvas

                // Nothing under the page takes clicks while it is up.
                MouseArea { anchors.fill: parent; acceptedButtons: Qt.AllButtons }

                Flickable {
                    anchors.fill: parent
                    contentHeight: settingsBody.implicitHeight + 64
                    clip: true
                    boundsBehavior: Flickable.StopAtBounds

                    Column {
                        id: settingsBody
                        x: 40
                        y: 28
                        width: Math.min(settingsPage.width - 80, 640)
                        spacing: 14

                        Text {
                            text: "Settings"
                            color: Theme.ink
                            font.family: Theme.family
                            font.pixelSize: Theme.title
                            font.weight: Font.Medium
                        }
                        CrossRule { width: parent.width }

                        Overline { text: "VIEW" }
                        Check {
                            text: "Show hidden files (Ctrl+H)"
                            checked: root.showHidden
                            onToggled: function (on) {
                                root.showHidden = on
                                checked = Qt.binding(function () { return root.showHidden })
                            }
                        }
                        Check {
                            text: "Show the preview panel"
                            checked: root.previewWanted
                            onToggled: function (on) {
                                root.togglePreview()
                                checked = Qt.binding(function () { return root.previewWanted })
                            }
                        }

                        CrossRule { width: parent.width }
                        Overline { text: "THEME" }
                    Chips {
                        width: parent.width
                        options: [{ key: "thallium", label: "Follow Thallium" }, { key: "custom", label: "Custom" }]
                        current: Theme.custom ? "custom" : "thallium"
                        onPicked: function (key) { root.setThemeCustom(key === "custom") }
                    }

                    Text {
                        visible: !Theme.custom
                        width: parent.width
                        wrapMode: Text.Wrap
                        text: "Colours follow the variant and accent picked in Thallium Settings, Appearance."
                        color: Theme.inkDim
                        font.family: Theme.family
                        font.pixelSize: Theme.text
                    }

                    Column {
                        visible: Theme.custom
                        width: parent.width
                        spacing: 12

                        Overline { text: "PRESETS" }
                        Chips {
                            width: parent.width
                            options: root.themePresets
                            current: root.themePreset
                            onPicked: function (key) {
                                const preset = root.themePresets.find(function (p) { return p.key === key })
                                root.setThemeColors(preset.colors)
                            }
                        }

                        CrossRule { width: parent.width }

                        Repeater {
                            model: Theme.editable
                            delegate: ColorField {
                                required property var modelData
                                width: parent.width
                                label: modelData.label
                                value: Theme.base[modelData.key]
                                onEdited: function (value) { root.setThemeColor(modelData.key, value) }
                            }
                        }

                        CrossRule { width: parent.width }

                        Row {
                            width: parent.width
                            spacing: 8
                            Rectangle {
                                width: parent.width - saveTheme.width - deleteTheme.width - 16
                                height: Theme.controlHeight
                                radius: Theme.radiusSmall
                                color: Theme.inset
                                border.width: 1
                                border.color: presetName.activeFocus ? Theme.outline : Theme.outlineSoft
                                TextInput {
                                    id: presetName
                                    anchors.fill: parent
                                    anchors.leftMargin: 12
                                    anchors.rightMargin: 12
                                    verticalAlignment: TextInput.AlignVCenter
                                    clip: true
                                    color: Theme.ink
                                    selectionColor: Theme.accentWash
                                    selectedTextColor: Theme.ink
                                    font.family: Theme.family
                                    font.pixelSize: Theme.text
                                    onAccepted: saveTheme.clicked()
                                    Text {
                                        anchors.verticalCenter: parent.verticalCenter
                                        visible: presetName.text.length === 0
                                        text: "Name for preset"
                                        color: Theme.inkFaint
                                        font: presetName.font
                                    }
                                }
                            }
                            TextButton {
                                id: saveTheme
                                text: "Save as preset"
                                enabled: presetName.text.trim().length > 0
                                onClicked: {
                                    root.saveThemePreset(presetName.text)
                                    presetName.text = ""
                                }
                            }
                            TextButton {
                                id: deleteTheme
                                text: "Delete preset"
                                enabled: root.themePreset.startsWith("u")
                                onClicked: root.deleteThemePreset(root.themePreset)
                            }
                        }

                        // The theme as text, to share it or paste one in.
                        Overline { text: "THEME CODE" }
                        Rectangle {
                            width: parent.width
                            height: Theme.controlHeight
                            radius: Theme.radiusSmall
                            color: Theme.inset
                            border.width: 1
                            border.color: themeCode.activeFocus ? Theme.outline : Theme.outlineSoft
                            TextInput {
                                id: themeCode
                                anchors.fill: parent
                                anchors.leftMargin: 12
                                anchors.rightMargin: 12
                                verticalAlignment: TextInput.AlignVCenter
                                clip: true
                                selectByMouse: true
                                text: JSON.stringify(Theme.base)
                                color: Theme.inkSoft
                                selectionColor: Theme.accentWash
                                selectedTextColor: Theme.ink
                                font.family: Theme.mono
                                font.pixelSize: 11
                                onEditingFinished: {
                                    try {
                                        const pasted = JSON.parse(themeCode.text)
                                        if (root.validColors(pasted))
                                            root.setThemeColors(pasted)
                                    } catch (error) {
                                    }
                                    themeCode.text = Qt.binding(function () { return JSON.stringify(Theme.base) })
                                }
                            }
                        }
                    }

                        CrossRule { width: parent.width }
                        Overline { text: "SYSTEM" }
                        Flow {
                            width: parent.width
                            spacing: 8
                            TextButton {
                                text: "Edit custom actions…"
                                onClicked: root.bridge.editActions()
                            }
                            TextButton {
                                text: root.defaultManager ? "Thallium Files is the default" : "Make Thallium Files the default"
                                enabled: !root.defaultManager
                                onClicked: {
                                    root.bridge.makeDefaultManager()
                                    root.defaultManager = root.bridge.isDefaultManager()
                                }
                            }
                        }
                    }
                }
            }
        }
        // ============================= SPLIT ==============================
        SplitPane {
            id: splitPane
            width: root.splitOpen ? Math.round((parent.width - root.sidebarWidth - (root.previewOpen ? root.inspectorWidth : 0)) * 0.45) : 0
            height: parent.height
            visible: root.splitOpen
            bridge: root.splitBridge
            showHidden: root.showHidden
            timeText: root.modifiedText
            onCloseRequested: root.toggleSplit()
            onDropped: function (drop, path) { root.dropInto(drop, path) }
        }

        // =========================== PREVIEW =============================
        // A panel, not a fixture: it folds away and gives the width back.
        Rectangle {
            id: inspector
            width: root.previewOpen ? root.inspectorWidth : 0
            height: parent.height
            visible: width > 0
            clip: true
            color: Theme.cBase

            Behavior on width {
                NumberAnimation { duration: Theme.tMed; easing.type: Easing.OutCubic }
            }

            // The entry described: the one held, or none.
            readonly property int row: {
                void root.revision
                if (root.selectedCount !== 1)
                    return -1
                return root.focusRow >= 0 && root.bridge.entrySelected(root.focusRow)
                       ? root.focusRow : root.firstSelectedRow()
            }
            readonly property bool single: inspector.row >= 0
            readonly property string entryName: {
                void root.revision
                return inspector.single ? root.bridge.entryName(inspector.row) : ""
            }
            readonly property bool entryFolder: {
                void root.revision
                return inspector.single && root.bridge.entryIsDirectory(inspector.row)
            }
            onRowChanged: if (inspector.row >= 0) {
                root.bridge.inspect(inspector.row)
                root.propTick++
            }
            function text(field) {
                void root.propTick
                void root.revision
                return inspector.single ? root.bridge.propText(field) : ""
            }
            function number(field) {
                void root.propTick
                void root.revision
                return inspector.single ? root.bridge.propNumber(field) : 0
            }
            readonly property string sizeText: inspector.entryFolder
                ? (inspector.number("measuring") === 1 && inspector.number("files") === 0 ? "Counting…"
                   : root.readableSize(inspector.number("size")) + " in " + inspector.number("files")
                     + (inspector.number("files") === 1 ? " file" : " files"))
                : root.readableSize(inspector.number("size"))

            readonly property string entryTag: root.tagOf(inspector.entryName, inspector.entryFolder)
            readonly property string entryPath: {
                void root.revision
                return inspector.single ? root.bridge.entryPath(inspector.row) : ""
            }
            // Where the record sits in the list, counted from one.
            readonly property int position: root.rows.indexOf(inspector.row) + 1
            readonly property string preview: {
                void root.revision
                const tag = inspector.entryTag
                return inspector.single && (tag === "CODE" || tag === "TXT") && !inspector.entryFolder
                       ? root.bridge.textHead(inspector.entryPath, 3) : ""
            }
            readonly property string placeText: {
                const folder = inspector.text("folder")
                const home = root.bridge.homePath
                if (home.length > 0 && (folder === home || folder.startsWith(home + "/")))
                    return "~" + folder.substring(home.length)
                return folder
            }

            Rectangle {
                anchors.left: parent.left
                width: 1
                height: parent.height
                color: Theme.cDead
            }

            Flickable {
                id: recordView
                anchors.fill: parent
                anchors.leftMargin: 1
                contentHeight: recordColumn.implicitHeight + 48
                clip: true
                boundsBehavior: Flickable.StopAtBounds

                Column {
                    id: recordColumn
                    x: 22
                    y: 26
                    width: recordView.width - 44
                    spacing: 26

                    // ---- The record: one held entry ----
                    Item {
                        id: recordCard
                        visible: inspector.single
                        width: parent.width
                        height: visible ? recordBody.implicitHeight + 36 : 0

                        ChamferPanel {
                            anchors.fill: parent
                            fill: Theme.mix(Theme.cPanel, Theme.cWash, 0.25)
                        }
                        CornerBrackets {
                            anchors.fill: parent
                            anchors.margins: 6
                        }
                        // The tag, huge and outlined, as a watermark.
                        Text {
                            anchors.right: parent.right
                            anchors.bottom: parent.bottom
                            anchors.rightMargin: -6
                            anchors.bottomMargin: 58
                            text: inspector.entryTag
                            color: "transparent"
                            style: Text.Outline
                            styleColor: Theme.cLine
                            opacity: 0.32
                            font.family: Theme.fontBrand
                            font.pixelSize: 104
                            font.weight: Font.ExtraBold
                        }

                        Column {
                            id: recordBody
                            x: 22
                            y: 20
                            width: parent.width - 44
                            spacing: 16

                            // RECORD 05 / 08, and a tick per entry.
                            Item {
                                width: parent.width
                                height: 14
                                Text {
                                    anchors.verticalCenter: parent.verticalCenter
                                    textFormat: Text.StyledText
                                    text: "RECORD  <font color='" + Theme.cGreen + "'><b>" + root.pad2(inspector.position)
                                          + "</b></font>  /  " + root.pad2(root.rows.length)
                                    color: Theme.cMuted
                                    font.family: Theme.fontMono
                                    font.pixelSize: 10
                                    font.letterSpacing: 3
                                }
                                Row {
                                    anchors.right: parent.right
                                    anchors.verticalCenter: parent.verticalCenter
                                    spacing: 3
                                    Repeater {
                                        model: 8
                                        delegate: Rectangle {
                                            required property int index
                                            // Which eighth of the list the record falls in.
                                            readonly property int at: root.rows.length > 0
                                                ? Math.min(7, Math.floor((inspector.position - 1) * 8 / root.rows.length)) : -1
                                            width: 4
                                            height: 10
                                            color: index === at ? Theme.cGreen : Theme.cDead
                                            antialiasing: true
                                            transform: Matrix4x4 {
                                                matrix: Qt.matrix4x4(1, Theme.shear, 0, -Theme.shear * 10, 0, 1, 0, 0, 0, 0, 1, 0, 0, 0, 0, 1)
                                            }
                                        }
                                    }
                                }
                            }
                            DashRule { width: parent.width; height: 1 }

                            // The tile, the kind and the name.
                            Row {
                                width: parent.width
                                spacing: 16
                                Rectangle {
                                    width: 64
                                    height: 64
                                    color: Theme.cDim
                                    border.width: 1
                                    border.color: Theme.cLine
                                    clip: true
                                    // Hatching, for the tile's ground.
                                    Canvas {
                                        anchors.fill: parent
                                        anchors.margins: 1
                                        onPaint: {
                                            const ctx = getContext("2d")
                                            ctx.reset()
                                            ctx.strokeStyle = Theme.cDead
                                            ctx.lineWidth = 1
                                            for (let k = -height; k < width; k += 6) {
                                                ctx.beginPath()
                                                ctx.moveTo(k, height)
                                                ctx.lineTo(k + height, 0)
                                                ctx.stroke()
                                            }
                                        }
                                    }
                                    Text {
                                        anchors.centerIn: parent
                                        text: inspector.entryTag
                                        color: Theme.typeColor(inspector.entryTag)
                                        font.family: Theme.fontBrand
                                        font.pixelSize: inspector.entryTag.length > 3 ? 12 : 15
                                        font.weight: Font.ExtraBold
                                    }
                                }
                                Column {
                                    width: parent.width - 80
                                    anchors.verticalCenter: parent.verticalCenter
                                    spacing: 6
                                    Text {
                                        text: root.kindLabel(inspector.entryName, inspector.entryFolder).toUpperCase()
                                        color: Theme.typeColor(inspector.entryTag)
                                        font.family: Theme.fontMono
                                        font.pixelSize: 10
                                        font.letterSpacing: 3
                                    }
                                    Text {
                                        width: parent.width
                                        text: inspector.entryName
                                        elide: Text.ElideMiddle
                                        color: Theme.cFg
                                        font.family: Theme.fontBrand
                                        font.pixelSize: 22
                                        font.weight: Font.DemiBold
                                    }
                                }
                            }

                            // MODIFIED | LOCATION
                            Row {
                                width: parent.width
                                Repeater {
                                    model: [
                                        { label: "MODIFIED", value: root.stampText(inspector.number("modified")) },
                                        { label: "LOCATION", value: inspector.placeText }
                                    ]
                                    delegate: Column {
                                        required property var modelData
                                        width: recordBody.width / 2
                                        spacing: 6
                                        Text {
                                            text: modelData.label
                                            color: Theme.cMuted
                                            font.family: Theme.fontMono
                                            font.pixelSize: 9
                                            font.letterSpacing: 3
                                        }
                                        Text {
                                            width: parent.width - 8
                                            text: modelData.value
                                            elide: Text.ElideLeft
                                            color: Theme.cFg
                                            font.family: Theme.fontMono
                                            font.pixelSize: 12
                                        }
                                    }
                                }
                            }

                            // The first lines of a text or code file.
                            Rectangle {
                                visible: inspector.preview.length > 0
                                width: parent.width
                                height: visible ? previewLines.implicitHeight + 16 : 0
                                color: Theme.cBase
                                border.width: 1
                                border.color: Theme.cDead
                                Column {
                                    id: previewLines
                                    x: 10
                                    y: 8
                                    width: parent.width - 20
                                    Repeater {
                                        model: inspector.preview.length > 0 ? inspector.preview.split("\n") : []
                                        delegate: Row {
                                            required property string modelData
                                            required property int index
                                            spacing: 10
                                            Text {
                                                text: index + 1
                                                width: 12
                                                horizontalAlignment: Text.AlignRight
                                                color: Theme.cLine
                                                font.family: Theme.fontMono
                                                font.pixelSize: 11
                                            }
                                            Text {
                                                width: previewLines.width - 22
                                                text: modelData
                                                elide: Text.ElideRight
                                                color: Theme.cFg
                                                font.family: Theme.fontMono
                                                font.pixelSize: 11
                                            }
                                        }
                                    }
                                }
                            }

                            // Open, then the icon actions.
                            Row {
                                id: recordActions
                                spacing: 10
                                TextButton {
                                    primary: true
                                    text: "Open  ↵"
                                    width: 82
                                    onClicked: root.activateRow(inspector.row)
                                }
                                IconButton {
                                    width: 40
                                    height: 40
                                    icon: "terminal"
                                    hint: "Open terminal here (Ctrl+Alt+T)"
                                    onClicked: root.bridge.openTerminalHere()
                                }
                                IconButton {
                                    width: 40
                                    height: 40
                                    icon: "copy"
                                    hint: "Copy path (Ctrl+Shift+C)"
                                    onClicked: root.copyToClipboard(inspector.entryPath)
                                }
                                IconButton {
                                    id: recordMore
                                    width: 40
                                    height: 40
                                    icon: "ellipsis"
                                    hint: "More"
                                    onClicked: root.openEntryMenu(inspector.row)
                                }
                            }
                        }
                    }

                    // ---- Several held ----
                    Item {
                        visible: root.selectedCount > 1
                        width: parent.width
                        height: visible ? 150 : 0
                        ChamferPanel { anchors.fill: parent }
                        CornerBrackets { anchors.fill: parent; anchors.margins: 6 }
                        Column {
                            anchors.centerIn: parent
                            spacing: 6
                            Text {
                                anchors.horizontalCenter: parent.horizontalCenter
                                text: root.pad2(root.selectedCount)
                                color: Theme.cFg
                                font.family: Theme.fontBrand
                                font.pixelSize: 40
                                font.weight: Font.ExtraBold
                            }
                            Text {
                                anchors.horizontalCenter: parent.horizontalCenter
                                text: "ITEMS HELD"
                                color: Theme.cMuted
                                font.family: Theme.fontMono
                                font.pixelSize: 10
                                font.letterSpacing: 3
                            }
                        }
                    }

                    // ---- The disk ----
                    Column {
                        id: storagePanel
                        width: parent.width
                        spacing: 16
                        visible: storagePanel.total > 0

                        readonly property real total: {
                            void root.revision
                            return root.bridge.storageTotal()
                        }
                        readonly property real used: {
                            void root.revision
                            return root.bridge.storageUsed()
                        }

                        HudHeader { width: parent.width; title: "Storage"; sub: "/" }
                        Row {
                            spacing: 20
                            Donut {
                                id: diskRing
                                readonly property real share: storagePanel.total > 0 ? storagePanel.used / storagePanel.total : 0
                                segments: [
                                    { value: storagePanel.used, color: Theme.cGreen },
                                    { value: Math.max(0, storagePanel.total - storagePanel.used), color: Theme.cDead }
                                ]
                                figure: Math.round(diskRing.share * 100) + "%"
                                caption: "USED"
                            }
                            Column {
                                anchors.verticalCenter: parent.verticalCenter
                                spacing: 10
                                Repeater {
                                    model: [
                                        { label: "Used", value: root.readableSize(storagePanel.used), color: Theme.cGreen },
                                        { label: "Free", value: root.readableSize(Math.max(0, storagePanel.total - storagePanel.used)), color: Theme.cDead },
                                        { label: "Total", value: root.readableSize(storagePanel.total), color: "transparent" }
                                    ]
                                    delegate: Legend {}
                                }
                            }
                        }
                    }

                    // ---- What this folder holds ----
                    Column {
                        width: parent.width
                        spacing: 16
                        visible: !root.trashOpen && !root.recentOpen
                        HudHeader { width: parent.width; title: "This folder"; sub: "composition" }
                        Row {
                            spacing: 20
                            Donut {
                                segments: root.composition.map(function (kind, i) { return { value: kind.count, color: Theme.chart[i] } })
                                figure: root.pad2(root.rows.length)
                                caption: root.rows.length === 1 ? "ITEM" : "ITEMS"
                            }
                            Column {
                                anchors.verticalCenter: parent.verticalCenter
                                spacing: 10
                                Repeater {
                                    model: root.composition.map(function (kind, i) {
                                        return { label: kind.label, value: String(kind.count), color: Theme.chart[i] }
                                    }).filter(function (row) { return row.value !== "0" })
                                    delegate: Legend {}
                                }
                            }
                        }
                    }

                    // ---- Move to trash: armed, then a second click within 3s ----
                    Item {
                        id: trashLink
                        visible: inspector.single && !root.trashOpen
                        width: parent.width
                        height: visible ? 40 : 0
                        property bool armed: false
                        Timer { id: trashDisarm; interval: 3000; onTriggered: trashLink.armed = false }
                        Rectangle { width: parent.width; height: 1; color: Theme.cDead }
                        Text {
                            anchors.left: parent.left
                            anchors.bottom: parent.bottom
                            text: trashLink.armed ? "CLICK AGAIN TO TRASH" : "MOVE TO TRASH"
                            color: Theme.cRed
                            font.family: Theme.fontMono
                            font.pixelSize: 11
                            font.letterSpacing: 3
                            font.bold: trashLink.armed
                            MouseArea {
                                anchors.fill: parent
                                anchors.margins: -6
                                cursorShape: Qt.PointingHandCursor
                                onClicked: {
                                    if (!trashLink.armed) {
                                        trashLink.armed = true
                                        trashDisarm.restart()
                                        return
                                    }
                                    trashLink.armed = false
                                    trashDisarm.stop()
                                    root.bridge.trashSelected()
                                }
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
            radius: 0
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
                        radius: 0
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
                            radius: 0
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
                                radius: 0
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
