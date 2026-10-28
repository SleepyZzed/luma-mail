import QtQuick
import QtQuick.Controls
Item {
    id:root;objectName:"draftCompareSheet";z:170;visible:opened||position.running
    property bool opened:false
    property bool pending:false
    property string session:""
    property var review:({})
    property var choices:["","","","","",""]
    property string attachments:""
    property bool showUnchanged:false
    readonly property bool ready:!!review.fields&&!pending&&attachments!==""&&review.fields.every((field,index)=>field.resolution!=="choose"||choices[index]!=="")
    readonly property real cardHeight:Math.min(height-Theme.unit(48),Theme.unit(800))
    function begin(session,form){root.session=session;review={};choices=["","","","","",""];attachments="";showUnchanged=false;pending=true;opened=true;Mail.send("composeCompare",{session:session,form:form})}
    function close(){if(session===Mail.composer.session)Mail.send("composeCompareCancel",{session:session});pending=false;opened=false;release()}
    function release(){if(!opened&&!position.running){review={};session=""}}
    function choose(index,choice){let values=choices.slice();values[index]=choice;choices=values}
    function merge(){if(!ready)return;pending=true;Mail.send("composeMerge",{session:session,ticket:review.ticket,choices:choices,attachments:attachments})}
    Motion{id:position;target:root.opened?(root.height-root.cardHeight)/2:root.height+Theme.unit(24);enabled:Theme.motion;onRunningChanged:Qt.callLater(root.release)}
    Motion{id:shade;target:root.opened?.4:0;enabled:Theme.motion;epsilon:.002}
    Rectangle{anchors.fill:parent;color:"black";opacity:Math.max(0,Math.min(.5,shade.value));MouseArea{anchors.fill:parent;onClicked:if(!root.pending)root.close()}}
    Rectangle{id:card;objectName:"draftComparisonCard";x:(parent.width-width)/2;y:position.value;width:Math.min(parent.width-Theme.unit(48),Theme.unit(900));height:root.cardHeight;radius:Theme.unit(22);color:Theme.surface;border.width:1;border.color:Theme.outline;clip:true
        MouseArea{anchors.fill:parent;acceptedButtons:Qt.AllButtons}
        Text{textFormat:Text.PlainText;x:Theme.unit(24);y:Theme.unit(23);text:"Compare draft versions";color:Theme.text;font.pixelSize:Theme.unit(21);font.weight:Font.DemiBold}
        FluidButton{anchors.right:parent.right;anchors.rightMargin:Theme.unit(18);y:Theme.unit(17);glyph:"close";tip:"Close comparison";enabled:!root.pending||!root.review.fields;onClicked:root.close()}
        Text{textFormat:Text.PlainText;x:Theme.unit(24);y:Theme.unit(65);width:parent.width-2*x;wrapMode:Text.Wrap;text:root.pending&&!root.review.fields?"Reading the current Gmail version…":"Changes to separate fields are combined. Choose a version for each conflict and an attachment set. The result opens as an unsaved copy for review; the original Gmail draft stays intact.";color:Theme.muted;font.pixelSize:Theme.unit(11)}
        Loader{id:content;active:root.opened||position.running;x:Theme.unit(24);y:Theme.unit(121);width:parent.width-2*x;height:parent.height-y-Theme.unit(78)
            sourceComponent:ScrollView{clip:true;contentWidth:availableWidth
                Column{width:content.width-Theme.unit(12);spacing:Theme.unit(16)
                    FluidCheck{text:"Show unchanged fields";checked:root.showUnchanged;visible:!!root.review.fields;onToggled:root.showUnchanged=checked}
                    Repeater{model:root.review.fields||[]
                        delegate:Column{id:field;required property var modelData;required property int index;width:parent.width;spacing:Theme.unit(7);visible:modelData.resolution!=="same"||root.showUnchanged
                            Row{width:parent.width;spacing:Theme.unit(12)
                                Text{textFormat:Text.PlainText;width:parent.width-Theme.unit(240);text:modelData.name;color:Theme.text;font.pixelSize:Theme.unit(13);font.weight:Font.DemiBold}
                                FluidCombo{objectName:"mergeChoice"+field.index;width:Theme.unit(225);visible:field.modelData.resolution==="choose";model:["Choose a version…","Keep my version","Keep Gmail version"];onActivated:root.choose(field.index,currentIndex===1?"mine":currentIndex===2?"gmail":"")}
                                Text{textFormat:Text.PlainText;visible:modelData.resolution!=="choose";text:modelData.resolution==="same"?"Unchanged":modelData.resolution==="mine"?"Your change":"Gmail change";color:Theme.accent;font.pixelSize:Theme.unit(11)}
                            }
                            Row{width:parent.width;spacing:Theme.unit(12)
                                Repeater{model:[{heading:"Your version",text:modelData.mine},{heading:"Gmail version",text:modelData.gmail}]
                                    delegate:Rectangle{required property var modelData;width:(parent.width-Theme.unit(12))/2;height:field.index===5?Theme.unit(190):Theme.unit(78);radius:Theme.unit(10);color:Theme.raised;border.width:1;border.color:Theme.outline
                                        Text{textFormat:Text.PlainText;x:Theme.unit(12);y:Theme.unit(8);text:modelData.heading;color:Theme.faint;font.pixelSize:Theme.unit(10)}
                                        ScrollView{x:Theme.unit(12);y:Theme.unit(27);width:parent.width-2*x;height:parent.height-y-Theme.unit(8);clip:true
                                            TextArea{text:modelData.text;readOnly:true;selectByMouse:true;textFormat:TextEdit.PlainText;wrapMode:TextEdit.Wrap;color:Theme.text;font.pixelSize:Theme.unit(11);padding:0;background:Item{}}
                                        }
                                    }
                                }
                            }
                        }
                    }
                    Text{textFormat:Text.PlainText;text:"Attachments";color:Theme.text;font.pixelSize:Theme.unit(13);font.weight:Font.DemiBold;visible:!!root.review.fields}
                    Text{textFormat:Text.PlainText;width:parent.width;wrapMode:Text.Wrap;text:"Your version: "+((root.review.mineAttachments||[]).join(", ")||"None")+"\nGmail version: "+((root.review.gmailAttachments||[]).join(", ")||"None");color:Theme.muted;font.pixelSize:Theme.unit(11);visible:!!root.review.fields}
                    FluidCombo{objectName:"mergeAttachments";width:Math.min(parent.width,Theme.unit(300));visible:!!root.review.fields;model:["Choose attachments…","Keep my attachments","Keep Gmail attachments"];onActivated:root.attachments=currentIndex===1?"mine":currentIndex===2?"gmail":""}
                }
            }
        }
        Row{x:Theme.unit(24);y:parent.height-Theme.unit(57);spacing:Theme.unit(12)
            FluidButton{text:"Keep editing";enabled:!root.pending||!root.review.fields;onClicked:root.close()}
            FluidButton{objectName:"openMergedCopy";text:root.pending?"Preparing…":"Open merged copy";primary:true;enabled:root.ready;onClicked:root.merge()}
        }
    }
    Connections{target:Mail
        function onReceived(kind,data){
            if(kind==="composeComparison"&&root.opened&&data.session===root.session){root.review=data;root.pending=false}
            else if(kind==="composeOpened"&&root.opened&&data.session!==root.session){root.pending=false;root.opened=false;root.release()}
        }
        function onComposerChanged(){if(root.opened&&(!Mail.composer.session||Mail.composer.session!==root.session)){root.pending=false;root.opened=false;root.release()}}
        function onErrorChanged(){if(Mail.error)root.pending=false}
    }
}
