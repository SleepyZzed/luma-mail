import QtQuick
import QtQuick.Controls
Item {
    id:root;objectName:"readerPane"
    property bool projected:false
    property bool nativeBlocked:false
    property var nativeMasks:[]
    property Item inlineComposer
    property real layoutWidth:width
    property real lastLayoutWidth:600
    onLayoutWidthChanged:if(Mail.readerOpen&&layoutWidth>0)lastLayoutWidth=layoutWidth
    function projectionSpec(readerX, paneWidth) {
        const senderY=subject.implicitHeight+Theme.unit(103)
        return {bounds:Qt.rect(readerX+Theme.unit(30),Theme.unit(30),Math.max(0,paneWidth-Theme.unit(60)),subject.implicitHeight+Theme.unit(145)),
            avatar:Qt.rect(0,subject.implicitHeight+Theme.unit(97),Theme.unit(38),Theme.unit(38)),
            sender:Qt.rect(Theme.unit(52),senderY,Theme.unit(200),Theme.unit(18))}
    }
    signal replyAt(string mode,var message)
    clip:true
    Rectangle{width:1;height:parent.height;color:Theme.outline;opacity:.5}
    Flickable {
        id:scroll;x:Theme.unit(30);y:Theme.unit(30);width:Math.max(0,root.layoutWidth-Theme.unit(60));height:parent.height-y-Theme.unit(24)
        clip:true;contentHeight:thread.height;boundsBehavior:Flickable.StopAtBounds
        onContentHeightChanged:if(root.followingReply)Qt.callLater(root.scrollToReply)
        onDraggingChanged:if(dragging)root.followingReply=false
        ScrollBar.vertical:ScrollBar{implicitWidth:5}
        Column {
            id:thread;width:scroll.width;spacing:Theme.unit(26)
            Item {
                width:parent.width;height:subject.implicitHeight+Theme.unit(66);opacity:root.projected?0:1
                Text {textFormat:Text.PlainText;id:subject;objectName:"readerSubject";width:parent.width-Theme.unit(20);text:Mail.reader.subject||"";wrapMode:Text.Wrap;color:Theme.text;font.pixelSize:Theme.unit(25);font.weight:Font.DemiBold;lineHeight:1.2}
                Row {y:subject.height+Theme.unit(17);spacing:Theme.unit(6);visible:Mail.selectionCount<=1
                    FluidButton {glyph:"back";tip:"Back to inbox";onClicked:Mail.closeReader()}
                    FluidButton {glyph:"archive";tip:"Archive conversation";onClicked:Mail.mutate("archive")}
                    FluidButton {glyph:"trash";tip:"Move to Trash";onClicked:Mail.mutate("trash")}
                    FluidButton {glyph:"unread";tip:"Mark unread";onClicked:Mail.mutate("unread")}
                    FluidButton {glyph:"reply";tip:"Reply";onClicked:root.replyAt("reply",Mail.messages.count?Mail.messages.at(Mail.messages.count-1):{})}
                }
            }
            Repeater {
                model:Mail.messages
                delegate:Column {
                    id:message;objectName:"threadMessage-"+item.id;required property var item;required property int index
                    property bool expanded:false
                    Component.onCompleted:expanded=index===Mail.messages.count-1&&!item.arrived
                    property bool originalColours:false
                    property bool hasHtml:item.hasHtml||false
                    function render(external=false){
                        if(!hasHtml||!expanded||!Mail.readerOpen)return
                        function rgb(c){return [Math.round(c.r*255),Math.round(c.g*255),Math.round(c.b*255)]}
                        Mail.renderMessage({account:Mail.reader.key.account,id:item.id,original:originalColours,external:external,background:rgb(Theme.window),foreground:rgb(Theme.text),dark:Theme.dark})
                    }
                    onHasHtmlChanged:if(hasHtml)themeDelay.restart()
                    onOriginalColoursChanged:themeDelay.restart()
                    onExpandedChanged:if(expanded&&hasHtml)themeDelay.restart()
                    Timer{id:themeDelay;interval:180;onTriggered:message.render()}
                    Connections{target:Mail;function onPaletteChanged(){if(message.hasHtml&&message.expanded)themeDelay.restart()}}
                    width:thread.width;spacing:Theme.unit(18)
                    Item {
                        width:parent.width;height:Theme.unit(53);opacity:root.projected&&message.index===0?0:1
                        Avatar {id:sender;objectName:"readerAvatar-"+message.index;sender:message.item.from||"";account:Mail.reader.key?.account||"";active:Mail.readerOpen&&message.y+parent.height>scroll.contentY&&message.y<scroll.contentY+scroll.height;width:Theme.unit(38);height:width;y:Theme.unit(5)}
                        Column {x:Theme.unit(52);anchors.verticalCenter:parent.verticalCenter;width:parent.width-x-Theme.unit(80);spacing:Theme.unit(5)
                            Text {textFormat:Text.PlainText;text:(message.item.from||"").replace(/<.*>/g,"").trim();color:Theme.text;font.pixelSize:Theme.unit(12);font.weight:Font.DemiBold;width:parent.width;elide:Text.ElideRight}
                            Text {textFormat:Text.PlainText;text:"to "+(message.item.to||"");color:Theme.faint;font.pixelSize:Theme.unit(10);width:parent.width;elide:Text.ElideRight}
                        }
                        Text {textFormat:Text.PlainText;anchors.right:parent.right;anchors.top:parent.top;anchors.topMargin:Theme.unit(9);text:Theme.date(message.item.date||0);color:Theme.faint;font.pixelSize:Theme.unit(9)}
                        Rectangle{width:Theme.unit(6);height:width;radius:width/2;color:Theme.accent;visible:message.item.unread||false;anchors.right:parent.right;anchors.bottom:parent.bottom;anchors.bottomMargin:Theme.unit(11)}
                        MouseArea{anchors.fill:parent;onClicked:{message.expanded=!message.expanded;if(message.expanded&&!message.item.text)Mail.send("message",{account:Mail.reader.key.account,id:message.item.id})}}
                    }
                    Item {
                        width:parent.width;height:Math.max(0,expansion.value);clip:true
                        Motion{id:expansion;target:message.expanded?body.implicitHeight:0;enabled:Theme.motion}
                        Column {
                            id:body;width:parent.width;spacing:Theme.unit(20)
                            Motion{id:reveal;target:root.projected||!Mail.readerOpen?0:1;epsilon:.002;enabled:Theme.motion}
                            opacity:Math.max(0,Math.min(1,reveal.value))
                            y:(1-reveal.value)*Theme.unit(18)
                            Rectangle{width:parent.width;height:1;color:Theme.outline;opacity:.6}
                            Row{width:parent.width;spacing:Theme.unit(10);visible:message.hasHtml
                                FluidButton{text:message.originalColours?"Use theme colours":"Original colours";enabled:!message.item.documentPending;onClicked:message.originalColours=!message.originalColours}
                                FluidButton{text:message.item.externalLoaded?"Images requested":"Load images";visible:message.item.externalImages||false;enabled:!message.item.documentPending&&!message.item.externalLoaded;tip:"Contact this message’s image servers. This may tell the sender you opened it.";onClicked:message.render(true)}
                            }
                            Text{textFormat:Text.PlainText;width:parent.width;visible:message.hasHtml&&message.item.externalImages&&!message.item.externalLoaded;text:"External images are blocked for privacy.";wrapMode:Text.Wrap;color:Theme.faint;font.pixelSize:Theme.unit(10)}
                            HtmlSurface{
                                id:htmlHost;width:parent.width;implicitHeight:message.hasHtml?lastHeight:0;height:implicitHeight;visible:message.hasHtml
                                property real lastHeight:Theme.unit(80)
                                property bool inView:viewportHeight>0
                                Loader{
                                    id:htmlLoader;anchors.fill:parent;active:message.hasHtml&&message.expanded&&Mail.readerOpen&&!root.projected&&htmlHost.inView&&(message.item.document||"")!==""
                                    source:isWindows?"WindowsReader.qml":"LinuxReader.qml"
                                    onLoaded:{item.objectName="mailHtml-"+message.item.id;item.page=Qt.binding(()=>message.item.document||"");item.uiScale=Qt.binding(()=>Theme.scale);item.paused=Qt.binding(()=>root.nativeBlocked);item.masks=Qt.binding(()=>root.nativeMasks);htmlHost.lastHeight=item.implicitHeight}
                                }
                                Connections{target:htmlLoader.item;function onImplicitHeightChanged(){htmlHost.lastHeight=htmlLoader.item.implicitHeight}function onWheel(delta){root.followingReply=false;scroll.contentY=Math.max(0,Math.min(scroll.contentHeight-scroll.height,scroll.contentY+delta))}}
                                Text{textFormat:Text.PlainText;anchors.top:parent.top;text:"Preparing private HTML view…";color:Theme.faint;font.pixelSize:Theme.unit(12);visible:!htmlLoader.active}
                            }
                            TextArea {
                                id:plainText;objectName:"readBody-"+message.item.id
                                visible:!message.hasHtml;height:visible?implicitHeight:0
                                width:parent.width;readOnly:true;selectByMouse:true;wrapMode:TextEdit.Wrap;textFormat:TextEdit.PlainText
                                ReadDocument{document:plainText.textDocument;source:message.hasHtml?"":message.item.text||"Opening message…";spans:message.item.links||[];foreground:Theme.text;linkColour:Theme.link;family:Theme.family;pixelSize:Theme.unit(13)}
                                onLinkActivated:link=>Mail.openLink(link)
                                color:Theme.text;selectionColor:Theme.selected;font.family:Theme.family;font.pixelSize:Theme.unit(13);padding:0;background:Item{}
                            }
                            Text{textFormat:Text.PlainText;width:parent.width;visible:(message.item.failedImages||0)>0;text:"Some images could not be previewed within the image size and privacy limits.";wrapMode:Text.Wrap;color:Theme.muted;font.pixelSize:Theme.unit(11)}
                            Row {spacing:Theme.unit(8);visible:Mail.selectionCount<=1
                                FluidButton {text:"Reply";glyph:"reply";quiet:false;onClicked:root.replyAt("reply",message.item)}
                                FluidButton {text:"Reply all";glyph:"reply-all";onClicked:root.replyAt("replyAll",message.item)}
                                FluidButton {text:"Forward";glyph:"forward";onClicked:root.replyAt("forward",message.item)}
                            }
                        }
                    }
                }
            }
            Item{
                id:inlineSlot;width:parent.width
                property bool owns:root.inlineComposer&&root.inlineComposer.inlineDock===inlineSlot
                height:owns?(root.inlineComposer.inlineMode?root.inlineComposer.height:placeholder.value):0
                Motion{id:placeholder;target:inlineSlot.owns&&root.inlineComposer.shown?root.inlineComposer.inlineHeight:0;enabled:Theme.motion}
            }
        }
    }
    property bool followingReply:false
    property real replyScrollGoal:0
    Motion {
        id:replyScroll;target:root.replyScrollGoal;enabled:Theme.motion
        onValueChanged:if(root.followingReply&&!scroll.dragging&&!scroll.flicking)scroll.contentY=Math.max(0,Math.min(scroll.contentHeight-scroll.height,value))
    }
    function scrollToReply(){
        if(!root.followingReply||!root.inlineComposer||!root.inlineComposer.shown)return
        replyScrollGoal=Math.max(0,inlineSlot.y+inlineSlot.height-scroll.height+Theme.unit(20))
    }
    function dockInline(card) {
        card.inlineDock=inlineSlot;card.inlineMode=true;card.parent=inlineSlot
        followingReply=true;replyScroll.snap(scroll.contentY)
        Qt.callLater(()=>{scrollToReply();replyScroll.resume()})
    }
}
