import QtQuick
import QtQuick.Controls.Basic
FluidMenu {
    id:root;objectName:"messageActionsMenu"
    property var message:({})
    property var threadKey:({})
    readonly property bool allowed:Mail.selectionCount<=1&&!!message.id&&threadKey.account===Mail.reader.key?.account&&threadKey.id===Mail.reader.key?.id
    function perform(action){if(allowed)Mail.mutateMessage(action,message)}
    signal detailsRequested(string message)
    FluidMenuItem{objectName:"messageDetailsAction";text:"Message details";enabled:root.allowed;onTriggered:root.detailsRequested(root.message.id)}
    FluidMenuItem{text:"Open Gmail draft";visible:!!root.message.draft;height:visible?implicitHeight:0;enabled:root.allowed;onTriggered:Mail.requestCompose("draft",root.message)}
    FluidMenuItem{text:"Reply to this message";visible:!root.message.draft;height:visible?implicitHeight:0;enabled:root.allowed;onTriggered:Mail.requestCompose("reply",root.message)}
    FluidMenuItem{text:"Reply all";visible:!root.message.draft;height:visible?implicitHeight:0;enabled:root.allowed;onTriggered:Mail.requestCompose("replyAll",root.message)}
    FluidMenuItem{text:"Forward this message";enabled:root.allowed;onTriggered:Mail.requestCompose("forward",root.message)}
    FluidMenuSeparator{}
    FluidMenuItem{objectName:"messageReadAction";text:root.message.unread?"Mark this message read":"Mark this message unread";enabled:root.allowed;onTriggered:root.perform(root.message.unread?"read":"unread")}
    FluidMenuItem{objectName:"messageStarAction";text:root.message.starred?"Remove this message’s star":"Star this message";enabled:root.allowed;onTriggered:root.perform(root.message.starred?"unstar":"star")}
    FluidMenuItem{objectName:"messageImportantAction";text:root.message.important?"Mark this message unimportant":"Mark this message important";enabled:root.allowed;onTriggered:root.perform(root.message.important?"unimportant":"important")}
    FluidMenuSeparator{}
    FluidMenuItem{text:"Archive this message";visible:!root.message.draft;height:visible?implicitHeight:0;enabled:root.allowed;onTriggered:root.perform("archive")}
    FluidMenuItem{text:"Move this message to Trash";visible:!root.message.draft;height:visible?implicitHeight:0;enabled:root.allowed;onTriggered:root.perform("trash")}
    Connections{target:Mail;function onReaderChanged(){root.close()}function onSelectionChanged(){if(Mail.selectionCount>1)root.close()}}
}
