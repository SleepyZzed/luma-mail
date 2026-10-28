import QtQuick
import QtQuick.Controls.Basic
import QtQml.Models
ApplicationWindow {
    id:window
    width:1440;height:900;minimumWidth:760;minimumHeight:560;visible:true;title:"Luma Mail";color:Theme.window
    flags:(isWindows?Qt.Window:Qt.Window|Qt.FramelessWindowHint)|(resourceFixture?Qt.WindowDoesNotAcceptFocus:0)
    font.family:Theme.family
    property bool geometryReady:false
    function restoreGeometry(){
        if(!Mail.preview){width=Math.max(minimumWidth,Math.min(Mail.settings.window_width||1320,Screen.desktopAvailableWidth));height=Math.max(minimumHeight,Math.min(Mail.settings.window_height||840,Screen.desktopAvailableHeight))}
        geometryReady=true
    }
    function flushGeometry(){geometrySave.stop();if(geometryReady&&!Mail.preview&&visibility===Window.Windowed)Mail.send("settingsUpdate",{window_width:Math.round(width),window_height:Math.round(height)})}
    onWidthChanged:if(geometryReady&&!Mail.preview&&visibility===Window.Windowed)geometrySave.restart()
    onHeightChanged:if(geometryReady&&!Mail.preview&&visibility===Window.Windowed)geometrySave.restart()
    Timer{id:geometrySave;interval:500;onTriggered:window.flushGeometry()}
    readonly property bool appModal:authSheet.visible||settings.visible||searchSheet.shown||comparison.visible||signature.visible||confirmation.visible||labelDialog.visible||attachmentPane.shown
    function accountShortcut(index){const accounts=(Mail.settings.accounts||[]).filter(account=>account.enabled!==false);if(index<accounts.length)Mail.setView("inbox",accounts[index].id)}
    function scaleBy(delta){Mail.send("settingsUpdate",{ui_scale_percent:Math.max(75,Math.min(200,(Mail.settings.ui_scale_percent||100)+delta))})}
    onClosing:(close)=>{flushGeometry();if((Mail.settings.close_to_tray===undefined||Mail.settings.close_to_tray)&&trayAvailable){close.accepted=false;window.hide()}else{close.accepted=false;Mail.quit()}}
    Item {
        id:scene;anchors.fill:parent
        readonly property real railGoal:Theme.unit(rail.collapsed?76:railHandle.value)
        readonly property real inboxGoal:Mail.readerOpen?(width<Theme.unit(1000)?0:Math.min(Theme.unit(inboxHandle.value),Math.max(0,width-railGoal-Theme.unit(380)))):width-railGoal
        readonly property real readerGoalX:railGoal+inboxGoal
        readonly property real readerGoalWidth:Mail.readerOpen?width-readerGoalX:reader.lastLayoutWidth
        function projectionSpec(){return reader.projectionSpec(readerGoalX,readerGoalWidth)}
        Motion{id:railMotion;target:scene.railGoal;enabled:Theme.motion}
        Motion{id:inboxMotion;target:scene.inboxGoal;enabled:Theme.motion}
        Sidebar {
            id:rail;width:railMotion.value;height:parent.height
            onComposeAt:(x,y,w,h)=>{composer.origin=Qt.rect(x,y,w,h);Mail.requestCompose("new")}
            onSettingsRequested:settings.opened=true
            onLabelActionRequested:(operation,account,label)=>labelDialog.edit(operation,account,label)
        }
        Inbox {
            id:inbox;scene:scene;searchMorphed:searchSheet.shown;
            onSearchRequested:origin=>searchSheet.openFrom(origin)
            x:rail.width;height:parent.height-Theme.unit(44)
            width:Math.max(0,inboxMotion.value)
            onSettingsRequested:settings.opened=true
            onContextRequested:(row,x,y)=>{context.row=row;context.x=x;context.y=y;context.open()}
        }
        Reader {
            id:reader;x:inbox.x+inbox.width;width:Math.max(0,scene.width-x);height:parent.height-Theme.unit(44);layoutWidth:scene.readerGoalWidth;projected:morph.active;inlineComposer:composer;visible:width>1
            onReplyAt:(mode,message)=>Mail.requestCompose(mode,message)
            onMessageMenuRequested:(message,x,y)=>{messageMenu.message=message;messageMenu.threadKey=Mail.reader.key;messageMenu.x=x;messageMenu.y=y;messageMenu.open()}
            onContextRequested:(row,x,y)=>{context.row=row;context.x=x;context.y=y;context.open()}
            nativeBlocked:authSheet.visible||settings.visible||context.visible||messageMenu.visible||rail.overlayOpen||errorToast.visible||composer.modalOpen||attachmentPane.shown||labelDialog.visible||confirmation.visible||searchSheet.shown||comparison.visible||signature.visible
            nativeMasks:(composer.shown&&!composer.inlineMode&&!composer.detached?[Qt.rect(composer.x,composer.y,composer.width,composer.height)]:[]).concat(downloadPill.visible?[Qt.rect(downloadPill.x,downloadPill.y,downloadPill.width,downloadPill.height)]:[])
        }
        PaneHandle{id:railHandle;objectName:"navigationResize";x:rail.width-width/2;height:parent.height;z:50;visible:!rail.collapsed;initial:Math.max(160,Math.min(480,Mail.settings.sidebar_width||240));minimum:160;maximum:Math.min(480,scene.width/Theme.scale*.4);onCommitted:value=>Mail.send("settingsUpdate",{sidebar_width:Math.round(value)})}
        PaneHandle{id:inboxHandle;objectName:"inboxResize";x:reader.x-width/2;height:parent.height;z:50;visible:Mail.readerOpen&&inbox.width>1;initial:Math.max(180,Math.min(1200,Mail.settings.list_width||450));minimum:180;maximum:Math.min(1200,Math.max(180,(scene.width-scene.railGoal)/Theme.scale-380));onCommitted:value=>Mail.send("settingsUpdate",{list_width:Math.round(value)})}
        ActivityPill{x:rail.width+Theme.unit(28);anchors.bottom:parent.bottom;anchors.bottomMargin:Theme.unit(13)}
        FluidButton{anchors.right:parent.right;anchors.rightMargin:Theme.unit(20);anchors.bottom:parent.bottom;anchors.bottomMargin:Theme.unit(9);text:"Undo";glyph:"undo";visible:Mail.canUndo;onClicked:Mail.send("undo")}
        RowMorph{id:morph}
        Composer{id:composer;scene:scene;homeScene:scene;shown:false;externalModal:authSheet.visible||labelDialog.opened||settings.opened||confirmation.opened||context.visible||messageMenu.visible||rail.overlayOpen||searchSheet.shown||comparison.visible||signature.visible;onComparisonRequested:(session,form)=>comparison.begin(session,form)}
        AuthSheet{id:authSheet;anchors.fill:parent}
        SettingsSheet{id:settings;anchors.fill:parent;onCloseRequested:opened=false;onSignatureRequested:account=>Mail.send("signatureOpen",{account:account});onConfirmationRequested:(heading,message,buttonText,command,parameters)=>confirmation.ask(heading,message,buttonText,command,parameters)}
        LabelDialog{id:labelDialog;anchors.fill:parent}
        ConfirmSheet{id:confirmation;anchors.fill:parent}
        SearchSheet{id:searchSheet;anchors.fill:parent}
        DraftCompareSheet{id:comparison;anchors.fill:parent}
        SignatureSheet{id:signature;anchors.fill:parent}
        AttachmentPane{id:attachmentPane;anchors.fill:parent}
        DownloadPill{id:downloadPill;anchors.right:parent.right;anchors.bottom:parent.bottom;anchors.margins:Theme.unit(18)}
        Rectangle {
            id:errorToast;anchors.horizontalCenter:parent.horizontalCenter;anchors.bottom:parent.bottom;anchors.bottomMargin:Theme.unit(58);width:Math.min(Theme.unit(600),parent.width-Theme.unit(64));height:message.implicitHeight+Theme.unit(30);radius:Theme.unit(16);color:Theme.raised;border.width:1;border.color:Theme.accent;visible:Mail.error!=="";z:200
            Text {textFormat:Text.PlainText;id:message;x:Theme.unit(16);y:Theme.unit(15);width:parent.width-Theme.unit(65);text:Mail.error;wrapMode:Text.Wrap;color:Theme.text;font.pixelSize:Theme.unit(12)}
            FluidButton{anchors.right:parent.right;anchors.verticalCenter:parent.verticalCenter;glyph:"close";onClicked:Mail.clearError()}
        }
        MailActionsMenu{id:context;objectName:"mailActionsMenu";onConfirmationRequested:(heading,message,buttonText,command,parameters)=>confirmation.ask(heading,message,buttonText,command,parameters)}
        MessageActionsMenu{id:messageMenu;onDetailsRequested:message=>reader.showDetails(message)}

    }
    Connections {
        target:Mail
        function onReady(){window.restoreGeometry()}
        function onFlushComposer(){window.flushGeometry()}
        function onOpenRowRequested(index){inbox.revealIndex(index)}
        function onProjectRow(row,x,y,w,h){morph.place(row,Qt.rect(x,y,w,h),scene.projectionSpec(),true)}
        function onReturnRow(row){const target=inbox.projectionTarget(row.key,scene.width-scene.railGoal);if(target)morph.place(row,target,scene.projectionSpec(),false)}
        function onReaderChanged(){
            if(morph.active&&morph.opening&&Mail.readerOpen)morph.destination=scene.projectionSpec()
            const key=Mail.composer.key,reading=Mail.reader.key
            if(composer.shown&&(!Mail.readerOpen||!key||!reading||key.id!==reading.id||key.account!==reading.account)){
                const wasInline=composer.inlineMode
                composer.inlineDock=null
                if(wasInline){composer.floatToScene();if(!composer.sendSubmitted&&!composer.busy&&!composer.statusMode)composer.requestClose()}
            }
        }
        function onComposeRequested(mode){
            if(composer.detached)composer.returnHome(false)
            composer.openingSurface=composer.visible
            const key=Mail.composer.key,reading=Mail.reader.key
            if((mode==="reply"||mode==="replyAll"||mode==="forward"||mode==="forwardAll")&&key&&reading&&key.id===reading.id&&key.account===reading.account)reader.dockInline(composer)
            else{composer.inlineDock=null;if(composer.parent!==scene)composer.floatToScene();composer.inlineMode=false}
            composer.open(mode)
        }
        function onComposeFocused(){composer.focusEditor();if(composer.inlineMode)reader.dockInline(composer)}
    }
    Shortcut{sequence:"Ctrl+N";enabled:!window.appModal;onActivated:Mail.requestCompose("new")}
    Shortcut{sequence:"Escape";enabled:!composer.recipientSuggestionsOpen||authSheet.opened||searchSheet.shown||comparison.visible||signature.visible;onActivated:{if(authSheet.opened)Mail.cancelAuth();else if(signature.opened){if(!signature.dialogsOpen)signature.requestClose()}else if(comparison.opened){if(!comparison.pending)comparison.close()}else if(searchSheet.shown)searchSheet.close();else if(confirmation.opened)confirmation.opened=false;else if(labelDialog.opened){if(!labelDialog.pending)labelDialog.opened=false}else if(attachmentPane.shown)attachmentPane.close();else if(settings.opened)settings.opened=false;else if(composer.shown)composer.requestClose();else if(Mail.selecting)Mail.clearSelection();else Mail.closeReader()}}
    Shortcut{sequence:"Ctrl+R";enabled:!window.appModal;onActivated:Mail.refreshView()}
    Shortcut{sequence:"Ctrl+F";enabled:!window.appModal&&!composer.shown;onActivated:inbox.focusSearch()}
    Shortcut{sequence:"Ctrl+,";enabled:!window.appModal;onActivated:settings.opened=true}
    Shortcut{sequence:"Ctrl+Q";onActivated:Mail.quit()}
    Shortcut{sequence:"Ctrl+W";enabled:!window.appModal;onActivated:if(composer.shown)composer.requestClose();else window.close()}
    Shortcut{sequences:["Ctrl++","Ctrl+="];enabled:!window.appModal;onActivated:window.scaleBy(10)}
    Shortcut{sequence:"Ctrl+-";enabled:!window.appModal;onActivated:window.scaleBy(-10)}
    Shortcut{sequence:"Ctrl+0";enabled:!window.appModal;onActivated:Mail.send("settingsUpdate",{ui_scale_percent:100})}
    Instantiator{model:9;delegate:Shortcut{required property int index;sequence:"Ctrl+"+(index+1);enabled:!window.appModal&&!composer.shown;onActivated:window.accountShortcut(index)}}
    Shortcut{sequence:"Alt+0";enabled:!window.appModal&&!composer.shown;onActivated:Mail.setView("inbox","")}
    Shortcut{sequence:"Ctrl+E";enabled:!window.appModal&&!composer.shown&&(inbox.listFocused||Mail.selecting);onActivated:Mail.mutate("archive")}
    Shortcut{sequence:"Ctrl+Shift+U";enabled:!window.appModal&&!composer.shown&&(inbox.listFocused||Mail.selecting);onActivated:Mail.mutate("unread")}
    Shortcut{sequence:"Delete";enabled:!window.appModal&&!composer.shown&&inbox.listFocused;onActivated:Mail.mutate("trash")}
}
