import QtQuick
import QtQuick.Controls
Item {
    id:root;objectName:"searchSheet";z:160;visible:shown
    property bool shown:false
    property bool opened:false
    property bool pending:false
    property rect origin:Qt.rect(width/2,height/2,200,37)
    property string folder:"inbox"
    property bool allAccounts:true
    property var selected:[]
    property string builtQuery:""
    readonly property bool labelScoped:folder.startsWith("label:")
    readonly property real goalWidth:Math.min(width-Theme.unit(48),Theme.unit(720))
    readonly property real goalHeight:Math.min(height-Theme.unit(64),Theme.unit(760))
    readonly property bool moving:horizontal.running||vertical.running||breadth.running||extent.running
    function openFrom(rect){
        origin=rect;if(!shown){horizontal.snap(rect.x);vertical.snap(rect.y);breadth.snap(rect.width);extent.snap(rect.height)}
        folder=Mail.folder;allAccounts=Mail.searchAllAccounts&&!labelScoped;selected=Mail.searchAccounts.slice()
        if(Mail.search!==builtQuery){from.text="";to.text="";subject.text="";words.text=Mail.search;after.text="";before.text="";unread.checked=false;starred.checked=false;attachments.checked=false}
        pending=false;shown=true;opened=true;Qt.callLater(()=>from.forceActiveFocus())
    }
    function close(){if(pending)Mail.cancelSearch();pending=false;opened=false;Qt.callLater(finishClose)}
    function finishClose(){if(shown&&!opened&&!moving)shown=false}
    function choose(id,checked){let ids=selected.slice();const index=ids.indexOf(id);if(checked&&index<0)ids.push(id);else if(!checked&&index>=0)ids.splice(index,1);selected=ids}
    function submit(){if(pending)return;pending=true;Mail.send("searchBuild",{folder:folder,allAccounts:allAccounts,accounts:selected,from:from.text,to:to.text,subject:subject.text,words:words.text,after:after.text,before:before.text,unread:unread.checked,starred:starred.checked,attachments:attachments.checked})}
    Motion{id:horizontal;target:root.opened?(root.width-root.goalWidth)/2:root.origin.x;enabled:Theme.motion;onRunningChanged:Qt.callLater(root.finishClose)}
    Motion{id:vertical;target:root.opened?(root.height-root.goalHeight)/2:root.origin.y;enabled:Theme.motion;onRunningChanged:Qt.callLater(root.finishClose)}
    Motion{id:breadth;target:root.opened?root.goalWidth:root.origin.width;enabled:Theme.motion;onRunningChanged:Qt.callLater(root.finishClose)}
    Motion{id:extent;target:root.opened?root.goalHeight:root.origin.height;enabled:Theme.motion;onRunningChanged:Qt.callLater(root.finishClose)}
    Motion{id:shade;target:root.opened?.4:0;enabled:Theme.motion;epsilon:.002}
    Rectangle{anchors.fill:parent;color:"black";opacity:Math.max(0,Math.min(.5,shade.value));MouseArea{anchors.fill:parent;onClicked:root.close()}}
    Rectangle{id:card;objectName:"searchCard";x:horizontal.value;y:vertical.value;width:Math.max(1,breadth.value);height:Math.max(1,extent.value);radius:Theme.unit(20);color:Theme.surface;border.width:1;border.color:Theme.outline;clip:true
        MouseArea{anchors.fill:parent;acceptedButtons:Qt.AllButtons}
        Icon{x:Theme.unit(24);y:Theme.unit(25);name:"search";color:Theme.accent}
        Text{textFormat:Text.PlainText;x:Theme.unit(54);y:Theme.unit(22);text:"Search mail";color:Theme.text;font.pixelSize:Theme.unit(21);font.weight:Font.DemiBold}
        FluidButton{x:root.goalWidth-Theme.unit(55);y:Theme.unit(17);glyph:"close";tip:"Close search filters";onClicked:root.close()}
        ScrollView{id:scroll;x:Theme.unit(24);y:Theme.unit(76);width:root.goalWidth-2*x;height:root.goalHeight-y-Theme.unit(79);contentWidth:availableWidth;clip:true
            Column{width:scroll.availableWidth;spacing:Theme.unit(17)
                Text{textFormat:Text.PlainText;width:parent.width;wrapMode:Text.Wrap;text:"Search "+Theme.folderTitle(root.folder)+" on Gmail. Older results load in bounded pages.";color:Theme.muted;font.pixelSize:Theme.unit(11)}
                Grid{id:fields;width:parent.width;columns:2;columnSpacing:Theme.unit(12);rowSpacing:Theme.unit(12)
                    FluidField{id:from;objectName:"searchFrom";width:(parent.width-parent.columnSpacing)/2;placeholderText:"From address or name";maximumLength:1024}
                    FluidField{id:to;objectName:"searchTo";width:from.width;placeholderText:"To address or name";maximumLength:1024}
                    FluidField{id:subject;objectName:"searchSubject";width:from.width;placeholderText:"Subject contains";maximumLength:1024}
                    FluidField{id:words;objectName:"searchWords";width:from.width;placeholderText:"Words or Gmail operators";maximumLength:8192}
                    FluidField{id:after;objectName:"searchAfter";width:from.width;placeholderText:"After · YYYY-MM-DD";maximumLength:10}
                    FluidField{id:before;objectName:"searchBefore";width:from.width;placeholderText:"Before · YYYY-MM-DD";maximumLength:10}
                }
                Flow{width:parent.width;spacing:Theme.unit(14)
                    FluidCheck{id:unread;text:"Unread"}
                    FluidCheck{id:starred;text:"Starred"}
                    FluidCheck{id:attachments;text:"Has attachments"}
                }
                Rectangle{width:parent.width;height:1;color:Theme.outline}
                Text{textFormat:Text.PlainText;text:"ACCOUNTS";color:Theme.faint;font.pixelSize:Theme.unit(10);font.letterSpacing:1}
                FluidCheck{text:"All enabled accounts";checked:root.allAccounts;enabled:!root.labelScoped;onToggled:root.allAccounts=checked}
                ListView{id:accounts;width:parent.width;height:Math.min(count,5)*Theme.unit(42);model:(Mail.settings.accounts||[]).filter(a=>a.enabled!==false);clip:true;reuseItems:true;cacheBuffer:Theme.unit(42)
                    ScrollBar.vertical:ScrollBar{implicitWidth:3}
                    delegate:FluidCheck{required property var modelData;width:accounts.width;height:Theme.unit(42);text:modelData.email;checked:root.allAccounts||root.selected.indexOf(modelData.id)>=0;enabled:!root.allAccounts&&(!root.labelScoped||modelData.id===Mail.account);onToggled:root.choose(modelData.id,checked)}
                }
                Text{textFormat:Text.PlainText;width:parent.width;wrapMode:Text.Wrap;text:root.labelScoped?"This label belongs to the selected account. Choose All Mail to search across accounts.":"Search uses Gmail’s servers. Luma creates no local index or historical mailbox download.";color:Theme.muted;font.pixelSize:Theme.unit(11)}
            }
        }
        Row{x:Theme.unit(24);y:root.goalHeight-Theme.unit(59);spacing:Theme.unit(12)
            FluidButton{text:"Cancel";onClicked:root.close()}
            FluidButton{text:root.pending?"Searching…":"Search";primary:true;enabled:!root.pending&&(root.allAccounts||root.selected.length>0);onClicked:root.submit()}
        }
    }
    Connections{target:Mail
        function onReceived(kind,data){if(kind==="searchBuilt"&&root.pending){root.builtQuery=data.query;root.pending=false;root.close()}}
        function onErrorChanged(){if(Mail.error)root.pending=false}
    }
}
