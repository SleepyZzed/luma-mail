import QtQuick
import QtQuick.Controls.Basic
Item {
    id:root;objectName:"composer"
    property bool shown:false
    property bool closingPending:false
    signal comparisonRequested(string session,var form)
    property bool expanded:false
    property bool inlineMode:false
    property string phase:"editing"
    property Item scene
    property Item homeScene
    property var popout:null
    readonly property bool detached:popout!==null
    property bool previousExpanded:false
    property bool previousInline:false
    readonly property bool hostVisible:!!root.Window.window?.visible&&root.Window.window?.visibility!==Window.Minimized
    property rect origin:Qt.rect(22,99,200,47)
    property Item inlineDock
    property bool manualPosition:false
    property point floatingPosition:Qt.point(0,0)
    property point dragStart:Qt.point(0,0)
    readonly property var to:formLoader.item?.toField||null
    readonly property var cc:formLoader.item?.ccField||null
    readonly property var bcc:formLoader.item?.bccField||null
    readonly property var subject:formLoader.item?.subjectField||null
    readonly property var body:formLoader.item?.bodyField||null
    readonly property var rich:formLoader.item?.richDocument||null
    readonly property var closeDialog:formLoader.item?.closePopup||null
    readonly property var linkDialog:formLoader.item?.linkPopup||null
    readonly property var tableDialog:formLoader.item?.tablePopup||null
    readonly property var findDialog:formLoader.item?.findPopup||null
    readonly property var historyDialog:formLoader.item?.historyPopup||null
    readonly property var formatMenu:formLoader.item?.formatPopup||null
    readonly property var drag:formLoader.item?.headerDrag||null
    readonly property bool controlsLoaded:formLoader.status===Loader.Ready
    property bool formLoading:false
    property string loadedSession:""
    property bool showCc:false
    property bool showBcc:false
    property var openingSurface:undefined
    property bool includeQuote:true
    property bool includeSignature:true
    property string localError:""
    readonly property var state:Mail.composer
    readonly property bool locked:!!state.locked||!!state.sourceMode
    readonly property bool attachmentEditable:state.state==="editing"||state.state==="uploading"
    readonly property bool inputBusy:!!state.inputBusy||Mail.clipboardBusy
    readonly property bool busy:inputBusy||!!state.identityBusy||state.state==="saving"||state.state==="uploading"||state.state==="sending"||!!state.pending
    readonly property var senderChoices:{
        let choices=[]
        for(const account of Mail.settings.accounts||[]){
            if(!account.enabled||(state.accountLocked&&account.id!==state.account))continue
            if(account.id===state.account){for(const identity of state.identities||[])choices.push({header:identity.header,address:identity.email,account:account.id})}
            else choices.push({header:account.email,address:account.email,account:account.id})
        }
        return choices
    }
    readonly property real compactWidth:Math.min(Theme.unit(650),scene.width-Theme.unit(48))
    readonly property real compactHeight:Math.min(Theme.unit(535),scene.height-Theme.unit(48))
    readonly property real inlineHeight:Theme.unit(480)
    readonly property real editorWidth:detached?scene.width:inlineMode&&parent?parent.width:expanded?scene.width-Theme.unit(40):compactWidth
    readonly property real editorHeight:detached?scene.height:inlineMode?inlineHeight:expanded?scene.height-Theme.unit(40):compactHeight
    readonly property bool statusMode:phase==="sent"||phase==="sending"
    readonly property real goalWidth:statusMode?Math.min(editorWidth,Theme.unit(330)):editorWidth
    readonly property real goalHeight:inlineMode&&!shown?0:statusMode?Theme.unit(54):editorHeight
    readonly property real goalX:detached&&!statusMode?0:inlineMode?0:statusMode?scene.width-goalWidth-Theme.unit(24):expanded?Theme.unit(20):manualPosition?Math.max(Theme.unit(12),Math.min(scene.width-goalWidth-Theme.unit(12),floatingPosition.x)):scene.width-goalWidth-Theme.unit(24)
    readonly property real goalY:detached&&shown&&!statusMode?0:inlineMode?0:!shown?scene.height+Theme.unit(24):statusMode?scene.height-goalHeight-Theme.unit(24):expanded?Theme.unit(20):manualPosition?Math.max(Theme.unit(12),Math.min(scene.height-goalHeight-Theme.unit(12),floatingPosition.y)):scene.height-goalHeight-Theme.unit(24)
    readonly property bool moving:horizontal.running||vertical.running||breadth.running||depth.running
    onMovingChanged:if(closingPending&&!shown&&!moving)Qt.callLater(()=>releaseDelay.restart())
    readonly property bool recipientSuggestionsOpen:!!to?.suggestionsOpen||!!cc?.suggestionsOpen||!!bcc?.suggestionsOpen
    visible:shown||moving;clip:true;z:70
    property bool externalModal:false
    readonly property bool modalOpen:externalModal||AttachmentViewer.opened||!!body?.editMenu?.visible||!!to?.editMenu?.visible||!!cc?.editMenu?.visible||!!bcc?.editMenu?.visible||!!subject?.editMenu?.visible||!!closeDialog?.visible||!!linkDialog?.visible||!!tableDialog?.visible||!!findDialog?.visible||!!historyDialog?.visible||!!formatMenu?.visible
    x:horizontal.value;y:vertical.value;width:Math.max(0,breadth.value);height:Math.max(0,depth.value)
    Motion{id:horizontal;target:root.goalX;enabled:Theme.motion&&root.hostVisible&&!drag?.active}
    Motion{id:vertical;target:root.goalY;enabled:Theme.motion&&root.hostVisible&&!drag?.active}
    Motion{id:breadth;target:root.goalWidth;enabled:Theme.motion&&root.hostVisible}
    Motion{id:depth;target:root.goalHeight;enabled:Theme.motion&&root.hostVisible}
    Rectangle{objectName:"composeSurface";anchors.fill:parent;radius:corners.value;color:Theme.raised;border.width:1;border.color:Theme.outline;Motion{id:corners;target:Theme.unit(root.statusMode?27:root.detached?0:root.inlineMode?18:22);enabled:Theme.motion&&root.hostVisible}}
    DropArea {
        id:fileDrop;objectName:"composeFileDrop";anchors.fill:parent;z:30
        enabled:root.shown&&root.attachmentEditable&&!root.inputBusy&&!root.modalOpen&&!root.statusMode
        onEntered:drag=>{if(!drag.hasUrls||drag.urls.length<1||drag.urls.length>64||!(drag.supportedActions&Qt.CopyAction))drag.accepted=false;else drag.accept(Qt.CopyAction)}
        onDropped:drop=>{if((drop.supportedActions&Qt.CopyAction)&&Mail.attachUrls(drop.urls))drop.accept(Qt.CopyAction);else drop.accepted=false}
    }
    Rectangle {
        anchors.fill:parent;anchors.margins:Theme.unit(5);z:31;radius:Theme.unit(18);color:Theme.selected;border.width:2;border.color:Theme.accent
        visible:fileDrop.containsDrag||dropPresence.running;opacity:Math.max(0,Math.min(1,dropPresence.value))
        Motion{id:dropPresence;target:fileDrop.containsDrag?.95:0;epsilon:.002;enabled:Theme.motion&&root.hostVisible}
        Text{textFormat:Text.PlainText;anchors.centerIn:parent;text:"Drop files to attach";color:Theme.text;font.pixelSize:Theme.unit(20);font.weight:Font.DemiBold}
    }
    component FormatButton:FluidButton {enabled:!root.locked}
    component Field:FluidField {
        Accessible.name:placeholderText
        color:Theme.text;placeholderTextColor:Theme.faint;selectByMouse:true;font.pixelSize:Theme.unit(12)
        leftPadding:0;rightPadding:0;topPadding:Theme.unit(4);bottomPadding:Theme.unit(4);background:Item{}
        enabled:!root.state.locked;maximumLength:8192;onTextEdited:root.schedule()
    }
    Loader {
        id:formLoader;objectName:"composeControlsLoader";anchors.fill:parent;active:false
        sourceComponent:Component {Item {
            property alias toField:to
            property alias ccField:cc
            property alias bccField:bcc
            property alias subjectField:subject
            property alias bodyField:body
            property alias richDocument:rich
            property alias closePopup:closeDialog
            property alias linkPopup:linkDialog
            property alias tablePopup:tableDialog
            property alias findPopup:findDialog
            property alias historyPopup:historyDialog
            property alias formatPopup:formatMenu
            property alias headerDrag:drag
    Item {
        id:header;width:parent.width;height:Theme.unit(52)
        Text{textFormat:Text.PlainText;x:Theme.unit(22);anchors.verticalCenter:parent.verticalCenter;text:root.phase==="sending"?"Sending · waiting for Gmail…":root.phase==="sent"?"Sent · confirmed by Gmail":root.state.mode==="draft"?"Gmail draft":root.inlineMode?"Reply":"New message";color:Theme.text;font.pixelSize:Theme.unit(13);font.weight:Font.DemiBold}
        Row{anchors.right:parent.right;anchors.rightMargin:Theme.unit(12);anchors.verticalCenter:parent.verticalCenter;visible:!root.statusMode
            FluidButton{glyph:"popout";tip:root.detached?"Dock composer":"Pop out composer";onClicked:root.toggleWindow()}
            FluidButton{visible:!root.detached;glyph:root.expanded?"minimize":"maximize";tip:root.expanded?"Restore composer":"Expand composer";onClicked:root.toggleExpanded()}
            FluidButton{glyph:"close";tip:"Close composer";onClicked:root.requestClose()}
        }
        DragHandler{id:drag;target:null;enabled:!root.statusMode&&(root.detached||(!root.inlineMode&&!root.expanded));acceptedButtons:Qt.LeftButton
            onActiveChanged:if(active){if(root.detached){root.popout.startSystemMove();return}root.dragStart=Qt.point(root.x,root.y);root.floatingPosition=root.dragStart;root.manualPosition=true}
            onTranslationChanged:if(active&&!root.detached)root.floatingPosition=Qt.point(root.dragStart.x+translation.x,root.dragStart.y+translation.y)
        }
    }
    Column {
        id:fields;x:Theme.unit(22);y:header.height;width:parent.width-2*x;spacing:0;visible:!root.statusMode;z:1
        Row{width:parent.width;height:Theme.unit(28)
            Text{textFormat:Text.PlainText;width:Theme.unit(38);anchors.verticalCenter:parent.verticalCenter;text:"From";color:Theme.faint;font.pixelSize:Theme.unit(11)}
            FluidCombo{objectName:"composeFrom";width:parent.width-Theme.unit(38);height:parent.height;model:root.senderChoices;textRole:"header";enabled:!root.busy&&!root.state.locked&&count>1
                onActivated:root.chooseSender(model[currentIndex])
                contentItem:Text{textFormat:Text.PlainText;text:root.state.from||"";color:Theme.muted;verticalAlignment:Text.AlignVCenter;font.pixelSize:Theme.unit(11);elide:Text.ElideRight}
                background:Item{}
            }
        }
        Row{width:parent.width;height:Theme.unit(35);z:3
            Text{textFormat:Text.PlainText;width:Theme.unit(38);anchors.verticalCenter:parent.verticalCenter;text:"To";font.pixelSize:Theme.unit(11);color:Theme.faint}
            RecipientField{id:to;objectName:"composeTo";width:parent.width-Theme.unit(118);height:parent.height;placeholderText:"Recipients";KeyNavigation.tab:cc.visible?cc:bcc.visible?bcc:subject;session:root.loadedSession;account:root.state.account||"";enabled:!root.state.locked;onEdited:root.schedule();z:3}
            FluidButton{text:"Cc";onClicked:{if(root.showCc&&!cc.text.trim())root.showCc=false;else{root.showCc=true;cc.forceActiveFocus()}}}
            FluidButton{text:"Bcc";onClicked:{if(root.showBcc&&!bcc.text.trim())root.showBcc=false;else{root.showBcc=true;bcc.forceActiveFocus()}}}
        }
        Row{width:parent.width;height:root.showCc?Theme.unit(34):0;visible:root.showCc;z:2
            Text{textFormat:Text.PlainText;width:Theme.unit(38);anchors.verticalCenter:parent.verticalCenter;text:"Cc";font.pixelSize:Theme.unit(11);color:Theme.faint}
            RecipientField{id:cc;objectName:"composeCc";width:parent.width-Theme.unit(38);height:parent.height;placeholderText:"Cc recipients";KeyNavigation.tab:bcc.visible?bcc:subject;KeyNavigation.backtab:to;session:root.loadedSession;account:root.state.account||"";enabled:!root.state.locked;onEdited:root.schedule()}
        }
        Row{width:parent.width;height:root.showBcc?Theme.unit(34):0;visible:root.showBcc;z:1
            Text{textFormat:Text.PlainText;width:Theme.unit(38);anchors.verticalCenter:parent.verticalCenter;text:"Bcc";font.pixelSize:Theme.unit(11);color:Theme.faint}
            RecipientField{id:bcc;objectName:"composeBcc";width:parent.width-Theme.unit(38);height:parent.height;placeholderText:"Bcc recipients";KeyNavigation.tab:subject;KeyNavigation.backtab:cc.visible?cc:to;session:root.loadedSession;account:root.state.account||"";enabled:!root.state.locked;onEdited:root.schedule()}
        }
        Rectangle{height:1;width:parent.width;color:Theme.outline}
        Field{id:subject;objectName:"composeSubject";width:parent.width;height:Theme.unit(38);placeholderText:"Subject";font.weight:Font.DemiBold;KeyNavigation.tab:body;KeyNavigation.backtab:bcc.visible?bcc:cc.visible?cc:to}
        Rectangle{height:1;width:parent.width;color:Theme.outline}
        Flickable{width:parent.width;height:Theme.unit(38);contentWidth:tools.width;clip:true;boundsBehavior:Flickable.StopAtBounds;enabled:!root.state.locked
            Row{id:tools;spacing:Theme.unit(1);height:parent.height
                FormatButton{objectName:"composeBold";glyph:"bold";tip:"Bold · Ctrl+B";tint:rich.bold?Theme.accent:Theme.text;onClicked:rich.toggle("bold")}
                FormatButton{glyph:"italic";tip:"Italic · Ctrl+I";tint:rich.italic?Theme.accent:Theme.text;onClicked:rich.toggle("italic")}
                FormatButton{glyph:"underline";tip:"Underline · Ctrl+U";tint:rich.underline?Theme.accent:Theme.text;onClicked:rich.toggle("underline")}
                FormatButton{text:"•";tip:"Bulleted paragraphs";onClicked:rich.list(false)}
                FormatButton{text:"1.";tip:"Numbered paragraphs";onClicked:rich.list(true)}
                FormatButton{glyph:"link";tip:"Insert hyperlink · Ctrl+K";onClicked:linkDialog.open()}
                FormatButton{objectName:"composeTable";glyph:"table";tip:"Insert table";onClicked:tableDialog.open()}
                FluidButton{objectName:"composeAttachFiles";glyph:"attachment";tip:"Attach files";enabled:root.attachmentEditable&&!root.inputBusy;onClicked:Mail.attachFiles()}
                FormatButton{text:"A";tip:"Font colour";onClicked:Mail.chooseColour(false)}
                FormatButton{text:"▰";tip:"Text highlight";onClicked:Mail.chooseColour(true)}
                FormatButton{text:"···";tip:"More formatting";onClicked:formatMenu.open()}
                FluidCombo{enabled:!root.locked;width:Theme.unit(126);model:rich.fonts;displayText:rich.fontFamily;onActivated:rich.family(currentText)}
                FluidCombo{enabled:!root.locked;width:Theme.unit(65);model:[8,9,10,11,12,14,16,18,24,32,48,72];currentIndex:3;displayText:Math.round(rich.fontPointSize)+" pt";onActivated:rich.pointSize(Number(currentText))}
            }
            ScrollBar.horizontal:ScrollBar{implicitHeight:3}
        }
        Row{width:parent.width;height:root.state.sourceMode?Theme.unit(36):0;visible:height>0
            Text{textFormat:Text.PlainText;width:parent.width-Theme.unit(135);anchors.verticalCenter:parent.verticalCenter;text:"Original HTML is preserved. Convert to edit.";color:Theme.muted;font.pixelSize:Theme.unit(10);elide:Text.ElideRight}
            FluidButton{text:"Convert to edit";onClicked:Mail.send("composeConvert",{session:root.loadedSession,text:rich.html()})}
        }
        Flickable{width:parent.width;height:root.state.attachments?.length?Theme.unit(74):0;contentWidth:attachmentCards.width;clip:true
            Row{id:attachmentCards;spacing:Theme.unit(8)
                Repeater{model:root.state.attachments||[];delegate:AttachmentTile{required property var modelData;attachment:modelData;owner:({session:root.loadedSession,generation:root.state.attachmentGeneration});removable:root.attachmentEditable;onRemoveRequested:Mail.send("composeRemoveAttachment",{session:root.loadedSession,attachment:modelData.id,generation:root.state.attachmentGeneration})}}
            }
            ScrollBar.horizontal:ScrollBar{implicitHeight:3}
        }
    }
    Flickable{
        id:bodyScroll;x:Theme.unit(22);y:fields.y+fields.height;width:parent.width-2*x;height:Math.max(0,historyToggles.y-y-Theme.unit(7));clip:true;contentWidth:width;contentHeight:body.height;visible:!root.statusMode
        FluidTextArea{
            id:body;objectName:"composeBody";width:bodyScroll.width;height:Math.max(bodyScroll.height,contentHeight+Theme.unit(12));padding:0;color:Theme.text
            richDocument:rich;pasteAction:()=>{if(!Mail.interceptImagePaste())body.paste()}
            placeholderText:"A little space for your thoughts…";placeholderTextColor:Theme.faint;textFormat:TextEdit.RichText;wrapMode:TextEdit.Wrap;selectByMouse:true;persistentSelection:true;selectionColor:Theme.selected;selectedTextColor:Theme.text;background:Item{} readOnly:root.locked
            onSelectionStartChanged:rich.selection(selectionStart,selectionEnd,cursorPosition)
            onSelectionEndChanged:rich.selection(selectionStart,selectionEnd,cursorPosition)
            onCursorPositionChanged:{rich.selection(selectionStart,selectionEnd,cursorPosition);if(cursorRectangle.y<bodyScroll.contentY)bodyScroll.contentY=cursorRectangle.y;else if(cursorRectangle.y+cursorRectangle.height>bodyScroll.contentY+bodyScroll.height)bodyScroll.contentY=Math.max(0,cursorRectangle.y+cursorRectangle.height-bodyScroll.height)}
            Keys.onPressed:(event)=>{
                const ctrl=event.modifiers&Qt.ControlModifier
                if((ctrl&&event.key===Qt.Key_V)||(event.key===Qt.Key_Insert&&(event.modifiers&Qt.ShiftModifier))){event.accepted=Mail.interceptImagePaste()}
                else if(ctrl&&event.key===Qt.Key_B){rich.toggle("bold");event.accepted=true}
                else if(ctrl&&event.key===Qt.Key_I){rich.toggle("italic");event.accepted=true}
                else if(ctrl&&event.key===Qt.Key_U){rich.toggle("underline");event.accepted=true}
                else if(ctrl&&event.key===Qt.Key_Z){if(event.modifiers&Qt.ShiftModifier)rich.redo();else rich.undo();event.accepted=true}
                else if(ctrl&&event.key===Qt.Key_Y){rich.redo();event.accepted=true}
                else if(ctrl&&event.key===Qt.Key_K){linkDialog.open();event.accepted=true}
                else if(ctrl&&(event.key===Qt.Key_F||event.key===Qt.Key_H)){findDialog.open();event.accepted=true}
                else if(ctrl&&(event.key===Qt.Key_Return||event.key===Qt.Key_Enter)){root.submit("composeSend");event.accepted=true}
                else if(event.key===Qt.Key_Tab||event.key===Qt.Key_Backtab){
                    const back=event.key===Qt.Key_Backtab||!!(event.modifiers&Qt.ShiftModifier)
                    if(rich.inList()){rich.indent(back?-1:1);event.accepted=true}
                    else if(rich.tableTab(back))event.accepted=true
                    else if(back){subject.forceActiveFocus();event.accepted=true}
                }
                else if((event.key===Qt.Key_Return||event.key===Qt.Key_Enter)&&!(event.modifiers&Qt.ShiftModifier)){event.accepted=rich.paragraphBreak()}
            }
            TapHandler{acceptedModifiers:Qt.ControlModifier;onTapped:(event)=>{const link=body.linkAt(event.position.x,event.position.y);if(link)Mail.openLink(link)}}
        }
        ScrollBar.vertical:ScrollBar{implicitWidth:5}
    }
    RichDocument{id:rich;objectName:"composeDocument";document:body.textDocument;uiScale:Theme.scale;locked:root.locked;onEdited:root.schedule();onPlaceCursor:(start,end)=>{body.select(start,end);if(root.shown)body.forceActiveFocus()};onError:(message)=>root.localError=message}
    Row{
        id:historyToggles;x:Theme.unit(18);y:footer.y-height;width:parent.width-2*x;height:root.state.quote||root.state.signatureHtml?Theme.unit(30):0;visible:!root.statusMode
        FluidCheck{text:"Include signature";visible:!!root.state.signatureHtml;checked:root.includeSignature;enabled:!root.locked;onToggled:{root.includeSignature=checked;root.schedule()}}
        FluidCheck{text:"Include quoted history";visible:!!root.state.quote;checked:root.includeQuote;enabled:!root.locked;onToggled:{root.includeQuote=checked;root.schedule()}}
        FluidButton{text:"View history";visible:!!root.state.quote;onClicked:historyDialog.open()}
    }
    Item{
        id:footer;x:Theme.unit(18);y:parent.height-Theme.unit(65);width:parent.width-2*x;height:Theme.unit(55);visible:!root.statusMode
        ProgressBar{width:parent.width;height:Theme.unit(3);visible:root.busy;from:0;to:Math.max(1,root.state.progress?.total||0);value:root.state.progress?.done||0;indeterminate:root.busy&&!(root.state.progress?.total>0)}
        Row{y:Theme.unit(12);spacing:Theme.unit(5)
            FluidButton{text:root.state.pending?"Send queued":"Send";primary:true;glyph:"send";enabled:!!root.loadedSession&&!root.state.locked&&root.state.state!=="conflict"&&!root.state.pending&&root.state.senderVerified!==false;tip:"Send · Ctrl+Enter";onClicked:root.submit("composeSend")}
            FluidButton{glyph:"save";tip:"Save to Gmail drafts";enabled:!!root.loadedSession&&!root.busy&&!root.state.locked&&root.state.state!=="conflict";onClicked:root.submit("composeSave")}
            FluidButton{text:root.inputBusy?"Cancel image":"Cancel upload";visible:root.inputBusy||root.state.state==="uploading";onClicked:root.inputBusy?Mail.cancelClipboard():Mail.send("composeCancelUpload",{session:root.loadedSession})}
            FluidButton{text:"Save separate copy";visible:root.state.state==="conflict";onClicked:{root.sync();Mail.send("composeSaveCopy",{session:root.loadedSession})}}
            FluidButton{text:"Compare versions";visible:root.state.state==="conflict";onClicked:{inputDelay.stop();root.comparisonRequested(root.loadedSession,root.form())}}
        }
        Text{textFormat:Text.PlainText;anchors.right:parent.right;y:Theme.unit(24);width:Math.max(0,parent.width-Theme.unit(240));visible:root.state.state!=="conflict";text:root.localError||(root.inputBusy?"Preparing clipboard image…":root.state.status)||"Changes stay in memory until saved";color:Theme.muted;elide:Text.ElideRight;font.pixelSize:Theme.unit(9);horizontalAlignment:Text.AlignRight}
    }
    FluidMenu{id:formatMenu
        FluidMenuItem{text:"Strikethrough";onTriggered:rich.toggle("strike")}
        FluidMenuItem{text:"Superscript";onTriggered:rich.toggle("super")}
        FluidMenuItem{text:"Subscript";onTriggered:rich.toggle("sub")}
        FluidMenuItem{text:"Code font";onTriggered:rich.toggle("code")}
        FluidMenu{title:"Line spacing"
            FluidMenuItem{text:"Single";onTriggered:rich.lineSpacing(1)}
            FluidMenuItem{text:"1.15";onTriggered:rich.lineSpacing(1.15)}
            FluidMenuItem{text:"1.5";onTriggered:rich.lineSpacing(1.5)}
            FluidMenuItem{text:"Double";onTriggered:rich.lineSpacing(2)}
        }
        FluidMenuSeparator{}
        FluidMenuItem{text:"Align left";onTriggered:rich.align(Qt.AlignLeft)}
        FluidMenuItem{text:"Align centre";onTriggered:rich.align(Qt.AlignCenter)}
        FluidMenuItem{text:"Align right";onTriggered:rich.align(Qt.AlignRight)}
        FluidMenuItem{text:"Justify";onTriggered:rich.align(Qt.AlignJustify)}
        FluidMenuItem{text:"Indent";onTriggered:rich.indent(1)}
        FluidMenuItem{text:"Outdent";onTriggered:rich.indent(-1)}
        FluidMenuItem{text:"Clear formatting";onTriggered:rich.clearFormat()}
        FluidMenuSeparator{}
        FluidMenuItem{text:"Undo";enabled:rich.canUndo;onTriggered:rich.undo()}
        FluidMenuItem{text:"Redo";enabled:rich.canRedo;onTriggered:rich.redo()}
        FluidMenuItem{text:"Find and replace";onTriggered:findDialog.open()}
    }
    FluidDialog{id:linkDialog;title:"Insert hyperlink";modal:true;anchors.centerIn:Overlay.overlay;width:Math.min(420,root.scene.width-40);standardButtons:Dialog.Ok|Dialog.Cancel
        Column{width:parent.width;spacing:12
            FluidField{id:linkLabel;width:parent.width;placeholderText:"Text to display"}
            FluidField{id:linkUrl;width:parent.width;placeholderText:"https://…"}
        }
        onOpened:{linkLabel.text=body.selectedText;linkUrl.text="";linkUrl.forceActiveFocus()}
        onAccepted:rich.link(linkUrl.text,linkLabel.text)
    }
    FluidDialog{id:tableDialog;objectName:"composeTableDialog";title:"Insert table";modal:true;anchors.centerIn:Overlay.overlay;width:Math.min(360,root.scene.width-40);standardButtons:Dialog.Ok|Dialog.Cancel
        Column{width:parent.width;spacing:Theme.unit(10)
            Text{textFormat:Text.PlainText;width:parent.width;wrapMode:Text.Wrap;text:"Choose up to 10 rows and columns. Tab moves between cells; Tab in the last cell adds a row.";color:Theme.muted;font.pixelSize:Theme.unit(11)}
            Row{width:parent.width;spacing:Theme.unit(12)
                Column{width:(parent.width-parent.spacing)/2;spacing:Theme.unit(4);Text{text:"Rows";color:Theme.muted;font.pixelSize:Theme.unit(11)}FluidField{id:tableRows;width:parent.width;text:"2";placeholderText:"Rows";validator:IntValidator{bottom:1;top:10}}}
                Column{width:(parent.width-parent.spacing)/2;spacing:Theme.unit(4);Text{text:"Columns";color:Theme.muted;font.pixelSize:Theme.unit(11)}FluidField{id:tableColumns;width:parent.width;text:"2";placeholderText:"Columns";validator:IntValidator{bottom:1;top:10}}}
            }
        }
        onOpened:{tableRows.selectAll();tableRows.forceActiveFocus()}
        onAccepted:if(!root.locked&&!rich.table(Number(tableRows.text),Number(tableColumns.text)))root.localError="Choose between 1 and 10 rows and columns."
    }
    FluidDialog{id:findDialog;title:"Find and replace";modal:true;anchors.centerIn:Overlay.overlay;width:Math.min(430,root.scene.width-40);standardButtons:Dialog.Close
        Column{width:parent.width;spacing:8
            FluidField{id:findText;width:parent.width;placeholderText:"Find"}
            FluidField{id:replaceText;width:parent.width;placeholderText:"Replace with"}
            Row{FluidButton{text:"Previous";onClicked:rich.find(findText.text,true)}FluidButton{text:"Next";onClicked:rich.find(findText.text)}}
            Row{FluidButton{text:"Replace";onClicked:rich.replace(findText.text,replaceText.text)}FluidButton{text:"Replace all";onClicked:rich.replace(findText.text,replaceText.text,true)}}
        }
        onOpened:findText.forceActiveFocus()
    }
    FluidDialog{id:historyDialog;title:"Quoted history · read only";modal:true;anchors.centerIn:Overlay.overlay;width:Math.min(600,root.scene.width-50);height:Math.min(400,root.scene.height-70);standardButtons:Dialog.Close
        ScrollView{anchors.fill:parent;FluidTextArea{text:root.state.quote||"";readOnly:true;selectByMouse:true;wrapMode:TextEdit.Wrap;textFormat:TextEdit.PlainText}}
    }
    FluidDialog{id:closeDialog;property var question:({});title:question.uncertain?"Gmail confirmation is uncertain":question.conflict?"This draft changed in Gmail":"Save this draft to Gmail?";modal:true;closePolicy:Popup.NoAutoClose;anchors.centerIn:Overlay.overlay;width:Math.min(540,root.scene.width-40)
        Column{width:parent.width;spacing:12
            Text{textFormat:Text.PlainText;width:parent.width;wrapMode:Text.Wrap;text:closeDialog.question.uncertain?"Check Sent and Drafts in Gmail before trying again. Closing keeps the existing Gmail draft and clears this in-memory editor.":closeDialog.question.conflict?"The Gmail version is preserved. Keep editing to save a separate copy, or close without saving your changes.":"Your changes are in memory. Any draft already saved in Gmail will be kept.";color:Theme.text}
            Flow{width:parent.width;spacing:6
                FluidButton{text:"Keep editing";onClicked:root.answerClose("keep")}
                FluidButton{text:"Close without saving";onClicked:root.answerClose("close")}
                FluidButton{text:"Save and close";primary:true;visible:!closeDialog.question.uncertain&&!closeDialog.question.conflict;onClicked:root.answerClose("save")}
            }
        }
    }
        }}
    }
    Timer{id:inputDelay;interval:100;onTriggered:root.sync()}
    Shortcut{sequence:"Ctrl+Return";enabled:root.shown&&!root.modalOpen;onActivated:root.submit("composeSend")}
    Shortcut{sequence:"Ctrl+Enter";enabled:root.shown&&!root.modalOpen;onActivated:root.submit("composeSend")}
    Shortcut{sequence:"Ctrl+S";enabled:root.shown&&!root.modalOpen;onActivated:root.submit("composeSave")}
    Timer{id:sentDelay;interval:1100;onTriggered:root.close()}
    Timer{id:releaseDelay;interval:0;onTriggered:root.releaseEditor()}
    Component {
        id:windowComponent
        ApplicationWindow {
            id:secondary;objectName:"composerWindow";property var card:null
            width:Math.max(Theme.unit(500),card?card.width:650);height:Math.max(Theme.unit(500),card?card.height:560)
            minimumWidth:Theme.unit(440);minimumHeight:Theme.unit(420)
            visible:false;title:"Compose - Luma Mail";color:Theme.window
            flags:isWindows?Qt.Window:Qt.Window|Qt.FramelessWindowHint
            transientParent:null
            onVisibilityChanged:if(!root.popout||root.popout===secondary)Mail.setComposerWindowVisible(visible&&visibility!==Window.Minimized)
            onClosing:event=>{event.accepted=false;if(card)card.requestClose()}
            Shortcut{sequence:"Ctrl+W";enabled:!!secondary.card&&!secondary.card.modalOpen;onActivated:secondary.card.requestClose()}
            Shortcut{sequence:"Escape";enabled:!!secondary.card&&!secondary.card.modalOpen&&!secondary.card.recipientSuggestionsOpen;onActivated:secondary.card.requestClose()}
            Shortcut{sequence:"Ctrl+Q";onActivated:Mail.quit()}
            Component.onDestruction:if(!root.popout||root.popout===secondary)Mail.setComposerWindowVisible(false)
        }
    }
    function toggleWindow(){
        if(detached){returnHome(true);return}
        if(!shown||statusMode||modalOpen)return
        inputDelay.stop();sync();linkDialog.close();tableDialog.close();findDialog.close();historyDialog.close();formatMenu.close()
        const at=mapToGlobal(0,0),oldWidth=width,oldHeight=height,oldRadius=corners.value
        previousExpanded=expanded;previousInline=inlineMode
        const inset=Theme.unit(20)
        const host=windowComponent.createObject(root,{card:root,width:Math.max(Theme.unit(500),oldWidth+2*inset),height:Math.max(Theme.unit(500),oldHeight+2*inset),x:at.x-inset,y:at.y-inset})
        if(!host)return
        Mail.trackWindow(host);popout=host;inlineMode=false;expanded=true;scene=host.contentItem;parent=scene
        // Show first so the springs are enabled in their new exposed window.
        // Rebase the same editor at its original global bounds, then let its
        // bounds/corners expand to the native host. No second editor or snapshot.
        host.show();horizontal.rebase((host.width-oldWidth)/2);vertical.rebase((host.height-oldHeight)/2);breadth.rebase(oldWidth);depth.rebase(oldHeight);corners.rebase(oldRadius)
        host.raise();host.requestActivate();Qt.callLater(()=>{if(root.shown&&body)body.forceActiveFocus()})
    }
    function releasePopout(){
        if(!popout)return
        const old=popout
        parent=homeScene;scene=homeScene;popout=null;inlineMode=false;expanded=false;inlineDock=null
        horizontal.snap(goalX);vertical.snap(goalY);breadth.snap(goalWidth);depth.snap(goalHeight)
        old.card=null;old.hide();old.destroy()
    }
    function returnHome(activate){
        if(!popout)return
        const old=popout,global=mapToGlobal(0,0),oldWidth=width,oldHeight=height
        if(activate)Mail.showMainWindow()
        parent=homeScene;scene=homeScene;popout=null;inlineMode=false;expanded=previousExpanded
        const at=homeScene.mapFromGlobal(global.x,global.y)
        horizontal.rebase(Math.max(0,Math.min(homeScene.width-oldWidth,at.x)));vertical.rebase(Math.max(0,Math.min(homeScene.height-oldHeight,at.y)))
        breadth.rebase(oldWidth);depth.rebase(oldHeight)
        if(previousInline&&inlineDock&&Mail.readerOpen&&Mail.reader.key?.id===state.key?.id&&Mail.reader.key?.account===state.key?.account){dockTo(inlineDock);expanded=false}
        else inlineDock=null
        old.card=null;old.hide();old.destroy();if(activate)Qt.callLater(()=>body.forceActiveFocus())
    }
    function ensureControls(){formLoader.active=true}
    function releaseEditor(){
        if(!closingPending||shown||moving)return
        inputDelay.stop();formLoading=true
        Mail.releaseComposerView(loadedSession);loadedSession="";showCc=false;showBcc=false
        releasePopout();formLoader.active=false;formLoading=false;closingPending=false
    }
    function schedule(){if(!formLoading&&shown&&loadedSession)inputDelay.restart()}
    function chooseSender(choice){root.sync();if(choice.account===state.account)Mail.send("composeIdentity",{session:loadedSession,address:choice.address});else Mail.send("composeAccount",{session:loadedSession,account:choice.account})}
    function form(){if(!controlsLoaded)return {};let value={to:to.text,cc:cc.text,bcc:bcc.text,subject:subject.text,includeQuote:includeQuote,includeSignature:includeSignature};if(!state.sourceMode){const p=rich.payload();value.text=p.text;value.html=p.html}return value}
    function sync(){inputDelay.stop();if(formLoading||!shown||!loadedSession||state.locked)return;let f=form();f.session=loadedSession;Mail.send("composeUpdate",f)}
    function submit(action){if(!loadedSession||state.locked)return;inputDelay.stop();Mail.send(action,{session:loadedSession,form:form()})}
    function requestClose(){if(closeDialog?.visible)return;if(loadedSession){inputDelay.stop();Mail.send("composeClose",{session:loadedSession,form:form()})}else close()}
    function answerClose(answer){Mail.send("composeCloseAnswer",{session:loadedSession,revision:closeDialog.question.revision,answer:answer});closeDialog.close()}
    function focusEditor(){if(detached)Mail.activateWindow(popout);Qt.callLater(()=>{if(!shown||!controlsLoaded)return;if(inlineMode)body.forceActiveFocus();else to.forceActiveFocus()})}
    function open(mode){
        ensureControls()
        const activeSurface=openingSurface===undefined?visible:openingSurface;openingSurface=undefined
        closingPending=false;releaseDelay.stop();sentDelay.stop();phase="editing";expanded=false;shown=true
        if(Theme.motion&&!inlineMode&&!activeSurface){horizontal.snap(origin.x);vertical.snap(origin.y);breadth.snap(origin.width);depth.snap(origin.height);horizontal.resume();vertical.resume();breadth.resume();depth.resume()}
        focusEditor()
    }
    function close(){inputDelay.stop();closingPending=true;shown=false;closeDialog?.close();linkDialog?.close();tableDialog?.close();findDialog?.close();historyDialog?.close();formatMenu?.close();Qt.callLater(()=>{if(!moving)releaseDelay.restart()})}
    function toggleExpanded(){
        const position=mapToItem(scene,0,0)
        if(inlineMode){inlineMode=false;parent=scene;expanded=true;horizontal.rebase(position.x);vertical.rebase(position.y)}
        else if(expanded&&inlineDock){const dock=inlineDock.mapToItem(scene,0,0);parent=inlineDock;inlineMode=true;expanded=false;horizontal.rebase(position.x-dock.x);vertical.rebase(position.y-dock.y)}
        else expanded=!expanded
    }
    function floatToScene(){const at=mapToItem(scene,0,0);parent=scene;inlineMode=false;manualPosition=false;horizontal.rebase(at.x);vertical.rebase(at.y)}
    function dockTo(dock){
        const activeSurface=openingSurface===undefined?visible:openingSurface
        const at=mapToItem(dock,0,0);inlineDock=dock;parent=dock;inlineMode=true
        if(activeSurface){horizontal.rebase(at.x);vertical.rebase(at.y)}
        else{horizontal.snap(0);vertical.snap(0);breadth.snap(dock.width);depth.snap(0);horizontal.resume();vertical.resume();breadth.resume();depth.resume()}
    }
    function dockBottomRight(){manualPosition=false}
    Connections{target:Mail
        function onComposerChanged(){const c=Mail.composer;formLoading=true;if(c.replace&&c.document){ensureControls();loadedSession=c.session;to.text=c.to||"";cc.text=c.cc||"";bcc.text=c.bcc||"";showCc=!!cc.text;showBcc=!!bcc.text;subject.text=c.subject||"";rich.load(c.document);includeQuote=c.includeQuote;includeSignature=c.includeSignature;localError=""}if(c.phase)phase=c.phase;formLoading=false;if(c.state==="sent")sentDelay.restart()}
        function onComposeClosed(){root.close()}
        function onComposeCloseQuestion(data){if(root.detached)Mail.activateWindow(root.popout);closeDialog.question=data;closeDialog.open()}
        function onFlushComposer(){root.sync()}
        function onColourChosen(colour,background){if(!rich)return;if(background)rich.highlight(colour);else rich.foreground(colour)}
    }
}
