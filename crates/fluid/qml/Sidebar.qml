import QtQuick
import QtQuick.Controls
Item {
    id:root;objectName:"navigation"
    property bool collapsed:false
    property bool unifiedOpen:false
    property string accountOpen:""
    property bool overlayOpen:false
    property var entries:({})
    property rect pillRect:Qt.rect(0,0,0,0)
    function registerEntry(entry){entries[entry.identity]=entry;Qt.callLater(refreshPill)}
    function unregisterEntry(entry){if(entries[entry.identity]===entry)delete entries[entry.identity]}
    function refreshPill(){
        const entry=entries[Mail.account+":"+Mail.folder]
        if(!entry||!entry.disclosed){pillRect=Qt.rect(pillRect.x,pillRect.y,pillRect.width,0);return}
        const p=entry.mapToItem(scroll.contentItem,0,0)
        pillRect=Qt.rect(p.x+Theme.unit(2),p.y+Theme.unit(2),entry.width-Theme.unit(4),entry.height-Theme.unit(4))
    }
    onWidthChanged:Qt.callLater(refreshPill)
    onUnifiedOpenChanged:Qt.callLater(refreshPill)
    onAccountOpenChanged:Qt.callLater(refreshPill)
    Connections{target:Mail;function onViewChanged(){if(Mail.account)root.accountOpen=Mail.account;Qt.callLater(root.refreshPill)}function onSettingsChanged(){if(root.accountOpen&&!((Mail.settings.accounts||[]).find(a=>a.id===root.accountOpen&&a.enabled!==false)))root.accountOpen=""}}
    signal composeAt(real x,real y,real w,real h)
    signal settingsRequested()
    signal labelActionRequested(string operation,string account,var label)
    clip:true
    Rectangle{anchors.fill:parent;color:Theme.rail}
    Rectangle{width:1;anchors.right:parent.right;height:parent.height;color:Theme.outline;opacity:.55}
    Item {
        id:fixedHeader;x:Theme.unit(22);y:Theme.unit(18);width:parent.width-2*x;height:Theme.unit(103)
        Image {id:logo;source:root.collapsed?"qrc:/brand/io.github.lumamail.Luma.svg":"qrc:/brand/luma-wordmark.png";width:root.collapsed?Theme.unit(32):Theme.unit(105);height:Theme.unit(36);fillMode:Image.PreserveAspectFit}
        Text {textFormat:Text.PlainText;x:root.collapsed?0:Theme.unit(1);y:Theme.unit(40);text:"Online-first mail";color:Theme.muted;font.pixelSize:Theme.unit(10);visible:!root.collapsed}
        FluidButton {
            id:composeButton;y:Theme.unit(63);width:parent.width;height:Theme.unit(41);primary:true;glyph:"compose";text:root.collapsed?"":"Compose";tip:"New message · Ctrl+N"
            onClicked:{let p=mapToItem(null,0,0);root.composeAt(p.x,p.y,width,height)}
        }
    }
    Flickable {
        id:scroll;objectName:"navigationScroll";x:Theme.unit(12);y:fixedHeader.y+fixedHeader.height+Theme.unit(25);width:parent.width-2*x;height:footer.y-y-Theme.unit(14);clip:true;contentHeight:nav.height;boundsBehavior:Flickable.StopAtBounds
        onContentHeightChanged:contentY=Math.max(0,Math.min(contentY,contentHeight-height))
        onHeightChanged:contentY=Math.max(0,Math.min(contentY,contentHeight-height))
        ScrollBar.vertical:ScrollBar{policy:ScrollBar.AsNeeded;implicitWidth:5}
        Rectangle {
            id:selectionPill
            x:root.pillRect.x;y:pillVertical.value;width:root.pillRect.width;height:Math.max(0,pillHeight.value)
            radius:Theme.unit(11);color:Theme.selected
            Motion{id:pillVertical;target:root.pillRect.y;enabled:Theme.motion}
            Motion{id:pillHeight;target:root.pillRect.height;enabled:Theme.motion}
        }
        Column {
            id:nav;width:scroll.width;spacing:Theme.unit(6)
            FluidButton {
                width:parent.width;text:root.collapsed?"":"Unified mail";glyph:"inbox";quiet:false;tip:root.unifiedOpen?"Collapse unified folders":"Expand unified folders";onClicked:root.unifiedOpen=!root.unifiedOpen
                Icon{anchors.right:parent.right;anchors.rightMargin:Theme.unit(13);anchors.verticalCenter:parent.verticalCenter;width:Theme.unit(12);height:width;name:"chevron-right";color:Theme.muted;visible:!root.collapsed;rotation:unifiedArrow.value;Motion{id:unifiedArrow;target:root.unifiedOpen?90:0;enabled:Theme.motion}}
            }
            Item {
                width:parent.width;height:Math.max(0,unifiedHeight.value);clip:true
                Motion{id:unifiedHeight;target:root.unifiedOpen?folderColumn.implicitHeight:0;enabled:Theme.motion}
                onHeightChanged:Qt.callLater(root.refreshPill)
                Column {
                    id:folderColumn;width:parent.width
                    Repeater {
                        model:[{key:"inbox",glyph:"inbox"},{key:"all_mail",glyph:"mail"},{key:"unread",glyph:"unread"},{key:"starred",glyph:"star"},{key:"sent",glyph:"send"},{key:"drafts",glyph:"compose"},{key:"archive",glyph:"archive"},{key:"spam",glyph:"spam"},{key:"trash",glyph:"trash"}]
                        delegate:NavigationEntry {
                            required property var modelData
                            navigation:root;folder:modelData.key;disclosed:root.unifiedOpen
                        }
                    }
                }
            }
            Item{width:1;height:Theme.unit(23)}
            Text {textFormat:Text.PlainText;x:Theme.unit(12);text:"YOUR ACCOUNTS";font.pixelSize:Theme.unit(9);font.letterSpacing:1.2;font.weight:Font.DemiBold;color:Theme.faint;visible:!root.collapsed}
            Repeater {
                model:Mail.accounts
                delegate:Column {
                    id:accountCard;required property var item;width:nav.width
                    Item {
                        width:parent.width;height:Theme.unit(62)
                        Avatar{objectName:"accountAvatar-"+accountCard.item.id;x:Theme.unit(10);anchors.verticalCenter:parent.verticalCenter;width:Theme.unit(32);height:width;sender:accountCard.item.name||accountCard.item.email;photoSender:accountCard.item.email;account:accountCard.item.id;active:accountCard.y+parent.height>scroll.contentY&&accountCard.y<scroll.contentY+scroll.height}
                        Column {x:Theme.unit(53);anchors.verticalCenter:parent.verticalCenter;visible:!root.collapsed;width:parent.width-x-Theme.unit(28);spacing:3
                            Text {textFormat:Text.PlainText;text:accountCard.item.name||accountCard.item.email;color:Theme.text;font.pixelSize:Theme.unit(12);elide:Text.ElideRight;width:parent.width}
                            Text {textFormat:Text.PlainText;text:accountCard.item.email;color:Theme.faint;font.pixelSize:Theme.unit(10);elide:Text.ElideMiddle;width:parent.width}
                        }
                        Text{textFormat:Text.PlainText;anchors.right:parent.right;anchors.rightMargin:Theme.unit(9);y:Theme.unit(39)
                            text:{const count=Mail.sync.unread?.[accountCard.item.id]||0;return count>999?"999+":String(count)}
                            visible:text!=="0"&&!root.collapsed;color:Theme.accent;font.pixelSize:Theme.unit(9)
                        }
                        Icon{anchors.right:parent.right;anchors.rightMargin:Theme.unit(9);anchors.verticalCenter:parent.verticalCenter;width:Theme.unit(12);height:width;name:"chevron-right";color:Theme.muted;visible:!root.collapsed;rotation:accountArrow.value;Motion{id:accountArrow;target:root.accountOpen===accountCard.item.id?90:0;enabled:Theme.motion}}
                        MouseArea{anchors.fill:parent;enabled:accountCard.item.enabled!==false;onClicked:{root.accountOpen=root.accountOpen===accountCard.item.id?"":accountCard.item.id;if(root.accountOpen!=="")Mail.setView("inbox",accountCard.item.id)}}
                    }
                    Item {
                        width:parent.width;height:Math.max(0,accountHeight.value);clip:true
                        Motion{id:accountHeight;target:root.accountOpen===accountCard.item.id?accountFolders.implicitHeight:0;enabled:Theme.motion}
                        onHeightChanged:Qt.callLater(root.refreshPill)
                        Column {id:accountFolders;width:parent.width
                            Repeater{model:["inbox","all_mail","unread","starred","sent","drafts","archive","spam","trash"];delegate:NavigationEntry{required property string modelData;navigation:root;folder:modelData;scope:accountCard.item.id;disclosed:root.accountOpen===accountCard.item.id}}
                            LabelSection{account:accountCard.item.id;navigation:root;disclosed:root.accountOpen===accountCard.item.id}
                        }
                    }
                }
            }
        }
    }
    Item {
        id:footer;x:Theme.unit(16);width:parent.width-2*x;height:footerDepth.value;anchors.bottom:parent.bottom
        Motion{id:footerDepth;target:Theme.unit(root.collapsed?110:72);enabled:Theme.motion}
        Motion{id:settingsY;target:Theme.unit(root.collapsed?12:20);enabled:Theme.motion}
        Motion{id:collapseY;target:Theme.unit(root.collapsed?60:20);enabled:Theme.motion}
        Rectangle{height:1;width:parent.width;color:Theme.outline;opacity:.55}
        FluidButton {objectName:"sidebarSettings";x:root.collapsed?(footer.width-width)/2:0;y:settingsY.value;width:root.collapsed?Theme.unit(34):Math.max(Theme.unit(34),footer.width-Theme.unit(42));labelWidth:Math.max(0,width-Theme.unit(61));glyph:"settings";text:root.collapsed?"":"Settings";tip:"Settings";onClicked:root.settingsRequested()}
        FluidButton {objectName:"sidebarCollapse";anchors.right:parent.right;anchors.rightMargin:root.collapsed?(footer.width-width)/2:0;y:collapseY.value;glyph:root.collapsed?"chevron-right":"chevron-left";tip:root.collapsed?"Expand navigation":"Collapse navigation";onClicked:root.collapsed=!root.collapsed}
    }
}
