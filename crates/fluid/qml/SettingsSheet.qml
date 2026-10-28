import QtQuick
import QtQuick.Controls.Basic
Item {
    id:root;objectName:"settingsPanel"
    property bool opened:false
    property bool contentRetained:false
    readonly property bool moving:position.running||backdrop.running
    function releaseClosedContent(){if(!opened&&!moving)contentRetained=false}
    onMovingChanged:if(!moving)Qt.callLater(releaseClosedContent)
    property real cacheBytes:-1
    property var accountCache:({})
    property string notice:""
    property var photoStats:({})
    property string rulesAccount:""
    function updatePhotoStats(){photoStats=Mail.profileStats()}
    function moveAccount(id,offset){let ids=(Mail.settings.accounts||[]).map(a=>a.id);const index=ids.indexOf(id),target=index+offset;if(index<0||target<0||target>=ids.length)return;ids.splice(index,1);ids.splice(target,0,id);Mail.send("accountReorder",{accounts:ids})}
    onOpenedChanged:{if(opened){contentRetained=true;notice="";Mail.send("cacheUsage");updatePhotoStats()}else Qt.callLater(releaseClosedContent)}
    signal closeRequested()
    signal signatureRequested(string account)
    signal confirmationRequested(string heading,string message,string buttonText,string command,var parameters)
    visible:opened||(contentRetained&&moving);z:100
    Motion{id:backdrop;target:root.opened?.45:0;epsilon:.002;enabled:Theme.motion}
    Motion{id:position;target:root.opened?(root.height-panel.height)/2:root.height+Theme.unit(30);enabled:Theme.motion}
    Rectangle{anchors.fill:parent;color:"#000";opacity:Math.max(0,Math.min(.5,backdrop.value)); MouseArea{anchors.fill:parent;onClicked:root.closeRequested()}}
    Rectangle {
        id:panel;width:Math.min(parent.width-Theme.unit(48),Theme.unit(620));height:Math.min(parent.height-Theme.unit(48),Theme.unit(700));x:(parent.width-width)/2;y:position.value;radius:Theme.unit(25);color:Theme.surface;border.width:1;border.color:Theme.outline
        Text {textFormat:Text.PlainText;x:Theme.unit(30);y:Theme.unit(30);text:"Make yourself at home";font.pixelSize:Theme.unit(23);font.weight:Font.DemiBold;color:Theme.text}
        FluidButton{anchors.right:parent.right;anchors.rightMargin:Theme.unit(18);y:Theme.unit(24);glyph:"close";tip:"Close settings";onClicked:root.closeRequested()}
        Loader{id:contentLoader;active:root.contentRetained;x:Theme.unit(30);y:Theme.unit(93);width:parent.width-2*x;height:parent.height-y-Theme.unit(24)
        sourceComponent:ScrollView{id:settingsScroll;objectName:"settingsContentScroll";anchors.fill:parent;contentWidth:availableWidth;clip:true
        Column{width:settingsScroll.availableWidth;spacing:Theme.unit(20)
            Text {textFormat:Text.PlainText;text:"ACCOUNTS";font.pixelSize:Theme.unit(10);font.letterSpacing:1.2;color:Theme.faint}
            Repeater{model:Mail.accounts;delegate:Column{
                id:accountSettings;required property var item;required property int index;width:parent.width;spacing:Theme.unit(8)
                property bool initialized:false
                property string remoteName:""
                Component.onCompleted:{remoteName=item.name||"";accountName.text=remoteName;initialized=true}
                onItemChanged:if(initialized&&item.name!==remoteName){remoteName=item.name||"";if(!accountName.activeFocus)accountName.text=remoteName}
                Row{width:parent.width;spacing:Theme.unit(14)
                    Avatar{sender:accountSettings.item.name||accountSettings.item.email;account:accountSettings.item.id;photoSender:accountSettings.item.email;active:root.opened}
                    Column{width:parent.width-Theme.unit(54);spacing:5
                        Text{textFormat:Text.PlainText;width:parent.width;elide:Text.ElideRight;text:accountSettings.item.name||accountSettings.item.email;color:Theme.text;font.pixelSize:Theme.unit(13)}
                        Text{textFormat:Text.PlainText;width:parent.width;elide:Text.ElideMiddle;text:accountSettings.item.email;color:Theme.muted;font.pixelSize:Theme.unit(11)}
                    }
                }
                Flow{width:parent.width;spacing:Theme.unit(8)
                    FluidCheck{text:"Enabled";checked:accountSettings.item.enabled!==false;onToggled:Mail.send("accountUpdate",{account:accountSettings.item.id,enabled:checked})}
                    FluidButton{text:Mail.settings.default_account===accountSettings.item.id?"Default sender":"Use as default";enabled:accountSettings.item.enabled!==false;onClicked:Mail.send("settingsUpdate",{default_account:accountSettings.item.id})}
                    FluidCheck{text:"Notifications";checked:accountSettings.item.notifications||false;onToggled:Mail.send("accountUpdate",{account:accountSettings.item.id,notifications:checked})}
                    FluidCheck{text:"Contact photos";checked:accountSettings.item.contact_photos||false;onToggled:{if(checked)Mail.send("authStart",{account:accountSettings.item.id,photos:true});else Mail.send("accountUpdate",{account:accountSettings.item.id,contact_photos:false})}}
                    FluidButton{text:"Reconnect";tip:"Reconnect this Google account and its enabled features";onClicked:Mail.send("authStart",{account:accountSettings.item.id})}
                    FluidCheck{text:"Drive links";checked:accountSettings.item.drive_enabled||false;onToggled:{if(checked)Mail.send("authStart",{account:accountSettings.item.id,drive:true});else Mail.send("accountUpdate",{account:accountSettings.item.id,drive_enabled:false})}}
                    FluidButton{text:accountSettings.item.sender_controls?"Blocked senders":"Connect Gmail sender controls";onClicked:{if(accountSettings.item.sender_controls)root.rulesAccount=root.rulesAccount===accountSettings.item.id?"":accountSettings.item.id;else Mail.send("authStart",{account:accountSettings.item.id,settings:true})}}
                }
                Row{width:parent.width;spacing:Theme.unit(8)
                    FluidField{id:accountName;objectName:"accountName-"+accountSettings.item.id;width:parent.width-Theme.unit(120);placeholderText:"Account display name";maximumLength:256;onAccepted:Mail.send("accountUpdate",{account:accountSettings.item.id,name:text})}
                    FluidButton{text:"Rename";onClicked:Mail.send("accountUpdate",{account:accountSettings.item.id,name:accountName.text})}
                }
                Flow{width:parent.width;spacing:Theme.unit(8)
                    FluidButton{glyph:"chevron-up";tip:"Move account up";enabled:accountSettings.index>0;onClicked:root.moveAccount(accountSettings.item.id,-1)}
                    FluidButton{glyph:"chevron-down";tip:"Move account down";enabled:accountSettings.index<(Mail.settings.accounts||[]).length-1;onClicked:root.moveAccount(accountSettings.item.id,1)}
                    FluidButton{text:"Clear account cache";onClicked:Mail.send("cacheClear",{account:accountSettings.item.id})}
                    FluidButton{text:"Remove account…";onClicked:root.confirmationRequested("Remove account from Luma?","Disconnect "+accountSettings.item.email+" and clear its disposable cache and recent-recipient metadata. Mail and drafts stay in Gmail.","Remove account","accountRemove",{account:accountSettings.item.id,confirmed:true})}
                }
                Text{textFormat:Text.PlainText;text:"Allocated account cache: "+((root.accountCache[accountSettings.item.id]||0)/1048576).toFixed(2)+" MiB";color:Theme.faint;font.pixelSize:Theme.unit(10)}
                FluidButton{text:"Edit online signature";glyph:"compose";enabled:accountSettings.item.enabled!==false&&(accountSettings.item.sender_controls||Mail.preview);tip:"Primary Gmail signature · connect sender controls first";onClicked:root.signatureRequested(accountSettings.item.id)}
                Column{width:parent.width;spacing:Theme.unit(8);visible:root.rulesAccount===accountSettings.item.id
                    Text{textFormat:Text.PlainText;width:parent.width;wrapMode:Text.Wrap;text:"These Gmail filters move future mail to Trash even when Luma is closed. Unblock removes only the unchanged rule Luma created.";color:Theme.muted;font.pixelSize:Theme.unit(11)}
                    ListView{id:rules;width:parent.width;height:Math.min(count,5)*Theme.unit(46);model:accountSettings.item.blocked_senders||[];clip:true;reuseItems:true;cacheBuffer:Theme.unit(46)
                        ScrollBar.vertical:ScrollBar{implicitWidth:3}
                        delegate:Item{required property var modelData;width:rules.width;height:Theme.unit(46)
                            Text{textFormat:Text.PlainText;width:parent.width-Theme.unit(100);anchors.verticalCenter:parent.verticalCenter;text:modelData.address;elide:Text.ElideMiddle;color:Theme.text;font.pixelSize:Theme.unit(11)}
                            FluidButton{anchors.right:parent.right;anchors.verticalCenter:parent.verticalCenter;text:"Unblock";onClicked:Mail.send("senderRule",{account:accountSettings.item.id,sender:modelData.address,operation:"unblock",confirmed:true})}
                        }
                    }
                    Text{textFormat:Text.PlainText;text:"No sender rules saved by Luma";visible:(accountSettings.item.blocked_senders||[]).length===0;color:Theme.faint;font.pixelSize:Theme.unit(11)}
                }
            }}
            FluidButton {text:"Sign in with Google";glyph:"add";primary:true;onClicked:Mail.send("authStart",{drive:true,settings:true,photos:true})}
            Text{textFormat:Text.PlainText;width:parent.width;wrapMode:Text.Wrap;text:"Photos come from Google Contacts and your connected accounts where Google exposes them. Gmail can display additional private profile pictures that other clients cannot retrieve; these senders keep their initials.";color:Theme.muted;font.pixelSize:Theme.unit(11)}
            Row{spacing:Theme.unit(12)
                FluidButton{text:"Retry visible photos";onClicked:{Mail.send("photosReset");root.notice="Retrying supported photos. The lookup rate limit still applies."}}
                Text{textFormat:Text.PlainText;anchors.verticalCenter:parent.verticalCenter;text:(root.photoStats.images||0)+" photos · "+((root.photoStats.bytes||0)/1048576).toFixed(2)+" MiB RAM";color:Theme.muted;font.pixelSize:Theme.unit(11)}
            }
            Rectangle{width:parent.width;height:1;color:Theme.outline}
            Text {textFormat:Text.PlainText;text:"YOUR INTERFACE";font.pixelSize:Theme.unit(10);font.letterSpacing:1.2;color:Theme.faint}
            Row{width:parent.width;spacing:Theme.unit(20);Text {textFormat:Text.PlainText;text:"UI scale";color:Theme.text;font.pixelSize:Theme.unit(13);anchors.verticalCenter:parent.verticalCenter}Slider{from:75;to:200;stepSize:5;value:Mail.settings.ui_scale_percent||100;width:Theme.unit(250);onMoved:Mail.send("settingsUpdate",{ui_scale_percent:Math.round(value)})}Text {textFormat:Text.PlainText;text:Math.round(Mail.settings.ui_scale_percent||100)+"%";color:Theme.muted;anchors.verticalCenter:parent.verticalCenter}}
            Row{width:parent.width;spacing:Theme.unit(15)
                Text{textFormat:Text.PlainText;width:Theme.unit(140);text:"Appearance";color:Theme.text;font.pixelSize:Theme.unit(12);anchors.verticalCenter:parent.verticalCenter}
                FluidCombo{width:parent.width-Theme.unit(155);model:["System","Light","Dark","Noctalia"];currentIndex:["system","light","dark","noctalia"].indexOf(Mail.settings.theme||"system");onActivated:Mail.send("settingsUpdate",{theme:["system","light","dark","noctalia"][currentIndex]})}
            }
            FluidCheck{objectName:"compactRowsChoice";text:"Compact inbox rows";checked:Theme.compact;onToggled:Mail.send("settingsUpdate",{compact:checked})}
            FluidCheck{text:"Reduce motion";checked:Mail.settings.reduced_motion||false;onToggled:Mail.send("settingsUpdate",{reduced_motion:checked})}
            Text{textFormat:Text.PlainText;text:"SYNC & DESKTOP";font.pixelSize:Theme.unit(10);font.letterSpacing:1.2;color:Theme.faint}
            Row{width:parent.width;spacing:Theme.unit(15)
                Text{textFormat:Text.PlainText;width:Theme.unit(140);text:"Check for new mail";color:Theme.text;font.pixelSize:Theme.unit(12);anchors.verticalCenter:parent.verticalCenter}
                FluidCombo{width:parent.width-Theme.unit(155);model:["Every 30 seconds","Every minute","Every 90 seconds","Every 2 minutes","Every 5 minutes"];currentIndex:[30,60,90,120,300].indexOf(Mail.settings.poll_seconds||60);onActivated:Mail.send("settingsUpdate",{poll_seconds:[30,60,90,120,300][currentIndex]})}
            }
            Text{textFormat:Text.PlainText;width:parent.width;wrapMode:Text.Wrap;text:"Automatic checks fetch change metadata silently. New mail appears without resetting the list or reader. Bodies and attachments still load only when opened.";color:Theme.muted;font.pixelSize:Theme.unit(11)}
            FluidCheck{text:"Hide sender and subject in system notifications";checked:Mail.settings.notification_privacy||false;onToggled:Mail.send("settingsUpdate",{notification_privacy:checked})}
            FluidCheck{text:"Keep running in the tray when closed";checked:Mail.settings.close_to_tray!==false;onToggled:Mail.send("settingsUpdate",{close_to_tray:checked})}
            Text{textFormat:Text.PlainText;width:parent.width;wrapMode:Text.Wrap;text:Mail.notificationStatus;visible:text!=="";color:Theme.muted;font.pixelSize:Theme.unit(11)}
            Text{textFormat:Text.PlainText;text:Mail.sync.lastChecked?"Last checked: "+new Date(Mail.sync.lastChecked*1000).toLocaleTimeString():"";visible:text!=="";color:Theme.muted;font.pixelSize:Theme.unit(11)}
            Repeater{model:Mail.sync.errors||[];delegate:Text{required property var modelData;textFormat:Text.PlainText;width:parent.width;wrapMode:Text.Wrap;text:modelData.message;color:Theme.muted;font.pixelSize:Theme.unit(11)}}
            Text{textFormat:Text.PlainText;text:"COMPOSING";font.pixelSize:Theme.unit(10);font.letterSpacing:1.2;color:Theme.faint}
            Row{width:parent.width;spacing:Theme.unit(15)
                Text{textFormat:Text.PlainText;width:Theme.unit(140);text:"On composer close";color:Theme.text;font.pixelSize:Theme.unit(12);anchors.verticalCenter:parent.verticalCenter}
                FluidCombo{objectName:"draftModeChoice";width:parent.width-Theme.unit(155);model:["Ask to save","Save eligible drafts to Gmail","Manual save only"];currentIndex:["ask","on_close","manual"].indexOf(Mail.settings.draft_mode||"ask");onActivated:Mail.send("settingsUpdate",{draft_mode:["ask","on_close","manual"][currentIndex]})}
            }
            Text{textFormat:Text.PlainText;width:parent.width;wrapMode:Text.Wrap;text:"Automatic close-time saving requires a recipient and authored body. Typing and attachment uploads never autosave. Saved drafts stay in Gmail.";color:Theme.muted;font.pixelSize:Theme.unit(11)}
            FluidCheck{text:"Suggest recently sent recipients";checked:Mail.settings.recent_recipients!==false;onToggled:Mail.send("settingsUpdate",{recent_recipients:checked})}
            Text{textFormat:Text.PlainText;width:parent.width;wrapMode:Text.Wrap;text:"Optional recipient metadata is limited to 100 entries, 64 KiB and 90 days. It contains no email bodies or attachments.";color:Theme.muted;font.pixelSize:Theme.unit(11)}
            FluidButton{text:"Clear recent recipients";onClicked:Mail.send("recentClear")}
            Text{textFormat:Text.PlainText;text:root.notice;visible:text!=="";color:Theme.accent;font.pixelSize:Theme.unit(11)}
            Text {textFormat:Text.PlainText;text:"STORAGE & PRIVACY";font.pixelSize:Theme.unit(10);font.letterSpacing:1.2;color:Theme.faint}
            FluidButton{objectName:"testSystemNotification";text:"Test system notification";enabled:!Mail.preview;tip:"Show a synthetic code notification with Open and Copy code actions in the installed app";onClicked:Mail.testNotification()}
            FluidCheck{objectName:"autoLoadImages";text:"Automatically load images in opened emails";checked:Mail.settings.auto_load_images===true;onToggled:Mail.send("settingsUpdate",{auto_load_images:checked})}
            Text{textFormat:Text.PlainText;width:parent.width;wrapMode:Text.Wrap;text:"External image servers may learn that you opened the email. Only expanded messages are loaded, within the existing image limits. Image pixels stay in memory and are released when the email closes.";color:Theme.muted;font.pixelSize:Theme.unit(11)}
            FluidCheck{objectName:"autoPreviewAttachments";text:"Preview image attachments in opened emails";checked:Mail.settings.auto_preview_attachments!==false;onToggled:Mail.send("settingsUpdate",{auto_preview_attachments:checked})}
            Text {textFormat:Text.PlainText;width:parent.width;text:"Mail stays in Gmail. Only the configured, bounded cache is retained. No mailbox mirror or local draft store.";wrapMode:Text.Wrap;color:Theme.muted;font.pixelSize:Theme.unit(12)}
            Row{spacing:Theme.unit(12);FluidCombo{width:Theme.unit(170);model:["Cache off","50 MiB","100 MiB","250 MiB","500 MiB"];currentIndex:[0,50,100,250,500].indexOf(Mail.settings.cache_mib||0);onActivated:Mail.send("settingsUpdate",{cache_mib:[0,50,100,250,500][currentIndex]})}FluidButton{text:"Clear cache";onClicked:Mail.send("cacheClear")}}
            Row{spacing:Theme.unit(10)
                Text{textFormat:Text.PlainText;anchors.verticalCenter:parent.verticalCenter;text:"Custom budget (MiB)";color:Theme.text;font.pixelSize:Theme.unit(12)}
                FluidField{id:customCache;width:Theme.unit(100);text:String(Mail.settings.cache_mib||0);validator:IntValidator{bottom:0;top:1048576}}
                FluidButton{text:"Apply";enabled:customCache.acceptableInput;onClicked:Mail.send("settingsUpdate",{cache_mib:Number(customCache.text)})}
            }
            Row{spacing:Theme.unit(10)
                Text{textFormat:Text.PlainText;anchors.verticalCenter:parent.verticalCenter;text:"Expire after (hours)";color:Theme.text;font.pixelSize:Theme.unit(12)}
                FluidField{id:cacheHours;width:Theme.unit(100);text:String(Mail.settings.cache_ttl_hours||0);validator:IntValidator{bottom:0;top:87600}}
                FluidButton{text:"Apply";enabled:cacheHours.acceptableInput;onClicked:Mail.send("settingsUpdate",{cache_ttl_hours:Number(cacheHours.text)})}
            }
            FluidCheck{text:"Clear disposable cache on quit";checked:Mail.settings.clear_cache_on_exit||false;onToggled:Mail.send("settingsUpdate",{clear_cache_on_exit:checked})}
            Text{textFormat:Text.PlainText;text:root.cacheBytes<0?"":"Allocated cache: "+(root.cacheBytes/1048576).toFixed(2)+" MiB · limit "+(Mail.settings.cache_mib||0)+" MiB";color:Theme.muted;font.pixelSize:Theme.unit(11)}
        }
        }
        }
    }
    Connections{target:Mail;function onReceived(event,data){if(event==="recentCleared")root.notice="Recent recipients cleared";else if(event==="cacheUsage"){root.cacheBytes=data.bytes;root.accountCache=data.accounts||{}}}function onProfileChanged(key){if(root.opened)root.updatePhotoStats()}function onProfilesReset(){if(root.opened)root.updatePhotoStats()}}
}
