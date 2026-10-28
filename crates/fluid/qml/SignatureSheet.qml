import QtQuick
import QtQuick.Controls.Basic
Item {
    id:root;objectName:"signatureSheet";z:190;visible:present&&(opened||position.running)
    property bool present:false
    property bool opened:false
    property string session:""
    property string account:""
    property string identity:""
    property string savedHtml:""
    property bool editable:false
    property bool visual:false
    property bool dirty:false
    property bool busy:false
    property string operation:""
    property bool loading:false
    property string notice:""
    property var progress:({})
    property string picturePath:""
    property string pictureName:""
    property string pendingPicture:""
    readonly property var editor:editorLoader.item
    readonly property real cardHeight:Math.min(height-Theme.unit(48),Theme.unit(800))
    readonly property bool dialogsOpen:closeQuestion.visible||linkDialog.visible||pictureDialog.visible||publicQuestion.visible||tableDialog.visible||colourDialog.visible||sourceDialog.visible
    function closePopups(){closeQuestion.close();linkDialog.close();pictureDialog.close();publicQuestion.opened=false;tableDialog.close();colourDialog.close();sourceDialog.close()}
    function html(){return visual&&editor?editor.html():savedHtml}
    function changed(){if(loading)return;dirty=true;Mail.send("signatureUpdate",{session:session,dirty:true})}
    function requestClose(){if(session)Mail.send("signatureClose",{session:session,dirty:dirty})}
    function discard(){Mail.send("signatureClose",{session:session,dirty:dirty,discardConfirmed:true});closeQuestion.close()}
    function keepEditing(){Mail.send("signatureKeepEditing",{session:session});closeQuestion.close()}
    function save(close=false){const value=html();if(value.length>65536){notice="Signature HTML exceeds the 64 KiB limit. Shorten it before saving.";return}if(Mail.send("signatureSave",{session:session,html:value,close:close}))busy=true;closeQuestion.close()}
    function finishClose(){if(!opened&&!position.running){present=false;session="";savedHtml="";identity="";picturePath="";pictureName="";pendingPicture="";dirty=false;notice="";progress={};operation="";linkLabel.clear();linkUrl.clear();pictureUrl.clear()}}
    function insertPicture(url){if(editor&&editor.picture(url)){dirty=true;Mail.send("signatureUpdate",{session:session,dirty:true})}}
    Motion{id:position;target:root.opened?(root.height-root.cardHeight)/2:root.height+Theme.unit(24);enabled:Theme.motion;onRunningChanged:Qt.callLater(root.finishClose)}
    Motion{id:shade;target:root.opened?.4:0;enabled:Theme.motion;epsilon:.002}
    Rectangle{anchors.fill:parent;color:"black";opacity:Math.max(0,Math.min(.5,shade.value));MouseArea{anchors.fill:parent;onClicked:if(!root.busy)root.requestClose()}}
    Rectangle{id:card;objectName:"signatureCard";x:(parent.width-width)/2;y:position.value;width:Math.min(parent.width-Theme.unit(48),Theme.unit(940));height:root.cardHeight;radius:Theme.unit(22);color:Theme.surface;border.width:1;border.color:Theme.outline;clip:true
        MouseArea{anchors.fill:parent;acceptedButtons:Qt.AllButtons}
        Text{textFormat:Text.PlainText;x:Theme.unit(24);y:Theme.unit(22);text:"Online signature";color:Theme.text;font.pixelSize:Theme.unit(21);font.weight:Font.DemiBold}
        FluidButton{anchors.right:parent.right;anchors.rightMargin:Theme.unit(17);y:Theme.unit(16);glyph:"close";tip:"Close signature editor";onClicked:root.requestClose();enabled:!root.busy}
        Text{textFormat:Text.PlainText;x:Theme.unit(24);y:Theme.unit(57);width:parent.width-2*x;text:root.identity+" · saved in Gmail";color:Theme.muted;font.pixelSize:Theme.unit(11);elide:Text.ElideMiddle}
        Text{textFormat:Text.PlainText;x:Theme.unit(24);y:Theme.unit(82);width:parent.width-2*x;wrapMode:Text.Wrap;text:"Changes affect this sender’s signature in Gmail web too. Existing HTML stays intact until you choose visual editing. No local signature or image archive is created.";color:Theme.faint;font.pixelSize:Theme.unit(11)}
        Row{id:options;x:Theme.unit(24);y:Theme.unit(129);spacing:Theme.unit(9)
            FluidButton{text:root.visual?"Editing visually":"Edit visually";enabled:root.editable&&!root.visual&&!root.busy;primary:!root.visual;onClicked:{root.visual=true;if(root.editor)root.editor.focusBody()}}
            FluidButton{text:"Load picture previews";glyph:"image";enabled:!root.busy;tip:"Load up to eight public HTTPS pictures anonymously into bounded memory";onClicked:Mail.send("signaturePictures",{session:root.session,html:root.html()})}
            FluidButton{text:"View HTML";onClicked:sourceDialog.open()}
            FluidButton{text:"Cancel pictures";visible:root.busy&&root.operation!=="signatureSave";onClicked:Mail.send("signatureCancelUpload",{session:root.session})}
        }
        Loader{id:editorLoader;objectName:"signatureEditorLoader";active:root.present&&(root.opened||position.running);x:Theme.unit(24);y:options.y+options.height+Theme.unit(13);width:parent.width-2*x;height:Math.max(0,card.height-y-Theme.unit(93));sourceComponent:signatureEditor
            onLoaded:{root.loading=true;item.load(root.savedHtml);root.loading=false}
        }
        ProgressBar{x:Theme.unit(24);y:card.height-Theme.unit(83);width:parent.width-2*x;height:Theme.unit(3);visible:root.busy;indeterminate:!(root.progress.total>0);from:0;to:root.progress.total||1;value:root.progress.done||0}
        Text{textFormat:Text.PlainText;x:Theme.unit(24);y:card.height-Theme.unit(72);width:parent.width-2*x;text:root.notice||(!root.editable?"This sender’s signature is read-only. Choose the primary Gmail address to edit.":root.dirty?"Unsaved changes stay in memory":"Signature is up to date");color:Theme.muted;font.pixelSize:Theme.unit(10);elide:Text.ElideRight}
        Row{x:Theme.unit(24);y:card.height-Theme.unit(45);spacing:Theme.unit(12)
            FluidButton{text:"Close";enabled:!root.busy;onClicked:root.requestClose()}
            FluidButton{objectName:"signatureSave";text:root.busy?"Working…":"Save to Gmail";primary:true;enabled:root.editable&&root.dirty&&!root.busy;onClicked:root.save()}
        }
    }
    Component{id:signatureEditor
        Item {
            id:content
            function load(html){rich.load({kind:"html",text:html})}
            function html(){return rich.html()}
            function picture(url){return rich.picture(url)}
            function preview(url,bytes){return rich.picturePreview(url,bytes)}
            function focusBody(){body.forceActiveFocus()}
            function hyperlink(url,label){return rich.link(url,label)}
            function insertTable(rows,columns){return rich.table(rows,columns)}
            function colour(value,background){if(background)rich.highlight(value);else rich.foreground(value)}
            Flow{id:tools;width:parent.width;spacing:Theme.unit(3);enabled:root.visual&&root.editable&&!root.busy
                FluidButton{glyph:"bold";tip:"Bold · Ctrl+B";tint:rich.bold?Theme.accent:Theme.text;onClicked:rich.toggle("bold")}
                FluidButton{glyph:"italic";tip:"Italic · Ctrl+I";onClicked:rich.toggle("italic")}
                FluidButton{glyph:"underline";tip:"Underline · Ctrl+U";onClicked:rich.toggle("underline")}
                FluidButton{glyph:"strikethrough";tip:"Strikethrough";onClicked:rich.toggle("strike")}
                FluidButton{text:"•";tip:"Bulleted list";onClicked:rich.list(false)}
                FluidButton{text:"1.";tip:"Numbered list";onClicked:rich.list(true)}
                FluidCombo{width:Theme.unit(150);model:rich.fonts;displayText:rich.fontFamily;onActivated:rich.family(currentText)}
                FluidCombo{width:Theme.unit(70);model:[8,9,10,11,12,14,16,18,24,32,48,72];displayText:Math.round(rich.fontPointSize)+" pt";onActivated:rich.pointSize(Number(currentText))}
                FluidButton{text:"A";tip:"Font colour";onClicked:{colourDialog.highlight=false;colourDialog.open()}}
                FluidButton{text:"▰";tip:"Highlight colour";onClicked:{colourDialog.highlight=true;colourDialog.open()}}
                FluidButton{glyph:"link";tip:"Insert hyperlink · Ctrl+K";onClicked:linkDialog.open()}
                FluidButton{glyph:"image";tip:"Insert a hosted picture";onClicked:pictureDialog.open()}
                FluidButton{glyph:"upload-cloud";tip:"Upload a picture copy to Drive; public visibility requires confirmation";onClicked:Mail.chooseSignaturePicture(root.session)}
                FluidButton{text:"Table";onClicked:tableDialog.open()}
                FluidButton{text:"Left";onClicked:rich.align(Qt.AlignLeft)}
                FluidButton{text:"Centre";onClicked:rich.align(Qt.AlignCenter)}
                FluidButton{text:"Right";onClicked:rich.align(Qt.AlignRight)}
                FluidButton{text:"Indent";onClicked:rich.indent(1)}
                FluidButton{text:"Outdent";onClicked:rich.indent(-1)}
                FluidButton{text:"Sup";tip:"Superscript";onClicked:rich.toggle("super")}
                FluidButton{text:"Sub";tip:"Subscript";onClicked:rich.toggle("sub")}
                FluidButton{text:"Code";onClicked:rich.toggle("code")}
                FluidCombo{width:Theme.unit(95);model:["Single","1.15 lines","1.5 lines","Double"];onActivated:rich.lineSpacing([1,1.15,1.5,2][currentIndex])}
                FluidButton{glyph:"clear";tip:"Clear formatting";onClicked:rich.clearFormat()}
                FluidButton{glyph:"undo";tip:"Undo";enabled:rich.canUndo;onClicked:rich.undo()}
                FluidButton{glyph:"redo";tip:"Redo";enabled:rich.canRedo;onClicked:rich.redo()}
            }
            ScrollView{id:scroll;y:tools.height+Theme.unit(12);width:parent.width;height:parent.height-y;clip:true;contentWidth:availableWidth
                TextArea{id:body;objectName:"signatureBody";readOnly:!root.visual||!root.editable||root.busy;selectByMouse:true;persistentSelection:true;wrapMode:TextEdit.Wrap;textFormat:TextEdit.RichText;color:Theme.text;selectionColor:Theme.selected;font.pixelSize:Theme.unit(13);padding:Theme.unit(12);background:Rectangle{color:Theme.raised;radius:Theme.unit(12);border.width:1;border.color:body.activeFocus?Theme.accent:Theme.outline}
                    onSelectionStartChanged:rich.selection(selectionStart,selectionEnd,cursorPosition)
                    onSelectionEndChanged:rich.selection(selectionStart,selectionEnd,cursorPosition)
                    onCursorPositionChanged:rich.selection(selectionStart,selectionEnd,cursorPosition)
                    Keys.onPressed:event=>{
                        const ctrl=event.modifiers&Qt.ControlModifier
                        if(ctrl&&event.key===Qt.Key_B){rich.toggle("bold");event.accepted=true}
                        else if(ctrl&&event.key===Qt.Key_I){rich.toggle("italic");event.accepted=true}
                        else if(ctrl&&event.key===Qt.Key_U){rich.toggle("underline");event.accepted=true}
                        else if(ctrl&&event.key===Qt.Key_K){linkDialog.open();event.accepted=true}
                        else if(ctrl&&event.key===Qt.Key_Z){if(event.modifiers&Qt.ShiftModifier)rich.redo();else rich.undo();event.accepted=true}
                        else if(ctrl&&event.key===Qt.Key_Y){rich.redo();event.accepted=true}
                        else if(event.key===Qt.Key_Tab||event.key===Qt.Key_Backtab){const back=event.key===Qt.Key_Backtab||!!(event.modifiers&Qt.ShiftModifier);if(rich.inList()){rich.indent(back?-1:1);event.accepted=true}else event.accepted=rich.tableTab(back)}
                        else if((event.key===Qt.Key_Return||event.key===Qt.Key_Enter)&&!(event.modifiers&Qt.ShiftModifier))event.accepted=rich.paragraphBreak()
                    }
                    TapHandler{acceptedModifiers:Qt.ControlModifier;onTapped:event=>{const link=body.linkAt(event.position.x,event.position.y);if(link)Mail.openLink(link)}}
                }
            }
            RichDocument{id:rich;objectName:"signatureDocument";document:body.textDocument;uiScale:Theme.scale;locked:!root.visual||!root.editable||root.busy;onEdited:root.changed();onPlaceCursor:(start,end)=>{body.select(start,end)};onError:message=>root.notice=message}
            Connections{target:root
                function onVisualChanged(){if(root.visual)Qt.callLater(content.focusBody)}
            }
        }
    }
    Dialog{id:linkDialog;title:"Insert signature link";modal:true;anchors.centerIn:Overlay.overlay;width:Math.min(440,root.width-40);standardButtons:Dialog.Ok|Dialog.Cancel
        Column{width:parent.width;spacing:10;FluidField{id:linkLabel;width:parent.width;placeholderText:"Text to display"}FluidField{id:linkUrl;width:parent.width;placeholderText:"https://…"}}
        onAccepted:if(root.editor)root.editor.hyperlink(linkUrl.text,linkLabel.text)
    }
    Dialog{id:sourceDialog;title:"Signature HTML · read only";modal:true;anchors.centerIn:Overlay.overlay;width:Math.min(700,root.width-50);height:Math.min(480,root.height-60);standardButtons:Dialog.Close
        ScrollView{anchors.fill:parent;TextArea{id:sourceText;readOnly:true;selectByMouse:true;wrapMode:TextEdit.Wrap;textFormat:TextEdit.PlainText}}
        onOpened:sourceText.text=root.html()
        onClosed:sourceText.clear()
    }
    Dialog{id:pictureDialog;title:"Insert hosted HTTPS picture";modal:true;anchors.centerIn:Overlay.overlay;width:Math.min(480,root.width-40);standardButtons:Dialog.Ok|Dialog.Cancel
        Column{width:parent.width;spacing:12;Text{textFormat:Text.PlainText;width:parent.width;wrapMode:Text.Wrap;text:"Gmail stores the public image URL. Preview loading is explicit and anonymous.";color:Theme.muted}FluidField{id:pictureUrl;width:parent.width;placeholderText:"https://example.com/logo.png"}}
        onAccepted:root.insertPicture(pictureUrl.text)
    }
    Dialog{id:tableDialog;title:"Insert table";modal:true;anchors.centerIn:Overlay.overlay;width:Math.min(360,root.width-40);standardButtons:Dialog.Ok|Dialog.Cancel
        Row{spacing:12;FluidField{id:tableRows;width:120;text:"2";placeholderText:"Rows";validator:IntValidator{bottom:1;top:10}}FluidField{id:tableColumns;width:120;text:"2";placeholderText:"Columns";validator:IntValidator{bottom:1;top:10}}}
        onAccepted:if(root.editor)root.editor.insertTable(Number(tableRows.text),Number(tableColumns.text))
    }
    Dialog{id:colourDialog;property bool highlight:false;title:highlight?"Highlight colour":"Font colour";modal:true;anchors.centerIn:Overlay.overlay;width:Math.min(420,root.width-40);standardButtons:Dialog.Ok|Dialog.Cancel
        Column{width:parent.width;spacing:12;Flow{width:parent.width;spacing:8;Repeater{model:["#171717","#ffffff","#1677ff","#ba36a5","#cb3234","#1c8a62","#e7b43e"];delegate:Rectangle{required property string modelData;width:30;height:30;radius:15;color:modelData;border.width:1;border.color:Theme.outline;MouseArea{anchors.fill:parent;onClicked:colourValue.text=parent.modelData}}}}FluidField{id:colourValue;width:parent.width;text:"#1677ff";placeholderText:"#RRGGBB"}}
        onAccepted:if(root.editor)root.editor.colour(colourValue.text,highlight)
    }
    ConfirmSheet{id:publicQuestion;objectName:"signaturePublicQuestion";anchors.fill:parent
        function open(){ask("Upload a public signature picture?","An optimized copy of "+root.pictureName+" will be uploaded to your Google Drive. Anyone with its link can view it, including people outside Gmail. Your original file stays unchanged. Removing the signature picture does not delete the Drive copy.","Upload public copy","signatureUploadPicture",{session:root.session,path:root.picturePath,publicConfirmed:true})}
    }
    Dialog{id:closeQuestion;objectName:"signatureCloseQuestion";title:"Save the signature before closing?";modal:true;closePolicy:Popup.NoAutoClose;anchors.centerIn:Overlay.overlay;width:Math.min(510,root.width-40)
        Column{width:parent.width;spacing:16;Text{textFormat:Text.PlainText;width:parent.width;wrapMode:Text.Wrap;color:Theme.text;text:"Your changes exist only in this editor. Gmail’s saved signature is kept until Save succeeds."}Flow{width:parent.width;spacing:8;FluidButton{text:"Keep editing";onClicked:{Mail.send("signatureKeepEditing",{session:root.session});closeQuestion.close()}}FluidButton{text:"Discard changes";onClicked:root.discard()}FluidButton{text:"Save and close";primary:true;onClicked:root.save(true)}}}
    }
    Shortcut{sequence:"Ctrl+S";enabled:root.opened&&!root.dialogsOpen&&root.dirty&&!root.busy;onActivated:root.save()}
    Shortcut{sequence:"Ctrl+W";enabled:root.opened&&!root.dialogsOpen&&!root.busy;onActivated:root.requestClose()}
    Connections{target:Mail
        function onSignaturePictureChosen(session,path,name){if(root.opened&&root.session===session&&root.visual&&!root.busy){root.picturePath=path;root.pictureName=name;publicQuestion.open()}}
        function onFlushComposer(){if(root.opened)Mail.send("signatureUpdate",{session:root.session,dirty:root.dirty})}
        function onReceived(kind,data){
            if(kind==="signatureOpened"){root.loading=true;root.session=data.session;root.account=data.account;root.identity=data.identity;root.savedHtml=data.html;root.editable=data.editable;root.visual=false;root.dirty=false;root.busy=false;root.notice="";root.present=true;root.opened=true;root.loading=false;return}
            if(data.session!==root.session||!root.session)return
            if(kind==="signatureStatus"){root.busy=data.busy;if(data.operation)root.operation=data.operation;root.notice=data.status||root.notice;if(!data.busy){root.progress={};if(root.pendingPicture)Qt.callLater(()=>{if(root.opened&&root.pendingPicture){const url=root.pendingPicture;root.pendingPicture="";root.insertPicture(url)}})}}
            else if(kind==="signatureProgress")root.progress=data
            else if(kind==="signatureCloseQuestion")closeQuestion.open()
            else if(kind==="signatureSaved"){root.savedHtml=data.html;root.dirty=false;root.notice=data.preview?"Preview saved in RAM · no Gmail call":"Saved to Gmail"}
            else if(kind==="signaturePicture"&&root.editor)root.editor.preview(data.url,data.bytes)
            else if(kind==="signatureUploaded"&&root.editor){root.editor.preview(data.url,data.bytes);root.pendingPicture=data.url;root.notice=data.preview?"Preview picture inserted":"Public picture copy uploaded · save the signature to keep it in Gmail"}
            else if(kind==="signaturePicturesLoaded")root.notice=data.count+" pictures loaded anonymously into memory"
            else if(kind==="signatureClosed"){root.opened=false;root.closePopups();root.finishClose();if(data.continueQuit)Mail.send("composePrepareQuit")}
        }
    }
}
