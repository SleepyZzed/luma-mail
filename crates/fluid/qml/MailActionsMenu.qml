import QtQuick
import QtQuick.Controls.Basic
FluidMenu {
    id:root
    property var row:({})
    property var state:({count:0})
    signal confirmationRequested(string heading,string message,string buttonText,string command,var parameters)
    function refresh(){state=Mail.actionState(row);if(state.account)Mail.ensureLabels(state.account)}
    onAboutToShow:refresh()
    FluidMenuItem{text:"Open draft";visible:root.state.count===1&&Mail.folder==="drafts";height:visible?implicitHeight:0;onTriggered:Mail.composeFor("draft",root.state.key)}
    FluidMenuItem{objectName:"deleteDraftAction";text:root.state.count===1?"Delete draft…":"Delete selected drafts…";visible:root.state.count>0&&Mail.folder==="drafts";height:visible?implicitHeight:0;onTriggered:root.confirmationRequested("Delete Gmail drafts?","Permanently discard drafts in "+root.state.count+" selected conversation(s). Sent and received messages are kept. This cannot be undone.","Delete drafts","draftDelete",{keys:root.state.keys,confirmed:true})}
    FluidMenuItem{text:"Reply";visible:root.state.count===1&&Mail.folder!=="drafts";height:visible?implicitHeight:0;onTriggered:Mail.composeFor("reply",root.state.key)}
    FluidMenuItem{text:"Reply all";visible:root.state.count===1&&Mail.folder!=="drafts";height:visible?implicitHeight:0;onTriggered:Mail.composeFor("replyAll",root.state.key)}
    FluidMenuItem{text:"Forward";visible:root.state.count===1&&Mail.folder!=="drafts";height:visible?implicitHeight:0;onTriggered:Mail.composeFor("forward",root.state.key)}
    FluidMenuSeparator{}
    FluidMenuItem{text:"Archive";enabled:root.state.count>0;visible:Mail.folder!=="archive"&&Mail.folder!=="trash"&&Mail.folder!=="drafts";height:visible?implicitHeight:0;onTriggered:Mail.mutate("archive",root.row)}
    FluidMenuItem{text:"Move to Trash";enabled:root.state.count>0;visible:Mail.folder!=="trash";height:visible?implicitHeight:0;onTriggered:Mail.mutate("trash",root.row)}
    FluidMenuItem{text:root.state.unread?"Mark read":"Mark unread";enabled:root.state.count>0;onTriggered:Mail.mutate(root.state.unread?"read":"unread",root.row)}
    FluidMenuItem{text:root.state.starred?"Remove star":"Star";enabled:root.state.count>0;onTriggered:Mail.mutate(root.state.starred?"unstar":"star",root.row)}
    FluidMenuItem{objectName:"importantAction";text:root.state.important?"Mark unimportant":"Mark important";enabled:root.state.count>0;onTriggered:Mail.mutate(root.state.important?"unimportant":"important",root.row)}
    FluidMenuItem{objectName:"moveInboxAction";text:"Move to Inbox";enabled:root.state.count>0;visible:Mail.folder!=="inbox";height:visible?implicitHeight:0;onTriggered:Mail.mutate("inbox",root.row)}
    FluidMenuItem{text:Mail.folder==="spam"?"Not spam":"Report spam";enabled:root.state.count>0;onTriggered:Mail.mutate(Mail.folder==="spam"?"notSpam":"spam",root.row)}
    FluidMenuItem{text:"Restore from Trash";enabled:root.state.count>0;visible:Mail.folder==="trash";height:visible?implicitHeight:0;onTriggered:Mail.mutate("restore",root.row)}
    FluidMenu{id:labels;title:"Add label";enabled:root.state.count>0
        onOpened:labelList.forceActiveFocus()
        contentItem:ListView{id:labelList;implicitWidth:Theme.unit(250);implicitHeight:Math.max(1,Math.min(count,8))*Theme.unit(36);model:Mail.labelSets[root.state.account]||[];clip:true;reuseItems:true;cacheBuffer:Theme.unit(36);keyNavigationEnabled:true
            ScrollBar.vertical:ScrollBar{implicitWidth:3}
            Keys.onReturnPressed:if(currentItem)currentItem.apply()
            Keys.onEnterPressed:if(currentItem)currentItem.apply()
            delegate:FluidMenuItem{required property var modelData;required property int index;width:labelList.width;height:Theme.unit(36);text:modelData.name;highlighted:index===labelList.currentIndex
                function apply(){Mail.mutate("addLabel",root.row,modelData.id);labels.close();root.close()}
                onHoveredChanged:if(hovered)labelList.currentIndex=index
                onTriggered:apply()
            }
            Text{textFormat:Text.PlainText;anchors.centerIn:parent;width:parent.width-Theme.unit(24);wrapMode:Text.Wrap;text:"Create labels in the account sidebar";visible:labelList.count===0;color:Theme.muted;font.pixelSize:Theme.unit(11)}
        }
    }
    FluidMenuItem{text:"Remove current label";enabled:root.state.count>0;visible:Mail.folder.startsWith("label:");height:visible?implicitHeight:0;onTriggered:Mail.mutate("removeLabel",root.row,Mail.folder.substring(6))}
    FluidMenuItem{objectName:"senderRuleAction";text:!root.state.ruleConnected?"Connect Gmail sender controls…":root.state.blocked?"Unblock sender…":"Block sender in Gmail…";visible:root.state.count===1&&Mail.folder!=="drafts"&&!!root.state.sender; height:visible?implicitHeight:0
        onTriggered:{if(!root.state.ruleConnected){Mail.send("authStart",{account:root.state.account,settings:true});return}root.confirmationRequested(root.state.blocked?"Unblock sender?":"Block sender in Gmail?",root.state.blocked?"Remove only Luma’s unchanged Gmail filter for "+root.state.address+"?":"Future mail from "+root.state.address+" will go to Trash on Gmail, including when Luma is closed. Existing messages are kept.",root.state.blocked?"Unblock":"Block sender","senderRule",{account:root.state.account,sender:root.state.sender,operation:root.state.blocked?"unblock":"block",confirmed:true})}
    }
    FluidMenuSeparator{}
    FluidMenuItem{text:"Select all loaded";onTriggered:Mail.selectAll()}
    Connections{target:Mail;function onSelectionChanged(){if(root.visible)root.refresh()}function onSettingsChanged(){if(root.visible)root.refresh()}}
}
