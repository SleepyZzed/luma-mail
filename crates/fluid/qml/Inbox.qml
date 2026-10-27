import QtQuick
import QtQuick.Controls
Item {
    id:root;objectName:"inboxPane"
    property Item scene
    property bool compact:Mail.readerOpen
    signal contextRequested(var row,real x,real y)
    clip:true
    Item {
        id:header;x:Theme.unit(28);y:Theme.unit(21);width:parent.width-2*x;height:Theme.unit(116)
        Text {textFormat:Text.PlainText;id:eyebrow;visible:Mail.preview;text:Mail.preview?"ISOLATED DESIGN PREVIEW":"";font.pixelSize:Theme.unit(9);font.letterSpacing:1.4;color:Theme.faint}
        Text {textFormat:Text.PlainText;y:Theme.unit(19);text:Mail.folder==="inbox"?"Your inbox":Theme.folderTitle(Mail.folder);font.pixelSize:Theme.unit(21);font.weight:Font.DemiBold;color:Theme.text}
        Row {anchors.right:parent.right;y:Theme.unit(23);spacing:Theme.unit(5)
            FluidButton {glyph:"refresh";tint:Mail.sync.updatesAvailable?Theme.accent:Theme.muted;tip:Mail.sync.updatesAvailable?"Mailbox changes available · refresh now":"Refresh now · automatic checks run silently";onClicked:{Mail.send("pollNow");Mail.setView(Mail.folder,Mail.account,Mail.search)}}
            FluidButton {glyph:"check";tip:"Select conversations";onClicked:Mail.selectionMode(!Mail.selecting)}
        }
        Rectangle {
            id:searchSurface;y:Theme.unit(61);height:Theme.unit(37);width:parent.width;radius:Theme.unit(13);color:search.activeFocus?Theme.raised:Theme.surface;border.width:1;border.color:search.activeFocus?Theme.accent:Theme.outline
            Icon{x:Theme.unit(13);anchors.verticalCenter:parent.verticalCenter;name:"search";color:Theme.muted}
            TextField {id:search;anchors.fill:parent;leftPadding:Theme.unit(41);rightPadding:Theme.unit(43);placeholderText:"Search your mail";font.pixelSize:Theme.unit(12);color:Theme.text;placeholderTextColor:Theme.faint;selectByMouse:true;background:Item{} text:Mail.search;onAccepted:Mail.setView(Mail.folder,Mail.account,text)}
            FluidButton {anchors.right:parent.right;anchors.rightMargin:5;anchors.verticalCenter:parent.verticalCenter;glyph:"filter";tip:"Search options";onClicked:filters.open()}
        }
        Text {textFormat:Text.PlainText;y:Theme.unit(107);text:Mail.selecting?Mail.selectionCount+" selected":Mail.inbox.count+" conversations · online first";font.pixelSize:Theme.unit(10);color:Theme.faint}
        Row {anchors.right:parent.right;y:Theme.unit(98);visible:Mail.selecting
            FluidButton{text:"Select all";onClicked:Mail.selectAll()}
            FluidButton{glyph:"archive";tip:"Archive selection";enabled:Mail.selectionCount>0;onClicked:Mail.mutate("archive")}
            FluidButton{glyph:"trash";tip:"Move selection to Trash";enabled:Mail.selectionCount>0;onClicked:Mail.mutate("trash")}
            FluidButton{glyph:"read";tip:"Mark selection read";enabled:Mail.selectionCount>0;onClicked:Mail.mutate("read")}
        }
    }
    ListView {
        id:list;objectName:"mailList";x:Theme.unit(12);y:header.y+header.height+Theme.unit(14);width:parent.width-2*x;height:parent.height-y-Theme.unit(62);model:Mail.inbox;clip:true;spacing:Theme.unit(5);cacheBuffer:Theme.unit(350);reuseItems:true;boundsBehavior:Flickable.StopAtBounds
        ScrollBar.vertical:ScrollBar{implicitWidth:5;policy:ScrollBar.AsNeeded}
        add:Transition {enabled:Theme.motion;SpringAnimation{property:"y";spring:4;damping:.45;epsilon:.3}}
        addDisplaced:Transition {enabled:Theme.motion;SpringAnimation{property:"y";spring:4;damping:.45;epsilon:.3}}

        removeDisplaced:Transition {enabled:Theme.motion;SpringAnimation{property:"y";spring:4;damping:.45;epsilon:.3}}
        move:Transition {enabled:Theme.motion;SpringAnimation{property:"y";spring:4;damping:.45;epsilon:.3}}
        moveDisplaced:Transition {enabled:Theme.motion;SpringAnimation{property:"y";spring:4;damping:.45;epsilon:.3}}
        delegate:Item {
            id:row;required property var item;required property int index;objectName:"mailRow-"+item.key.id
            property bool selected: {let n=Mail.selectionCount;return Mail.isSelected(item)}
            property bool open:Mail.readerOpen&&Mail.reader.key&&Mail.reader.key.id===item.key.id&&Mail.reader.key.account===item.key.account
            function box(item) {let p=item.mapToItem(row,0,0);return Qt.rect(p.x,p.y,item.width,item.height)}
            function projection() {return {avatar:box(avatar),sender:box(senderText),subject:box(subjectText),snippet:box(snippetText)}}
            property bool removing:false
            property bool pooled:false
            width:list.width;height:Math.max(0,rowHeight.value);clip:true
            Motion{id:rowHeight;target:row.removing?0:Theme.unit(68);enabled:Theme.motion;onRunningChanged:if(!running&&row.removing)row.ListView.delayRemove=false}
            ListView.onRemove:{ListView.delayRemove=Theme.motion;row.removing=true}
            ListView.onPooled:row.pooled=true
            ListView.onReused:{row.removing=false;row.pooled=false}
            Motion{id:rowHover;target:hover.hovered?1:0;epsilon:.002;enabled:Theme.motion}
            transform:Translate{y:-rowHover.value*Theme.unit(1)}
            Rectangle {anchors.fill:parent;anchors.margins:Theme.unit(2);radius:Theme.unit(17);color:row.selected||row.open?Theme.selected:Theme.hover;opacity:row.selected||row.open?1:Math.max(0,Math.min(1,rowHover.value))*.8
                border.width:row.open?1:0;border.color:Qt.rgba(Theme.accent.r,Theme.accent.g,Theme.accent.b,.22)
            }
            HoverHandler{id:hover}
            Avatar {id:avatar;x:Theme.unit(16);y:Theme.unit(12);sender:row.item.sender;account:row.item.key.account;active:!row.pooled&&list.width>1&&row.y+row.height>list.contentY&&row.y<list.contentY+list.height;width:Theme.unit(32);height:width}
            Rectangle{x:Theme.unit(14);y:Theme.unit(12);width:Theme.unit(5);height:width;radius:width/2;color:Theme.accent;visible:row.item.unread}
            Column {x:Theme.unit(60);y:Theme.unit(10);width:row.width-x-Theme.unit(21);spacing:Theme.unit(3)
                Row {width:parent.width
                    Text {textFormat:Text.PlainText;id:senderText;width:parent.width-Theme.unit(65);text:row.item.sender.replace(/<.*>/g,"").trim();font.pixelSize:Theme.unit(12);font.weight:row.item.unread?Font.DemiBold:Font.Medium;color:Theme.text;elide:Text.ElideRight}
                    Text {textFormat:Text.PlainText;width:Theme.unit(65);horizontalAlignment:Text.AlignRight;text:Theme.date(row.item.date);font.pixelSize:Theme.unit(9);color:row.item.unread?Theme.accent:Theme.faint}
                }
                Text {textFormat:Text.PlainText;id:subjectText;width:parent.width;text:row.item.subject||"(No subject)";font.pixelSize:Theme.unit(12);font.weight:row.item.unread?Font.DemiBold:Font.Normal;color:row.item.unread?Theme.text:Theme.muted;elide:Text.ElideRight}
                Text {textFormat:Text.PlainText;id:snippetText;width:parent.width;text:row.item.snippet;font.pixelSize:Theme.unit(10);color:Theme.faint;elide:Text.ElideRight}
            }
            MouseArea {anchors.fill:parent;acceptedButtons:Qt.LeftButton|Qt.RightButton;onClicked:(mouse)=>{let p=row.mapToItem(root.scene,0,0);if(mouse.button===Qt.RightButton)root.contextRequested(row.item,p.x+mouse.x,p.y+mouse.y);else Mail.selectRow(row.index,mouse.modifiers,p.x,p.y,row.width,row.height,row.projection())}}
            Row {x:row.width-width-Theme.unit(12)+(1-rowHover.value)*Theme.unit(12);y:Theme.unit(29);spacing:Theme.unit(1);opacity:Math.max(0,Math.min(1,rowHover.value));enabled:hover.hovered;visible:!Mail.selecting
                FluidButton {glyph:row.item.starred?"star-filled":"star";tip:row.item.starred?"Remove star":"Star";onClicked:Mail.mutate(row.item.starred?"unstar":"star",row.item)}
                FluidButton {glyph:"archive";tip:"Archive";onClicked:Mail.mutate("archive",row.item)}
                FluidButton {glyph:"trash";tip:"Move to Trash";onClicked:Mail.mutate("trash",row.item)}
            }
        }
    }
    FluidButton {anchors.horizontalCenter:parent.horizontalCenter;anchors.bottom:parent.bottom;anchors.bottomMargin:Theme.unit(15);text:"Load 25 more";glyph:"chevron-down";visible:Mail.more;onClicked:Mail.send("page")}
    function projectionTarget(key, paneWidth) {
        for (let i=0;i<Mail.inbox.count;i++) {
            const item=Mail.inbox.at(i)
            if(item.key.account!==key.account||item.key.id!==key.id)continue
            const row=list.itemAtIndex(i)
            if(!row||row.y<list.contentY||row.y+row.height>list.contentY+list.height)return null
            const p=row.mapToItem(root.scene,0,0)
            return Qt.rect(p.x,p.y,paneWidth-2*list.x,row.height)
        }
        return null
    }
    function openIndex(index, modifiers) {
        const row=list.itemAtIndex(index)
        if(!row)return
        const p=row.mapToItem(root.scene,0,0)
        Mail.selectRow(index,modifiers,p.x,p.y,row.width,row.height,row.projection())
    }
    function revealIndex(index){list.positionViewAtIndex(index,ListView.Contain);list.forceLayout();Qt.callLater(()=>openIndex(index,0))}
    Popup {
        id:filters;x:header.x;y:header.y+Theme.unit(131);width:header.width;padding:Theme.unit(18)
        background:Rectangle{radius:Theme.unit(16);color:Theme.raised;border.width:1;border.color:Theme.outline}
        Column{spacing:Theme.unit(12);width:parent.width
            Text {textFormat:Text.PlainText;text:"Search in Gmail";color:Theme.text;font.pixelSize:Theme.unit(14);font.weight:Font.DemiBold}
            TextField{id:from;placeholderText:"From address";width:parent.width}
            TextField{id:subject;placeholderText:"Subject contains";width:parent.width}
            CheckBox{id:attachment;text:"Has attachments"}
            FluidButton{text:"Search";primary:true;onClicked:{let q=(from.text?"from:"+from.text+" ":"")+(subject.text?"subject:("+subject.text+") ":"")+(attachment.checked?"has:attachment":"");search.text=q;Mail.setView(Mail.folder,Mail.account,q);filters.close()}}
        }
    }
}
