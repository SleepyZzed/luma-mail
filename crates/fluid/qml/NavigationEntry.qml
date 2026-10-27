import QtQuick

Item {
    id: root
    required property var navigation
    required property string folder
    property string scope: ""
    property bool disclosed: true
    readonly property bool selected: Mail.folder === folder && Mail.account === scope
    readonly property string identity: scope + ":" + folder
    width: parent.width
    height: Theme.unit(40)
    Component.onCompleted: navigation.registerEntry(root)
    Component.onDestruction: navigation.unregisterEntry(root)
    onYChanged: Qt.callLater(navigation.refreshPill)
    FluidButton {
        anchors.fill: parent
        anchors.leftMargin: Theme.unit(3)
        glyph: ({inbox:"inbox",all_mail:"mail",unread:"unread",starred:"star",sent:"send",drafts:"compose",archive:"archive",spam:"spam",trash:"trash"})[root.folder] || "label"
        text: root.navigation.collapsed ? "" : Theme.folderTitle(root.folder)
        tip: Theme.folderTitle(root.folder)
        tint: root.selected ? Theme.accent : Theme.muted
        onClicked: Mail.setView(root.folder, root.scope)
    }
}
